//! AI helper for the stable `chat` / `chat-untrusted` surface.
//!
//! [`chat`] shells out to `yu ai chat --app <id>`. Passing
//! `origin("external-content")` makes the kernel select
//! `ai.chat.untrusted`.
//!
//! ```no_run
//! use yuos_sdk::ai;
//!
//! fn summarise(body: &str) -> Result<String, ai::AiError> {
//!     let r = ai::chat(body, ai::ChatOpts::default().origin("external-content").max_units(2000))?;
//!     Ok(r.text)
//! }
//! ```

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{yu_call_json_structured, BridgeError, StructuredCosError};
pub use crate::generated::{
    AiAppTool, AiChatImage, AiChatRequest, AiChatToolCall, AiChatToolResult, AiInference, AiInferenceRequest,
    AiToolTurn,
};

// ---------------------------------------------------------------------------
// Response shape
// ---------------------------------------------------------------------------

/// Token / unit accounting for a single call.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub units: u64,
}

/// Snapshot of the app's budget *after* this call.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub period: String,
    pub units_used: u64,
    pub units_cap: u64,
}

/// Safety / review metadata.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Review {
    pub safety: String,
    pub prompt_redacted: bool,
}

/// A proposed call to an App function, not authorization to execute it.
pub type ProposedToolCall = AiChatToolCall;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse<T> {
    pub provider: String,
    pub model: String,
    pub units: u64,
    pub output: T,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatOutput {
    pub text: String,
    pub tool_calls: Vec<ProposedToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub vector: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speech {
    pub format: String,
    pub audio_base64: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub format: String,
    pub image_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub answers: serde_json::Map<String, serde_json::Value>,
}

/// Parsed reply from `yu ai chat`. See `wire/v1/ai.schema.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiResponse {
    pub text: String,
    pub model: String,
    pub provider: String,
    #[serde(skip)]
    pub units: u64,
    #[serde(skip)]
    pub output: ChatOutput,
    pub verb: String,
    pub usage: Usage,
    pub budget: Budget,
    pub review: Review,
    #[serde(default)]
    pub tool_calls: Vec<ProposedToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_state: Option<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("ai: {0}")]
    InvalidArg(String),

    /// Gate refused (caps / origin / unknown app / unknown verb).
    #[error("ai denied: {message}")]
    Denied {
        message: String,
        code: String,
        payload: serde_json::Value,
    },

    /// Per-app monthly budget exhausted.
    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),

    /// Safety pipeline blocked the call.
    #[error("safety violation: {0}")]
    SafetyViolation(String),

    /// Transport / decode failure.
    #[error("ai unavailable: {0}")]
    Unavailable(String),

    #[error(transparent)]
    Bridge(#[from] BridgeError),
}

// ---------------------------------------------------------------------------
// Options builder
// ---------------------------------------------------------------------------

/// Options for [`chat`].
#[derive(Debug, Default, Clone)]
pub struct ChatOpts {
    origin: Option<String>,
    max_units: Option<u64>,
    system: Option<String>,
    app_id: Option<String>,
    tools: Option<Vec<String>>,
    images: Option<Vec<AiChatImage>>,
    functions: Option<Vec<AiAppTool>>,
    tool_turns: Option<Vec<AiToolTurn>>,
}

impl ChatOpts {
    /// Declare `trusted`, `user-input`, or `external-content`. Required for chat.
    pub fn origin(mut self, o: impl Into<String>) -> Self {
        self.origin = Some(o.into());
        self
    }
    pub fn max_units(mut self, n: u64) -> Self {
        self.max_units = Some(n);
        self
    }
    /// Optional App instruction, not the system Agent's prompt.
    pub fn system(mut self, s: impl Into<String>) -> Self {
        self.system = Some(s.into());
        self
    }
    pub fn app(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }
    pub fn images(mut self, images: Vec<AiChatImage>) -> Self {
        self.images = (!images.is_empty()).then_some(images);
        self
    }
    pub fn functions(mut self, functions: Vec<AiAppTool>) -> Self {
        self.functions = (!functions.is_empty()).then_some(functions);
        self
    }
    pub fn tool_turns(mut self, tool_turns: Vec<AiToolTurn>) -> Self {
        self.tool_turns = (!tool_turns.is_empty()).then_some(tool_turns);
        self
    }
    /// Legacy catalog names are not accepted. Use [`chat_request`] for App functions.
    pub fn tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tools = Some(tools.into_iter().map(Into::into).collect());
        self
    }
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Single-shot chat completion. Requires an explicit [`ChatOpts::origin`].
pub fn chat(prompt: &str, opts: ChatOpts) -> Result<AiResponse, AiError> {
    if prompt.trim().is_empty() {
        return Err(AiError::InvalidArg("chat: prompt must be non-empty".into()));
    }
    let origin = opts
        .origin
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AiError::InvalidArg("chat: prompt origin is required".into()))?;
    if opts.images.is_some() || opts.functions.is_some() || opts.tool_turns.is_some() {
        if opts.tools.as_ref().is_some_and(|tools| !tools.is_empty()) {
            return Err(AiError::InvalidArg("chat: supply App functions in functions".into()));
        }
        if opts.app_id.as_ref().is_some_and(|app| std::env::var("YUOS_APP_ID").ok().as_ref() != Some(app)) {
            return Err(AiError::Denied {
                message: "chat: App identity does not match the session".into(),
                code: "PERMISSION_DENIED".into(),
                payload: serde_json::json!({"code": "PERMISSION_DENIED", "error": "chat: App identity does not match the session"}),
            });
        }
        return chat_request(&AiChatRequest {
            origin: origin.into(),
            prompt: prompt.into(),
            system: opts.system,
            images: opts.images,
            max_units: opts.max_units,
            tools: opts.functions,
            tool_turns: opts.tool_turns,
        });
    }
    dispatch(prompt, origin, &opts)
}

/// Call the configured text model with optional App functions and prior results.
pub fn chat_request(request: &AiChatRequest) -> Result<AiResponse, AiError> {
    let value = serde_json::to_value(request)
        .map_err(|error| AiError::InvalidArg(error.to_string()))?;
    let input = serde_json::to_vec(&value)
        .map_err(|error| AiError::InvalidArg(error.to_string()))?;
    if input.len() > 720 * 1024 {
        return Err(AiError::InvalidArg("App chat request exceeds the byte limit".into()));
    }
    crate::generated::validate_ai_chat_request(&value)
        .map_err(|error| AiError::InvalidArg(error.to_string()))?;
    match crate::yu_call_json_structured_input(
        "ai", "chat", ["ai", "chat", "--request-stdin"], Some(&input),
        std::process::Stdio::piped(), None,
    ) {
        Ok(reply) => parse_response(reply),
        Err(StructuredCosError::App(error)) =>
            Err(classify_ai_error(&error.message, &error.payload)),
        Err(StructuredCosError::Bridge(error)) => Err(AiError::Bridge(error)),
    }
}

/// Run the owner's configured embedding, speech, image, or decision model.
pub fn infer(request: &AiInferenceRequest) -> Result<AiInference, AiError> {
    let value = serde_json::to_value(request)
        .map_err(|error| AiError::InvalidArg(error.to_string()))?;
    let input = serde_json::to_vec(&value)
        .map_err(|error| AiError::InvalidArg(error.to_string()))?;
    if input.len() > 720 * 1024 {
        return Err(AiError::InvalidArg("AI request exceeds the byte limit".into()));
    }
    crate::generated::validate_ai_inference_request(&value)
        .map_err(|error| AiError::InvalidArg(error.to_string()))?;
    let response = crate::yu_call_json_structured_input(
        "ai", "infer", ["ai", "infer", "--request-stdin"], Some(&input),
        std::process::Stdio::piped(), None,
    );
    let mut reply = match response {
        Ok(reply) => reply,
        Err(StructuredCosError::App(error)) =>
            return Err(classify_ai_error(&error.message, &error.payload)),
        Err(StructuredCosError::Bridge(error)) => return Err(AiError::Bridge(error)),
    };
    crate::generated::validate_ai_inference(&reply)
        .map_err(|error| AiError::Unavailable(error.to_string()))?;
    if reply["output"]["kind"] != value["input"]["kind"] {
        return Err(AiError::Unavailable("AI inference reply does not match the request".into()));
    }
    crate::generated::normalize_ai_inference_integers(&mut reply);
    serde_json::from_value(reply).map_err(|error| AiError::Unavailable(error.to_string()))
}

#[derive(Debug, Clone)]
pub struct ModelOpts {
    pub origin: String,
    pub max_units: Option<u32>,
}

impl ModelOpts {
    pub fn new(origin: impl Into<String>) -> Self {
        Self { origin: origin.into(), max_units: None }
    }
}

fn run_model<T: serde::de::DeserializeOwned>(
    input: serde_json::Value,
    opts: ModelOpts,
) -> Result<ModelResponse<T>, AiError> {
    let reply = infer(&AiInferenceRequest { origin: opts.origin, max_units: opts.max_units, input })?;
    let output = serde_json::from_value(reply.output)
        .map_err(|error| AiError::Unavailable(error.to_string()))?;
    Ok(ModelResponse { provider: reply.provider, model: reply.model, units: reply.units.into(), output })
}

pub fn embed(text: &str, opts: ModelOpts) -> Result<ModelResponse<Embedding>, AiError> {
    run_model(serde_json::json!({"kind": "embed", "text": text}), opts)
}

pub fn tts(text: &str, opts: ModelOpts) -> Result<ModelResponse<Speech>, AiError> {
    run_model(serde_json::json!({"kind": "tts", "text": text}), opts)
}

pub fn stt(audio_base64: &str, format: &str, opts: ModelOpts) -> Result<ModelResponse<Transcription>, AiError> {
    run_model(serde_json::json!({"kind": "stt", "audio_base64": audio_base64, "format": format}), opts)
}

pub fn imagegen(prompt: &str, opts: ModelOpts) -> Result<ModelResponse<Image>, AiError> {
    run_model(serde_json::json!({"kind": "imagegen", "prompt": prompt}), opts)
}

pub fn decide(
    state: serde_json::Value,
    questions: serde_json::Map<String, serde_json::Value>,
    opts: ModelOpts,
) -> Result<ModelResponse<Decision>, AiError> {
    run_model(serde_json::json!({"kind": "decision", "state": state, "questions": questions}), opts)
}

/// Open Applications settings after a user action.
pub fn open_permission_settings() -> Result<(), AiError> {
    match yu_call_json_structured("ai", "settings", ["ai", "settings"]) {
        Ok(_) => Ok(()),
        Err(StructuredCosError::App(error)) =>
            Err(classify_ai_error(&error.message, &error.payload)),
        Err(StructuredCosError::Bridge(error)) => Err(AiError::Bridge(error)),
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

struct PrivateInputFile {
    path: PathBuf,
}

impl PrivateInputFile {
    fn new(label: &str, contents: &str) -> Result<Self, AiError> {
        static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "yuos-ai-{}-{nonce}-{sequence}-{label}",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|error| AiError::Unavailable(format!("create private {label}: {error}")))?;
        if let Err(error) = file
            .write_all(contents.as_bytes())
            .and_then(|_| file.sync_all())
        {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(AiError::Unavailable(format!(
                "write private {label}: {error}"
            )));
        }
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateInputFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path) {
            eprintln!("yuos-sdk: failed to remove private AI input file: {error}");
        }
    }
}

fn dispatch(prompt: &str, origin: &str, opts: &ChatOpts) -> Result<AiResponse, AiError> {
    let app = opts
        .app_id
        .clone()
        .or_else(|| std::env::var("YUOS_APP_ID").ok())
        .ok_or_else(|| {
            AiError::InvalidArg(
                "chat: app id is required (pass .app(...) or set YUOS_APP_ID)".into(),
            )
        })?;

    let mut argv: Vec<OsString> = vec![
        "ai".into(),
        "chat".into(),
        "--app".into(),
        app.into(),
        "--origin".into(),
        origin.into(),
    ];
    let prompt_file = PrivateInputFile::new("prompt", prompt)?;
    argv.push("--prompt-file".into());
    argv.push(prompt_file.path().as_os_str().to_owned());
    if let Some(n) = opts.max_units {
        argv.push("--max-units".into());
        argv.push(n.to_string().into());
    }
    let system_file = opts
        .system
        .as_deref()
        .map(|system| PrivateInputFile::new("system", system))
        .transpose()?;
    if let Some(file) = &system_file {
        argv.push("--system-file".into());
        argv.push(file.path().as_os_str().to_owned());
    }
    if let Some(tools) = &opts.tools {
        argv.push("--tools".into());
        argv.push(tools.join(",").into());
    }

    let value = match yu_call_json_structured("ai", "chat", argv) {
        Ok(value) => value,
        Err(StructuredCosError::App(error)) => {
            let error = *error;
            let message = error.payload["error"]
                .as_str()
                .unwrap_or("AI request denied");
            return Err(classify_ai_error(message, &error.payload));
        }
        Err(StructuredCosError::Bridge(error)) => return Err(AiError::Bridge(error)),
    };
    parse_response(value)
}

fn parse_response(mut value: serde_json::Value) -> Result<AiResponse, AiError> {
    crate::generated::validate_ai(&value)
        .map_err(|error| AiError::Unavailable(format!("ai response decode failed: {error}")))?;
    crate::generated::normalize_ai_integers(&mut value);
    let mut resp: AiResponse = serde_json::from_value(value)
        .map_err(|e| AiError::Unavailable(format!("ai response decode failed: {e}")))?;
    resp.units = resp.usage.units;
    resp.output = ChatOutput {
        text: resp.text.clone(),
        tool_calls: resp.tool_calls.clone(),
        provider_state: resp.provider_state.clone(),
    };
    Ok(resp)
}

/// Best-effort redaction of an AI request/response payload for use in
/// error messages and Debug output. Replaces the bulky text /
/// embedding / image-bytes fields with their byte counts so the
/// surrounding diagnostic stays useful without leaking content.
fn redact_payload(value: &serde_json::Value) -> serde_json::Value {
    let mut redacted = value.clone();
    if let Some(obj) = redacted.as_object_mut() {
        for key in [
            "text",
            "prompt",
            "messages",
            "embedding",
            "embeddings",
            "data",
            "raw",
        ] {
            if let Some(v) = obj.get_mut(key) {
                let bytes = serde_json::to_string(v).map(|s| s.len()).unwrap_or(0);
                *v = serde_json::json!({
                    "redacted": true,
                    "approx_bytes": bytes,
                });
            }
        }
    }
    redacted
}

fn classify_ai_error(message: &str, payload: &serde_json::Value) -> AiError {
    let code = payload
        .get("code")
        .and_then(|v| v.as_str())
        .expect("validated wire error envelope must include code")
        .to_string();
    match code.as_str() {
        "BUDGET_EXCEEDED" => return AiError::BudgetExceeded(message.to_string()),
        "SAFETY_VIOLATION" => return AiError::SafetyViolation(message.to_string()),
        _ => {}
    }
    AiError::Denied {
        message: message.to_string(),
        code,
        // Never embed the raw kernel reply here: AI calls routinely
        // include user text, embedding vectors, or image bytes that
        // logs or error toasts would happily render. Redact to byte
        // counts so the structural diagnostic survives without the
        // payload.
        payload: redact_payload(payload),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test/unit/ai.rs"));
}
