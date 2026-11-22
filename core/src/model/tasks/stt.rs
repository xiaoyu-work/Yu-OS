//! Speech-to-text task — audio → text via OpenAI-compatible
//! `/audio/transcriptions` (preserves source language) and
//! `/audio/translations` (translates to English).
//!
//! Phase 1.5: cloud only. Local Whisper via ort lands when ONNX files
//! are supplied.
//!
//! Azure OpenAI uses a resource URL and deployment name with API-key auth.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::SttConfig;

pub(crate) mod azure;

// =====================================================================
// Public types
// =====================================================================

/// Whether to keep the source language (`Transcribe`) or render to
/// English (`Translate`). Maps to `/audio/transcriptions` and
/// `/audio/translations` respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SttMode {
    Transcribe,
    Translate,
}

impl SttMode {
    fn path(self) -> &'static str {
        match self {
            Self::Transcribe => "audio/transcriptions",
            Self::Translate => "audio/translations",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttRequest {
    /// Raw audio bytes — already-encoded file (mp3/wav/m4a/flac/...).
    pub audio: Vec<u8>,
    /// Hint at the upload's filename so the multipart part has a useful
    /// extension. Provider sniffs the format from this.
    pub filename: String,
    /// Optional source language hint (BCP-47, e.g. "en", "zh"). Only
    /// used for `Transcribe` mode.
    pub language: Option<String>,
    /// Optional prompt (style/spelling biasing).
    pub prompt: Option<String>,
    /// Response shape: `"json"`, `"text"`, `"verbose_json"`, `"srt"`, `"vtt"`.
    pub response_format: Option<String>,
    /// Sampling temperature [0.0–1.0].
    pub temperature: Option<f32>,
    pub mode: SttMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttResponse {
    pub text: String,
    pub model: String,
    /// Detected source language (if provider returned it).
    pub language: Option<String>,
    /// Raw provider response — preserved when caller asks for verbose_json
    /// or non-JSON formats so segment/word timestamps survive.
    pub raw: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("not configured: set [stt] block in config.json")]
    NotConfigured,
    #[error("authentication failed: bad or missing API key")]
    Auth,
    #[error("rate limited (retry after {retry_after_ms}ms)")]
    RateLimited { retry_after_ms: u64 },
    #[error("provider returned error: {status} — {message}")]
    Provider { status: u16, message: String },
    #[error("transport: {0}")]
    Transport(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

#[async_trait]
pub trait SpeechToText: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn is_configured(&self) -> bool;
    async fn transcribe(&self, request: SttRequest) -> Result<SttResponse, SttError>;
}

// =====================================================================
// Factory
// =====================================================================

pub fn build_default() -> Result<Option<Box<dyn SpeechToText>>, String> {
    build_from(&crate::config::current_snapshot().stt)
}

pub fn build_from(cfg: &SttConfig) -> Result<Option<Box<dyn SpeechToText>>, String> {
    match cfg.provider.as_str() {
        "none" | "" => Ok(None),
        alias if alias.trim().eq_ignore_ascii_case("azure") => {
            Ok(Some(Box::new(azure::AzureStt::from_config(cfg)?)))
        }
        "openai" | "groq" | "mistral" | "deepseek" | "openrouter" | "ollama" => {
            Ok(Some(Box::new(OpenAICompatStt::from_config(cfg))))
        }
        other => Err(format!("unknown stt provider: {other}")),
    }
}

// =====================================================================
// OpenAI-compatible cloud STT
// =====================================================================

const DEFAULT_OPENAI_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_GROQ_BASE: &str = "https://api.groq.com/openai/v1";
const DEFAULT_MISTRAL_BASE: &str = "https://api.mistral.ai/v1";

fn default_base_url(provider: &str) -> &'static str {
    match provider {
        "groq" => DEFAULT_GROQ_BASE,
        "mistral" => DEFAULT_MISTRAL_BASE,
        _ => DEFAULT_OPENAI_BASE,
    }
}

pub struct OpenAICompatStt {
    alias: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
    extra_headers: HashMap<String, String>,
    default_response_format: String,
    client: reqwest::Client,
}

impl OpenAICompatStt {
    pub fn from_config(cfg: &SttConfig) -> Self {
        let alias = cfg.provider.clone();
        let base_url = cfg
            .base_url
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_base_url(&cfg.provider).to_string());
        let base_url = base_url.trim_end_matches('/').to_string();

        let api_key = crate::agent::llm::construction::resolve_process_api_key(
            cfg.api_key_credential.as_deref(),
            cfg.api_key_env.as_deref(),
        )
        .ok()
        .flatten();

        let timeout = if cfg.request_timeout == 0 {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(cfg.request_timeout)
        };
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            alias,
            base_url,
            api_key,
            model: cfg.model.clone(),
            extra_headers: cfg.extra_headers.clone(),
            default_response_format: cfg.default_response_format.clone(),
            client,
        }
    }

    fn endpoint(&self, mode: SttMode) -> String {
        let path = mode.path();
        let (base, query) = match self.base_url.split_once('?') {
            Some((b, q)) => (b.trim_end_matches('/'), Some(q)),
            None => (self.base_url.as_str(), None),
        };
        match query {
            Some(q) => format!("{base}/{path}?{q}"),
            None => format!("{base}/{path}"),
        }
    }
}

#[async_trait]
impl SpeechToText for OpenAICompatStt {
    fn name(&self) -> &str {
        &self.alias
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn transcribe(&self, request: SttRequest) -> Result<SttResponse, SttError> {
        let response_format = request
            .response_format
            .clone()
            .unwrap_or_else(|| self.default_response_format.clone());
        let endpoint = self.endpoint(request.mode);
        // Keep legacy manually configured deployment-style gateways working.
        let model = (!self.base_url.contains("/deployments/")).then_some(self.model.as_str());
        let form = request_form(request, model, &response_format)?;
        let mut http = self.client.post(endpoint).multipart(form);
        if let Some(key) = &self.api_key {
            http = http.bearer_auth(key);
        }
        for (k, v) in &self.extra_headers {
            http = http.header(k.as_str(), v.as_str());
        }
        let resp = http
            .send()
            .await
            .map_err(|e| SttError::Transport(e.to_string()))?;
        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| SttError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(classify_http_error(status.as_u16(), &bytes));
        }

        parse_response(&bytes, &response_format, &self.model)
    }
}

fn request_form(
    request: SttRequest,
    model: Option<&str>,
    format: &str,
) -> Result<reqwest::multipart::Form, SttError> {
    if request.audio.is_empty() {
        return Err(SttError::InvalidInput(
            "audio bytes must not be empty".into(),
        ));
    }
    if request.filename.trim().is_empty() {
        return Err(SttError::InvalidInput("filename must not be empty".into()));
    }

    let mime = guess_mime(&request.filename);
    let part = reqwest::multipart::Part::bytes(request.audio)
        .file_name(request.filename)
        .mime_str(&mime)
        .map_err(|e| SttError::Transport(e.to_string()))?;
    let mut form = reqwest::multipart::Form::new().part("file", part);
    if let Some(model) = model {
        form = form.text("model", model.to_owned());
    }
    form = form.text("response_format", format.to_owned());
    if let Some(t) = request.temperature {
        form = form.text("temperature", t.to_string());
    }
    if let Some(p) = request.prompt {
        form = form.text("prompt", p);
    }
    if request.mode == SttMode::Transcribe {
        if let Some(lang) = request.language {
            form = form.text("language", lang);
        }
    }
    Ok(form)
}

fn parse_response(bytes: &[u8], format: &str, model: &str) -> Result<SttResponse, SttError> {
    if matches!(format, "text" | "srt" | "vtt") {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let raw = serde_json::Value::String(text.clone());
        return Ok(SttResponse {
            text,
            model: model.into(),
            language: None,
            raw,
        });
    }

    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| SttError::Parse(e.to_string()))?;
    let text = raw
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let language = raw
        .get("language")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(SttResponse {
        text,
        model: model.into(),
        language,
        raw,
    })
}

fn guess_mime(filename: &str) -> String {
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => "audio/mpeg".into(),
        "m4a" | "mp4" => "audio/mp4".into(),
        "wav" => "audio/wav".into(),
        "flac" => "audio/flac".into(),
        "ogg" | "opus" => "audio/ogg".into(),
        "webm" => "audio/webm".into(),
        _ => "application/octet-stream".into(),
    }
}

fn classify_http_error(status: u16, bytes: &[u8]) -> SttError {
    let message = serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).chars().take(400).collect());
    match status {
        401 | 403 => SttError::Auth,
        429 => SttError::RateLimited {
            retry_after_ms: 1000,
        },
        _ => SttError::Provider { status, message },
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/model/tasks/stt.rs"
    ));
}
