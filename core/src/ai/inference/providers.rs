use std::path::Path;
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use yuos_embed::{EmbedRequest, EmbedResponse, Embedder};

use crate::agent::media::{factory, imagegen, stt, tts};
use crate::caps::Verb;
use crate::config::{DecisionConfig, EmbedConfig, YuConfig};
use crate::model::tasks::{decision, embed, qwen3_genai};

use super::{local, AiError, Prepared, MEDIA_OUTPUT_BYTES};

enum Backend {
    Embed(Box<dyn Embedder>),
    LocalEmbed(EmbedConfig),
    Speech(Arc<dyn tts::TtsProvider>),
    Transcribe(Arc<dyn stt::SttProvider>),
    Image(Arc<dyn imagegen::ImageGenProvider>),
    Decision(DecisionConfig),
}

pub(super) struct Target {
    pub(super) provider: String,
    pub(super) model: String,
    backend: Backend,
}

impl Target {
    pub(super) fn resolve(verb: Verb, config: &YuConfig) -> Result<Self, AiError> {
        let unconfigured = |slot: &str| {
            AiError::Provider(format!(
                "no usable {slot} model is configured; run `yu agent setup {slot}`"
            ))
        };
        let target = match verb {
            Verb::AI_EMBED => match config.embed.provider.as_str() {
                "" | "auto" | "local" | "qwen3-local" => Self {
                    provider: "qwen3-local".into(),
                    model: qwen3_genai::MODEL_NAME.into(),
                    backend: Backend::LocalEmbed(config.embed.clone()),
                },
                _ => {
                    let provider = embed::build_from_with_agent(&config.embed, &config.agent)
                        .ok()
                        .flatten()
                        .filter(|provider| provider.is_configured())
                        .ok_or_else(|| unconfigured("embed"))?;
                    Self {
                        provider: provider.name().into(),
                        model: provider.model().into(),
                        backend: Backend::Embed(provider),
                    }
                }
            },
            Verb::AI_AUDIO_TTS => {
                let alias = config.tts.provider.trim().to_ascii_lowercase();
                let provider = factory::tts_registry_from_cfg(config)
                    .get(match alias.as_str() {
                        "edge-tts" => "edge",
                        "gemini-tts" => "gemini",
                        name => name,
                    })
                    .filter(|provider| alias != "noop" && provider.is_configured())
                    .ok_or_else(|| unconfigured("tts"))?;
                Self {
                    provider: provider.name().into(),
                    model: config.tts.model.clone(),
                    backend: Backend::Speech(provider),
                }
            }
            Verb::AI_AUDIO_STT => {
                let alias = config.stt.provider.trim().to_ascii_lowercase();
                let provider = factory::stt_registry_from_cfg(config)
                    .get(&alias)
                    .filter(|provider| alias != "noop" && provider.is_configured())
                    .ok_or_else(|| unconfigured("stt"))?;
                Self {
                    provider: provider.name().into(),
                    model: config.stt.model.clone(),
                    backend: Backend::Transcribe(provider),
                }
            }
            Verb::AI_IMAGE_GENERATE => {
                let alias = config.imagegen.provider.trim().to_ascii_lowercase();
                let provider = factory::imagegen_registry_from_cfg(config)
                    .get(&alias)
                    .filter(|provider| alias != "noop" && provider.is_configured())
                    .ok_or_else(|| unconfigured("imagegen"))?;
                Self {
                    provider: provider.name().into(),
                    model: config.imagegen.model.clone(),
                    backend: Backend::Image(provider),
                }
            }
            Verb::AI_DECIDE => {
                decision::validate_model_alias(&config.decision.model)
                    .map_err(|_| unconfigured("decision"))?;
                Self {
                    provider: decision::LOCAL_PROVIDER.into(),
                    model: config.decision.model.clone(),
                    backend: Backend::Decision(config.decision.clone()),
                }
            }
            _ => return Err(unconfigured("inference")),
        };
        if target.model.trim().is_empty()
            || target.model == crate::caps::manifest::CONFIGURED_AI_MODEL
            || target.model.len() > 256
            || target.model.contains(['*', '?', '[', ']', '{', '}'])
            || target.model.chars().any(char::is_control)
        {
            return Err(AiError::Provider(
                "model configuration must identify an exact model".into(),
            ));
        }
        Ok(target)
    }

    pub(super) async fn run(
        &self,
        input: Prepared,
        owner: u32,
        home: &Path,
    ) -> Result<Value, &'static str> {
        match (&self.backend, input) {
            (Backend::Embed(provider), Prepared::Embed(text)) => {
                let response = provider
                    .embed(EmbedRequest { inputs: vec![text] })
                    .await
                    .map_err(|_| "embedding provider failed")?;
                embedding(response, &self.model)
            }
            (Backend::LocalEmbed(config), Prepared::Embed(text)) => {
                let response = local::run(
                    owner,
                    home,
                    &local::Task::Embed {
                        config: config.clone(),
                        text,
                    },
                )
                .await?;
                let response = serde_json::from_value(response)
                    .map_err(|_| "invalid local embedding reply")?;
                embedding(response, &self.model)
            }
            (Backend::Speech(provider), Prepared::Tts(text)) => {
                let response = provider
                    .synthesize(tts::TtsRequest::new(text))
                    .await
                    .map_err(|_| "speech provider failed")?;
                if response.format == tts::AudioFormat::Other {
                    return Err("unsupported speech output format");
                }
                let mut output = json!({
                    "kind": "tts", "format": response.format.extension(),
                    "audio_base64": media(&response.audio)?,
                });
                if let Some(rate) = response.sample_rate.filter(|rate| *rate > 0) {
                    output["sample_rate_hz"] = rate.into();
                }
                Ok(output)
            }
            (Backend::Transcribe(provider), Prepared::Stt(audio, format)) => {
                let response = provider
                    .transcribe(stt::SttRequest::new(audio, format))
                    .await
                    .map_err(|_| "transcription provider failed")?;
                Ok(json!({"kind": "stt", "text": response.text}))
            }
            (Backend::Image(provider), Prepared::Imagegen(prompt)) => {
                let response = provider
                    .generate(imagegen::ImageGenRequest::new(prompt))
                    .await
                    .map_err(|_| "image provider failed")?;
                let [image] = response.images.as_slice() else {
                    return Err("image provider did not return one image");
                };
                if image.format == imagegen::ImageFormat::Other {
                    return Err("unsupported image output format");
                }
                Ok(json!({
                    "kind": "imagegen", "format": image.format.extension(),
                    "image_base64": media(&image.bytes)?,
                }))
            }
            (Backend::Decision(config), Prepared::Decision(request)) => {
                let response = local::run(
                    owner,
                    home,
                    &local::Task::Decide {
                        config: config.clone(),
                        request,
                    },
                )
                .await?;
                let response: decision::DecisionResult =
                    serde_json::from_value(response).map_err(|_| "invalid local decision reply")?;
                if response.model != self.model {
                    return Err("decision reply did not match the authorized model");
                }
                Ok(json!({"kind": "decision", "answers": response.answers}))
            }
            _ => Err("inference input did not match the authorized model"),
        }
    }
}

fn embedding(response: EmbedResponse, model: &str) -> Result<Value, &'static str> {
    let [vector] = response.embeddings.as_slice() else {
        return Err("embedding provider did not return one vector");
    };
    if response.model != model
        || vector.is_empty()
        || vector.len() > 8192
        || vector.len() != response.dim
        || vector.iter().any(|value| !value.is_finite())
    {
        return Err("invalid vector or mismatched embedding model");
    }
    Ok(json!({"kind": "embed", "vector": vector}))
}

fn media(bytes: &[u8]) -> Result<String, &'static str> {
    if bytes.is_empty() || bytes.len() > MEDIA_OUTPUT_BYTES {
        return Err("media result is empty or exceeds the App reply limit");
    }
    Ok(STANDARD.encode(bytes))
}
