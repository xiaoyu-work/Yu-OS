//! App access to the settings slots other than chat.

mod local;
mod providers;

use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::llm::run_log::{self, LlmRunRecord};
use crate::agent::llm::types::{FinishReason, Usage};
use crate::agent::media::tts::AudioFormat;
use crate::caps::manifest::AiSafety;
use crate::caps::{Cap, Scope, Verb};
use crate::model::tasks::decision::{DecisionRequest, Question};
use crate::yuosd::authority::Decision;
use crate::yuosd::routes::Command;
use crate::yuosd::wire::bounded::AI_INFERENCE_REQUEST_BYTES;

use super::gate::{self, AiError};
use super::model_service;

pub use local::serve_local;

pub(super) fn target_identity(
    verb: Verb,
    config: &crate::config::YuConfig,
) -> Option<(String, String)> {
    providers::Target::resolve(verb, config)
        .ok()
        .filter(providers::Target::is_configured)
        .map(|target| (target.provider, target.model))
}

const AUDIO_INPUT_BYTES: usize = 512 * 1024;
const MEDIA_OUTPUT_BYTES: usize = 3 * 1024 * 1024 - 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceRequest {
    origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_units: Option<u32>,
    input: Input,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Input {
    Embed {
        text: String,
    },
    Tts {
        text: String,
    },
    Stt {
        audio_base64: String,
    },
    Imagegen {
        prompt: String,
    },
    Decision {
        state: Value,
        questions: BTreeMap<String, Question>,
    },
}

/// Detect the audio container from its leading bytes.
fn audio_format(audio: &[u8]) -> Option<AudioFormat> {
    match audio {
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'A', b'V', b'E', ..] => Some(AudioFormat::Wav),
        [b'O', b'g', b'g', b'S', ..] => Some(AudioFormat::Ogg),
        // An ID3 tag, or an MPEG audio frame sync (11 set bits) with a defined
        // layer. AAC ADTS uses the reserved layer 00.
        [b'I', b'D', b'3', ..] => Some(AudioFormat::Mp3),
        [0xff, second, ..] if *second & 0xe0 == 0xe0 && *second & 0x06 != 0 => {
            Some(AudioFormat::Mp3)
        }
        _ => None,
    }
}

enum Prepared {
    Embed(String),
    Tts(String),
    Stt(Vec<u8>, AudioFormat),
    Imagegen(String),
    Decision(DecisionRequest),
}

impl Input {
    fn verb(&self) -> Verb {
        match self {
            Self::Embed { .. } => Verb::AI_EMBED,
            Self::Tts { .. } => Verb::AI_AUDIO_TTS,
            Self::Stt { .. } => Verb::AI_AUDIO_STT,
            Self::Imagegen { .. } => Verb::AI_IMAGE_GENERATE,
            Self::Decision { .. } => Verb::AI_DECIDE,
        }
    }

    fn prepare(self, safety: AiSafety) -> Result<Prepared, AiError> {
        fn text(value: String, safety: AiSafety) -> Result<String, AiError> {
            if value.trim().is_empty() {
                return Err(AiError::InvalidInference("AI input text cannot be blank"));
            }
            Ok(gate::apply_safety(&value, safety).0)
        }
        Ok(match self {
            Self::Embed { text: data } => Prepared::Embed(text(data, safety)?),
            Self::Tts { text: data } => Prepared::Tts(text(data, safety)?),
            Self::Imagegen { prompt } => Prepared::Imagegen(text(prompt, safety)?),
            Self::Stt { audio_base64 } => {
                let audio = STANDARD.decode(audio_base64).map_err(|_| {
                    AiError::InvalidInference("audio must be standard padded base64")
                })?;
                if audio.is_empty() || audio.len() > AUDIO_INPUT_BYTES {
                    return Err(AiError::InvalidInference(
                        "audio must contain 1 to 524288 bytes",
                    ));
                }
                let format = audio_format(&audio).ok_or(AiError::InvalidInference(
                    "unsupported audio format; send WAV, MP3 or Ogg",
                ))?;
                Prepared::Stt(audio, format)
            }
            Self::Decision { state, questions } => {
                let request = DecisionRequest { state, questions };
                request
                    .validate()
                    .map_err(|_| AiError::InvalidInference("invalid decision questions"))?;
                if serde_json::to_vec(&request).map_err(internal)?.len()
                    > crate::model::tasks::decision::MAX_REQUEST_BYTES - 256
                {
                    return Err(AiError::InvalidInference(
                        "decision input exceeds its byte limit",
                    ));
                }
                // System One receives this data locally, not through the cloud
                // prompt pipeline. Preserve its structured state and question IDs.
                Prepared::Decision(request)
            }
        })
    }
}

impl Prepared {
    fn units(&self) -> Result<u32, AiError> {
        Ok(match self {
            Self::Embed(text) => text.len() as u32,
            Self::Tts(text) => model_service::speech_units(text) as u32,
            Self::Stt(audio, _) => model_service::transcription_units(audio.len()) as u32,
            Self::Imagegen(_) => model_service::image_units(1) as u32,
            Self::Decision(request) => model_service::decision_units(request).map_err(internal)? as u32,
        })
    }

    fn audit_input(&self) -> Value {
        match self {
            Self::Embed(text) => json!({"kind": "embed", "text": text}),
            Self::Tts(text) => json!({"kind": "tts", "text": text}),
            Self::Imagegen(prompt) => json!({"kind": "imagegen", "prompt": prompt}),
            Self::Stt(audio, format) => json!({
                "kind": "stt", "format": format.extension(), "bytes": audio.len(),
                "sha256": crate::crypto::sha256_hex(audio),
            }),
            Self::Decision(request) => json!({"kind": "decision", "request": request}),
        }
    }
}

fn internal(error: serde_json::Error) -> AiError {
    AiError::Internal(error.to_string())
}

fn decode(mut value: Value) -> Result<InferenceRequest, &'static str> {
    yuos_sdk::generated::validate_ai_inference_request(&value)
        .map_err(|_| "invalid App AI inference input")?;
    yuos_sdk::generated::normalize_ai_inference_request_integers(&mut value);
    serde_json::from_value(value).map_err(|_| "invalid App AI inference input")
}

pub(crate) fn cli(args: &[String], stdin: Option<&[u8]>) -> Result<Value, String> {
    if args != ["--request-stdin"] {
        return Err(super::invalid_args("usage: yu ai infer --request-stdin"));
    }
    let data = stdin
        .filter(|data| data.len() <= AI_INFERENCE_REQUEST_BYTES)
        .ok_or_else(|| super::invalid_args("AI inference requires bounded JSON on stdin"))?;
    let request = serde_json::from_slice(data)
        .map_err(|_| super::invalid_args("AI inference requires valid JSON"))
        .and_then(|value| decode(value).map_err(super::invalid_args))?;
    let app = std::env::var("YUOS_APP_ID")
        .map_err(|_| super::permission_denied("App identity is required", None))?;
    let session = std::env::var("YUOS_SESSION")
        .map_err(|_| super::permission_denied("App session is required", None))?;
    super::chat::broker_request(
        Command::AiInference,
        json!({"session": session, "app_id": app, "request": request}),
    )
}

pub(crate) async fn run(
    app: &str,
    request: InferenceRequest,
    authority: &Decision,
    home: &std::path::Path,
) -> Result<Value, AiError> {
    let started = std::time::Instant::now();
    let verb = request.input.verb();
    let mut model = String::new();
    let result = run_inner(app, request, authority, home, &mut model).await;
    let duration = started.elapsed().as_millis() as u64;
    let rec = match &result {
        Ok(result) => LlmRunRecord::from_success(
            result["provider"].as_str().unwrap_or_default(),
            &model,
            None,
            FinishReason::Stop,
            &Usage::default(),
            duration,
            authority.session_id(),
        )
        .with_app(app),
        Err(error) => LlmRunRecord::from_denial(
            app,
            &model,
            gate::denial_reason_token(error),
            &error.to_string(),
            duration,
            authority.session_id(),
        ),
    };
    run_log::record(&rec.with_verb(verb.as_str()));
    result
}

async fn run_inner(
    app: &str,
    request: InferenceRequest,
    authority: &Decision,
    home: &std::path::Path,
    model: &mut String,
) -> Result<Value, AiError> {
    let canonical = serde_json::to_value(request).map_err(internal)?;
    if serde_json::to_vec(&canonical).map_err(internal)?.len() > AI_INFERENCE_REQUEST_BYTES {
        return Err(AiError::InvalidInference(
            "AI inference input exceeds its byte limit",
        ));
    }
    let request = decode(canonical).map_err(AiError::InvalidInference)?;
    let origin = request.origin.as_str();
    let verb = request.input.verb();
    let cfg = crate::config::current_snapshot();
    let target = providers::Target::resolve(verb, &cfg)?;
    model.clone_from(&target.model);
    let authorization = authority.require(Cap::new(verb, Scope::name(&target.model)));
    let (policy, app_cap) = gate::app_policy(app, origin, verb, Some(authority))?;
    let _authorized = authorization.map_err(|error| AiError::Denied(json!({"error": error})))?;
    if !target.is_configured() {
        return Err(AiError::Provider("The configured AI model is unavailable; check Yu OS settings".into()));
    }
    let input = request.input.prepare(policy.safety)?;
    let units = input.units()?;
    if let Some(limit) = request.max_units.filter(|limit| units > *limit) {
        return Err(AiError::RequestBudgetExceeded {
            required: units.into(),
            limit: limit.into(),
        });
    }
    let call = model_service::begin_app_call(
        Some(authority),
        app,
        origin,
        app_cap,
        units.into(),
        &json!({
            "provider": target.provider, "model": target.model, "input": input.audit_input(),
        }),
    )?;
    let (output, units, _) = call.run(
        target.run(input, authority.owner_uid(), home),
        |_| units.into(),
    ).await?;
    let result = json!({"provider": target.provider, "model": target.model, "units": units, "output": output});
    yuos_sdk::generated::validate_ai_inference(&result)
        .map_err(|_| AiError::Provider("model returned an invalid inference result".into()))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test/unit/ai/inference.rs"));
}
