//! The App–AI Gate. Single entry point Apps use to reach a model.
//!
//! ```text
//!     yu ai chat --app …       yuos-sdk/python/src/yuos_sdk/ai.py
//!          │                          │
//!          ▼                          ▼
//!   ai::gate::chat_blocking ─── caps::require(ai.*, name(model))
//!          │                          │
//!          │                          ▼
//!          │                  manifest.ai origin allowlist
//!          │                          │   ▲
//!          │                          │   │ tighten only:
//!          │                          │   │   safety = stricter(M, U)
//!          │                          │   │   origins = M ∩ U
//!          │                          │   │
//!          │                          │   └─ user override
//!          │                          │      ($HOME/.config/yuos/apps/<id>.json)
//!          │                          │
//!          │                          ▼
//!          │                  user consent (snapshot of manifest AI block)
//!          │                          │   missing  → ConsentRequired
//!          │                          │   drifted  → ConsentStale
//!          │                          │   ($HOME/.config/yuos/consents/<id>.json)
//!          │                          ▼
//!          │                  OS-level model from agent.toml
//!          │                          │
//!          │                          ▼
//!          │                  budget::reserve (hard-deny overcap)
//!          │                          │   owner limit from
//!          │                          │   $HOME/.config/yuos/ai/budget.json
//!          │                          │   (0 = unlimited)
//!          │                          ▼
//!          │                  safety::redact (Strict / Standard)
//!          │                          │
//!          │                          ▼
//!          └──────►  llm::registry::build(provider).chat(ChatRequest)
//!                                       │
//!                                       ▼
//!                              budget::settle (actual usage)
//! ```
//!
//! Strict-mode is on by default for the OS; an app whose manifest has
//! no `ai` block can never reach a model — manifest validation refuses
//! to register such an app in the first place if it declared an
//! `ai.*` need. This file is the runtime defence on top of that:
//! even if a session somehow carries an AI verb, the gate still
//! re-checks the manifest's origin allowlist and uses the OS-owned
//! provider/model from `/etc/yuos/agent.toml`. **Apps never pick the
//! model** — the machine owner does, once, in agent config.
//!
//! On top of the manifest, the kernel reads a per-user override file
//! at `$HOME/.config/yuos/apps/<id>.json`. It can raise the safety
//! profile, shrink the origin allowlist, or disable an App. Existing
//! owner-set per-App limits are also preserved. See
//! [`crate::ai::overrides`] for the merge semantics.
//!
//! Above and orthogonal to overrides, the kernel requires a fresh
//! [`crate::ai::consent`] record — a JSON snapshot of the App's
//! manifest AI block that the user has explicitly approved (via
//! `yu app consent grant <id>` or the equivalent UI). A missing
//! record denies with `consent_required`; a drifted snapshot denies
//! with `consent_stale`. Consent tracks the manifest, not the
//! override, so a developer pushing a looser update always forces a
//! re-prompt regardless of the user's local tightening.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use async_trait::async_trait;
use futures_util::stream::BoxStream;

use crate::agent::llm::{
    self,
    run_log::{self, LlmRunRecord},
    types::{
        ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, ContentBlock, EngineInfo,
        FinishReason, Message, Role, StreamEvent, ToolChoice,
    },
    Provider as LlmProvider, Result as LlmResult,
};
use crate::agent::safety::redact::Redactor;
use crate::apps;
use crate::caps::manifest::{AiSafety, PromptOrigin};
use crate::caps::{self, Scope, Verb};
use crate::config;

use super::app_tools::AppTools;
use super::budget::BudgetError;
use super::consent;
use super::model_service;
use super::overrides;
pub use super::budget::SYSTEM_AGENT_BUCKET;
pub(super) const APP_INPUT: &str = "app";

// ---------------------------------------------------------------------------
// Public request / response shapes
// ---------------------------------------------------------------------------

/// One-shot AI request handed in by the CLI / `yuos_sdk`. The gate
/// auto-derives the [`Modality`] (and therefore the caps `Verb` to
/// require) from the **shape** of this request — callers never pass a
/// verb directly. See [`Modality::derive`] for the rules.
#[derive(Debug, Clone, Default)]
pub struct ChatRequest {
    /// App id this call is attributed to. The gate looks up the app's
    /// manifest to honour its `ai` policy (prompt origin allowlist,
    /// safety profile). The model is **not** under app
    /// control — the OS picks it from `/etc/yuos/agent.toml`.
    pub app_id: String,
    /// Where the prompt text originated — `"trusted"`, `"user-input"`,
    /// or `"external-content"`. `external-content` automatically
    /// hardens chat into [`Modality::ChatUntrusted`].
    pub origin: String,
    /// Text portion of the request. Required for chat / embed /
    /// image-generate / audio-tts / video-generate; optional for the
    /// "analyse this artefact" modalities.
    pub prompt: Option<String>,
    pub system: Option<String>,
    pub max_units: Option<u64>,

    // Modality selectors. The gate derives the verb from these — the
    // caller never supplies a verb directly.
    /// True when the caller wants a vector back instead of text.
    /// Mutually exclusive with the other modality selectors.
    pub embed: bool,
    pub image_input: Option<PathBuf>,
    pub image_output: Option<PathBuf>,
    pub audio_input: Option<PathBuf>,
    pub audio_output: Option<PathBuf>,
    pub video_input: Option<PathBuf>,
    pub video_output: Option<PathBuf>,

    /// Legacy input carried by older clients. The gate rejects nonempty lists.
    pub tools: Vec<String>,
    pub(crate) images: Vec<String>,
    pub(crate) app_tools: AppTools,
}

async fn send_chat_text(
    output: &tokio::sync::mpsc::Sender<String>,
    mut text: &str,
    total: usize,
) -> Result<(), String> {
    use crate::yuosd::ai_stream::{MAX_DELTA_BYTES, MAX_TEXT_BYTES};
    if total > MAX_TEXT_BYTES {
        return Err("App chat stream exceeded its text limit".into());
    }
    while !text.is_empty() {
        let mut end = text.len().min(MAX_DELTA_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        output.send(text[..end].to_owned()).await
            .map_err(|_| "App chat stream closed".to_string())?;
        text = &text[end..];
    }
    Ok(())
}

/// Structured envelope returned to apps. Always JSON-serialisable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    /// The textual result (for chat / image-analyze / audio-stt /
    /// vision-analyze / video-analyze) or an empty string (for
    /// modalities whose primary output is a file or vector).
    pub text: String,
    /// JSON vector for `ai.embed`. Empty for everything else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedding: Vec<f32>,
    /// Path the gate wrote the binary output to (image-generate,
    /// audio-tts, video-generate). `None` for text-only modalities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    pub model: String,
    pub provider: String,
    /// The caps verb the gate derived for this call. Surfaces in the
    /// JSON envelope so app developers can confirm the gate picked the
    /// modality they intended.
    pub verb: String,
    pub usage: Usage,
    pub budget: BudgetReport,
    pub review: ReviewReport,

    /// Validated proposals for this request's App-owned functions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ProposedToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_state: Option<String>,
}

/// A tool call proposal, not authority to execute it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedToolCall {
    /// Provider-issued unique id.
    pub id: String,
    /// Name of the function supplied by the App for this request.
    pub name: String,
    /// JSON arguments the model wants to pass.
    pub input: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Modality
// ---------------------------------------------------------------------------

/// What the gate is being asked to do. The CLI / `yuos_sdk` never names
/// this directly — the gate derives it from the request shape (which
/// of `prompt` / `image_input` / `audio_output` / … are set) and then
/// requires the corresponding caps verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modality {
    /// Prompt and optional images in, text and App function proposals out.
    Chat,
    /// Compatibility route for existing external-content grants.
    ChatUntrusted,
    /// Text in, vector out. The text is what the caller wants
    /// embedded; the response carries an `embedding` array.
    Embed,
    /// Text in, image file out.
    ImageGenerate,
    /// Image in, text out (no prompt).
    ImageAnalyze,
    /// Image in + text prompt in, text out.
    VisionAnalyze,
    /// Text in, audio file out.
    AudioTts,
    /// Audio in, text out.
    AudioStt,
    /// Text in, video file out.
    VideoGenerate,
    /// Video in, text out.
    VideoAnalyze,
}

impl Modality {
    /// Infer the modality from the request shape.
    ///
    /// Precedence (first match wins):
    ///
    ///   1. `embed=true`                            → [`Embed`]
    ///   2. `image_output.is_some()`                → [`ImageGenerate`]
    ///   3. `audio_output.is_some()`                → [`AudioTts`]
    ///   4. `video_output.is_some()`                → [`VideoGenerate`]
    ///   5. `image_input.is_some()` + prompt        → [`VisionAnalyze`]
    ///   6. `image_input.is_some()` (no prompt)     → [`ImageAnalyze`]
    ///   7. `audio_input.is_some()`                 → [`AudioStt`]
    ///   8. `video_input.is_some()`                 → [`VideoAnalyze`]
    ///   9. origin = `external-content`             → [`ChatUntrusted`]
    ///  10. otherwise                               → [`Chat`]
    ///
    /// Returns [`AiError::ModalityConflict`] if more than one modality
    /// selector is set (e.g. `image_input` + `audio_input` together).
    pub fn derive(req: &ChatRequest) -> Result<Modality, AiError> {
        // Reject obviously-incoherent combinations up front so apps
        // get a sharp error instead of silent wrong-verb selection.
        let selectors = [
            req.embed,
            req.image_input.is_some(),
            req.image_output.is_some(),
            req.audio_input.is_some(),
            req.audio_output.is_some(),
            req.video_input.is_some(),
            req.video_output.is_some(),
        ];
        let on = selectors.iter().filter(|b| **b).count();
        if on > 1 {
            return Err(AiError::ModalityConflict(
                "multiple modality selectors set; pass at most one of \
                 --embed / --image-input / --image-output / \
                 --audio-input / --audio-output / \
                 --video-input / --video-output"
                    .to_string(),
            ));
        }

        if req.embed {
            return Ok(Modality::Embed);
        }
        if req.image_output.is_some() {
            return Ok(Modality::ImageGenerate);
        }
        if req.audio_output.is_some() {
            return Ok(Modality::AudioTts);
        }
        if req.video_output.is_some() {
            return Ok(Modality::VideoGenerate);
        }
        if req.image_input.is_some() {
            return Ok(if has_prompt(req) {
                Modality::VisionAnalyze
            } else {
                Modality::ImageAnalyze
            });
        }
        if req.audio_input.is_some() {
            return Ok(Modality::AudioStt);
        }
        if req.video_input.is_some() {
            return Ok(Modality::VideoAnalyze);
        }

        if req.origin == APP_INPUT {
            return Ok(Modality::Chat);
        }
        let origin = parse_origin(&req.origin)?;
        Ok(match origin {
            PromptOrigin::ExternalContent => Modality::ChatUntrusted,
            _ => Modality::Chat,
        })
    }

    /// Caps verb required for this modality.
    pub fn verb(self) -> Verb {
        match self {
            Modality::Chat => Verb::AI_CHAT,
            Modality::ChatUntrusted => Verb::AI_CHAT_UNTRUSTED,
            Modality::Embed => Verb::AI_EMBED,
            Modality::ImageGenerate => Verb::AI_IMAGE_GENERATE,
            Modality::ImageAnalyze => Verb::AI_IMAGE_ANALYZE,
            Modality::VisionAnalyze => Verb::AI_VISION_ANALYZE,
            Modality::AudioTts => Verb::AI_AUDIO_TTS,
            Modality::AudioStt => Verb::AI_AUDIO_STT,
            Modality::VideoGenerate => Verb::AI_VIDEO_GENERATE,
            Modality::VideoAnalyze => Verb::AI_VIDEO_ANALYZE,
        }
    }

    /// Lower-snake-case label, used for audit and error messages.
    pub fn label(self) -> &'static str {
        match self {
            Modality::Chat => "chat",
            Modality::ChatUntrusted => "chat_untrusted",
            Modality::Embed => "embed",
            Modality::ImageGenerate => "image_generate",
            Modality::ImageAnalyze => "image_analyze",
            Modality::VisionAnalyze => "vision_analyze",
            Modality::AudioTts => "audio_tts",
            Modality::AudioStt => "audio_stt",
            Modality::VideoGenerate => "video_generate",
            Modality::VideoAnalyze => "video_analyze",
        }
    }

    /// True for the stable App-facing modalities. Other selectors are
    /// rejected before app lookup, consent, capability, or budget work.
    fn is_chat_like(self) -> bool {
        matches!(self, Modality::Chat | Modality::ChatUntrusted)
    }
}

fn has_prompt(req: &ChatRequest) -> bool {
    req.prompt
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub units: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    pub period: String,
    pub units_used: u64,
    pub units_cap: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    /// Safety profile actually applied to this call.
    pub safety: String,
    /// True if the prompt was modified by the redactor before being
    /// sent upstream. The redacted text itself is not echoed back.
    pub prompt_redacted: bool,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("{0}")]
    InvalidInference(&'static str),
    #[error("unknown app `{0}` — not present in YUOS_APPS_DIR")]
    UnknownApp(String),

    #[error("app `{app}` has no `ai` block in its manifest — AI is disabled for it")]
    NoAiPolicy { app: String },

    #[error(
        "app `{0}` is disabled by the user (set `disabled: true` in \
         $HOME/.config/yuos/apps/{0}.json)"
    )]
    AppDisabled(String),

    #[error("user override for app `{app}` is malformed: {detail}")]
    BadOverride { app: String, detail: String },

    #[error(
        "app `{app}` has not been approved yet — run `yu app consent grant {app}` \
         to review its AI policy and approve"
    )]
    ConsentRequired { app: String },

    #[error("AI permission {verb} for app `{app}` is off; allow it in Settings → Applications")]
    ModelSettingsRequired { app: String, verb: crate::caps::Verb },

    #[error(
        "consent for app `{app}` is stale (changed: {changed:?}) — \
         run `yu app consent grant {app}` to review the new AI policy and re-approve"
    )]
    ConsentStale { app: String, changed: Vec<String> },

    #[error("stored consent for app `{app}` is malformed: {detail}")]
    BadConsent { app: String, detail: String },

    #[error("invalid prompt origin `{0}` — try trusted, user-input, external-content")]
    BadOrigin(String),

    #[error(
        "origin `{got}` is not in the app's declared origins; \
         allowed: {allowed:?}"
    )]
    OriginNotAllowed { got: String, allowed: Vec<String> },

    #[error("invalid request: {0}")]
    ModalityConflict(String),

    #[error("modality `{0}` is not yet wired to a provider — gate is ready, but no installed model supports it")]
    ModalityNotSupported(&'static str),

    #[error("App model calls do not support tools by catalog name; supply App-owned functions in a chat request")]
    ToolsNotSupported,

    #[error("invalid App tool turn: {0}")]
    InvalidToolTurn(&'static str),
    #[error("missing required input for `{modality}`: {field}")]
    MissingInput {
        modality: &'static str,
        field: &'static str,
    },

    #[error("capability denied: {0}")]
    Denied(serde_json::Value),

    #[error("{0}")]
    Budget(#[from] BudgetError),

    #[error("The owner's App AI budget cannot cover this request. See Settings > AI Budget.")]
    UserBudgetExceeded,

    #[error(
        "request unit limit exceeded: conservative maximum is {required} units, \
         but --max-units is {limit}"
    )]
    RequestBudgetExceeded { required: u64, limit: u64 },

    #[error("provider error: {0}")]
    Provider(String),

    #[error("safety violation: {0}")]
    Safety(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl AiError {
    pub fn wire_code(&self) -> &'static str {
        match self {
            AiError::UnknownApp(_) => "UNKNOWN_APP",
            AiError::BadOrigin(_)
            | AiError::ModalityConflict(_)
            | AiError::ToolsNotSupported
            | AiError::InvalidToolTurn(_)
            | AiError::MissingInput { .. }
            | AiError::InvalidInference(_) => "INVALID_ARGS",
            AiError::NoAiPolicy { .. }
            | AiError::AppDisabled(_)
            | AiError::ConsentRequired { .. }
            | AiError::ModelSettingsRequired { .. }
            | AiError::ConsentStale { .. }
            | AiError::OriginNotAllowed { .. }
            | AiError::Denied(_) => "PERMISSION_DENIED",
            AiError::Budget(BudgetError::OverUnitCap { .. })
            | AiError::UserBudgetExceeded
            | AiError::RequestBudgetExceeded { .. } => "BUDGET_EXCEEDED",
            AiError::Safety(_) => "SAFETY_VIOLATION",
            AiError::ModalityNotSupported(_) => "UNKNOWN_VERB",
            AiError::Provider(_) => "KERNEL_UNAVAILABLE",
            AiError::BadOverride { .. }
            | AiError::BadConsent { .. }
            | AiError::Budget(_)
            | AiError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Blocking entry point. Wraps the async path in a fresh runtime — the
/// CLI is naturally synchronous and we don't want every app to know
/// about tokio.
pub fn chat_blocking(req: ChatRequest) -> Result<ChatResult, AiError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AiError::Internal(e.to_string()))?;
    rt.block_on(chat(req))
}

/// Async entry point. Performs the full gate sequence and emits a
/// per-call record to `<log_dir>/ai.jsonl` for **every** outcome —
/// allowed (status=ok or status=error after the provider runs) and
/// denied (status=denied with a stable `denial_reason` token).
pub async fn chat(req: ChatRequest) -> Result<ChatResult, AiError> {
    chat_with_authority(req, None, None).await
}

pub(crate) async fn chat_authorized(
    req: ChatRequest,
    authority: &crate::yuosd::authority::Decision,
    output: Option<&tokio::sync::mpsc::Sender<String>>,
) -> Result<ChatResult, AiError> {
    authority
        .require_app(&req.app_id)
        .map_err(|error| AiError::Denied(serde_json::json!({"error": error})))?;
    chat_with_authority(req, Some(authority), output).await
}

async fn chat_with_authority(
    req: ChatRequest,
    authority: Option<&crate::yuosd::authority::Decision>,
    output: Option<&tokio::sync::mpsc::Sender<String>>,
) -> Result<ChatResult, AiError> {
    let started = std::time::Instant::now();
    // Best-effort verb derivation up-front so audit records carry it
    // even on denial paths. `Modality::derive` runs again inside
    // `chat_inner`; the duplication is cheap and lets the audit
    // attribute denials caused by *non-derivation* errors to the
    // correct verb.
    let verb_label: Option<String> = Modality::derive(&req)
        .ok()
        .map(|m| m.verb().as_str().to_string());
    let result = chat_inner(&req, authority, output).await;
    let duration_ms = started.elapsed().as_millis() as u64;

    match &result {
        Ok(ok) => {
            let mut rec = LlmRunRecord::from_success(
                &ok.provider,
                &ok.model,
                None,
                FinishReason::Stop,
                &llm::types::Usage {
                    input_tokens: ok.usage.input_tokens,
                    output_tokens: ok.usage.output_tokens,
                    ..Default::default()
                },
                duration_ms,
                authority.and_then(|a| a.session_id()),
            )
            .with_app(&req.app_id);
            if !ok.verb.is_empty() {
                rec = rec.with_verb(&ok.verb);
            }
            run_log::record(&rec);
        }
        Err(err) => {
            let mut rec = LlmRunRecord::from_denial(
                &req.app_id,
                &config::current_snapshot().agent.model,
                denial_reason_token(err),
                &err.to_string(),
                duration_ms,
                authority.and_then(|a| a.session_id()),
            );
            if let Some(v) = &verb_label {
                rec = rec.with_verb(v);
            }
            run_log::record(&rec);
        }
    }

    result
}

/// Stable, lower-cased machine token classifying a gate denial. Kept
/// in lockstep with the [`AiError`] variants — operators and tests
/// rely on the exact spellings, so don't rename without updating the
/// grep'able doc on [`LlmRunRecord::denial_reason`].
pub(crate) fn denial_reason_token(err: &AiError) -> &'static str {
    match err {
        AiError::UnknownApp(_) => "unknown_app",
        AiError::NoAiPolicy { .. } => "no_ai_policy",
        AiError::AppDisabled(_) => "app_disabled",
        AiError::BadOverride { .. } => "bad_override",
        AiError::ConsentRequired { .. } => "consent_required",
        AiError::ModelSettingsRequired { .. } => "model_permission_required",
        AiError::ConsentStale { .. } => "consent_stale",
        AiError::BadConsent { .. } => "bad_consent",
        AiError::BadOrigin(_) => "bad_origin",
        AiError::OriginNotAllowed { .. } => "origin_not_allowed",
        AiError::ModalityConflict(_) => "modality_conflict",
        AiError::ModalityNotSupported(_) => "modality_not_supported",
        AiError::ToolsNotSupported => "tools_not_supported",
        AiError::InvalidToolTurn(_) => "invalid_tool_turn",
        AiError::MissingInput { .. } => "missing_input",
        AiError::InvalidInference(_) => "invalid_args",
        AiError::Denied(_) => "caps_denied",
        AiError::Budget(_) => "budget_exceeded",
        AiError::UserBudgetExceeded => "user_budget_exceeded",
        AiError::RequestBudgetExceeded { .. } => "request_budget_exceeded",
        AiError::Provider(_) => "provider_error",
        AiError::Safety(_) => "safety_block",
        AiError::Internal(_) => "internal",
    }
}

pub(super) fn app_policy(
    app_id: &str,
    origin: &str,
    verb: crate::caps::Verb,
    authority: Option<&crate::yuosd::authority::Decision>,
) -> Result<(crate::caps::manifest::AiPolicy, u64), AiError> {
    // 1. Locate the app and its AI policy.
    let app = if authority.is_some() {
        // A long-lived broker must reverify, not keep a stale cached policy
        // after package replacement or publisher revocation.
        let dir = std::env::var_os("YUOS_APPS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/usr/lib/yuos/apps"));
        apps::discover_verified(&dir)
            .remove(app_id)
            .ok_or_else(|| AiError::UnknownApp(app_id.to_owned()))?
    } else {
        lookup_app(app_id)?
    };
    let manifest_policy = app
        .manifest
        .ai
        .as_ref()
        .ok_or_else(|| AiError::NoAiPolicy {
            app: app_id.to_owned(),
        })?
        .clone();

    // 1a. Layer the per-user override on top. The user can only
    //     tighten — never loosen — the manifest. A missing override
    //     file is normal; a malformed file aborts the call so the
    //     user notices the problem.
    let user_override = overrides::load(app_id).map_err(|detail| AiError::BadOverride {
        app: app_id.to_owned(),
        detail,
    })?;
    if let Some(o) = &user_override {
        if o.disabled {
            return Err(AiError::AppDisabled(app_id.to_owned()));
        }
    }
    let policy = overrides::apply_to_policy(&manifest_policy, user_override.as_ref());

    // 1b. Require fresh user consent. The user must have explicitly
    //     approved the App's AI ask — consent tracks the **manifest**
    //     policy (not the override), so a developer pushing a looser
    //     manifest update forces a re-prompt even if the user's
    //     override keeps the effective policy tight.
    let bad_consent = |detail| AiError::BadConsent { app: app_id.to_owned(), detail };
    let consent_required = || AiError::ConsentRequired { app: app_id.to_owned() };
    let optional_gui = authority.filter(|authority| authority.is_gui_session()).filter(|authority| {
        let named = authority.session().ok()
            .and_then(|session| session.command.first())
            .and_then(|command| command.strip_prefix("--gui:"));
        app.manifest.gui_entry(named).is_ok_and(|entry| entry.optional_needs.iter().any(|need| {
            need.verb == verb && crate::caps::manifest::requests_configured_ai(&need.scope)
        }))
    });
    if let Some(authority) = optional_gui {
        let consent_required = || AiError::ModelSettingsRequired { app: app_id.to_owned(), verb };
        let package = app.require_verified().map_err(bad_consent)?;
        let running = crate::provenance::runtime::package_for(
        authority.owner_uid(), authority.session_id().ok_or_else(consent_required)?,
        ).map_err(bad_consent)?;
        if running.as_ref() != Some(&crate::provenance::runtime::PackageRef::of(package)) {
        return Err(AiError::Denied(serde_json::json!({"error": "App package changed; restart it"})));
        }
        let choices = consent::gui_ai_choices(
        authority.owner_uid(), package, &app.manifest, &config::current_snapshot(),
        ).map_err(bad_consent)?;
        let choice = choices.iter().find(|choice| choice.cap.verb == verb)
        .ok_or_else(|| AiError::Provider("No usable model is available for this App permission".into()))?;
        if !choice.enabled || !authority.caps().covers(&choice.cap) {
        return Err(consent_required());
        }
    } else {
        let stored_consent = consent::load(app_id).map_err(bad_consent)?
            .ok_or_else(consent_required)?;
        if let consent::Freshness::Stale { changed } =
            consent::freshness(&manifest_policy, &stored_consent)
        {
            return Err(AiError::ConsentStale {
                app: app_id.to_owned(),
                changed,
            });
        }
    }

    let allowed = if origin == APP_INPUT {
        // Do not promote a legacy external-content-only policy to ai.chat.
        policy.origins.iter().any(|origin| {
            *origin != PromptOrigin::ExternalContent || verb != Verb::AI_CHAT
        })
    } else {
        policy.origins.contains(&parse_origin(origin)?)
    };
    if !allowed {
        return Err(AiError::OriginNotAllowed {
            got: origin.to_owned(),
            allowed: policy.origins.iter().map(origin_label).collect(),
        });
    }
    Ok((policy, overrides::monthly_limit(user_override.as_ref())))
}

async fn chat_inner(
    req: &ChatRequest,
    authority: Option<&crate::yuosd::authority::Decision>,
    output: Option<&tokio::sync::mpsc::Sender<String>>,
) -> Result<ChatResult, AiError> {
    let modality = Modality::derive(req)?;
    if !modality.is_chat_like() {
        return Err(AiError::ModalityNotSupported(modality.label()));
    }
    let verb = modality.verb();
    let config = config::current_snapshot();
    let cfg = &config.agent;
    let images = chat_images(&req.images, req.origin == APP_INPUT || modality == Modality::ChatUntrusted);
    if cfg.model == crate::caps::manifest::CONFIGURED_AI_MODEL {
        return Err(AiError::Provider("Configure a chat model in Yu OS settings".into()));
    }
    let model = cfg.model.clone();
    let authorization = authority.map(|authority| {
        authority.require(crate::caps::Cap::new(verb, Scope::name(&model)))
    });
    let (policy, app_cap) = app_policy(&req.app_id, &req.origin, verb, authority)?;
    // Spend the exact model capability before releasing input diagnostics.
    if let Some(authorization) = authorization {
        let _authorized = authorization
            .map_err(|error| AiError::Denied(serde_json::json!({"error": error})))?;
    } else {
        caps::require(verb, Scope::name(&model)).map_err(|d| AiError::Denied(d.to_json()))?;
    }
    validate_inputs(req, modality)?;
    let images = images?;
    if !images.is_empty() && (cfg.provider == "llama_local"
        || llm::metadata::lookup(&model).is_some_and(|model| !model.supports_vision))
    {
        return Err(AiError::Provider("Choose a chat model that accepts images in Yu OS settings".into()));
    }

    // 7. Apply safety pipeline to the prompt (when present).
    let (prompt_for_provider, mut prompt_redacted) = match req.prompt.as_deref() {
        Some(p) if !p.is_empty() => {
            let (out, changed) = apply_safety(p, policy.safety);
            (Some(out), changed)
        }
        _ => (None, false),
    };

    let prompt = prompt_for_provider.ok_or(AiError::MissingInput {
        modality: modality.label(),
        field: "prompt",
    })?;

    // 8. Build the exact provider request, then reserve its conservative
    // maximum (serialized input bytes + provider output limit). Reserving
    // the maximum rather than a 4-chars/token guess prevents concurrent
    // calls from collectively overselling the monthly cap.
    let mut llm_req = build_chat_request(
        &model,
        &prompt,
        req.system.as_deref(),
        DEFAULT_APP_MAX_OUTPUT_TOKENS,
    );
    llm_req.messages[0].content.extend(images);
    llm_req.tools = req.app_tools.project(
        &cfg.provider, &model, policy.safety, &mut llm_req.messages, &mut prompt_redacted,
    )?;
    if !llm_req.tools.is_empty() {
        llm_req.tool_choice = ToolChoice::Auto;
    }
    let input_units = model_service::estimate_chat_input(&llm_req);
    if let Some(limit) = req.max_units {
        let minimum = input_units.saturating_add(1);
        let available_output = limit.checked_sub(input_units).filter(|units| *units > 0);
        let Some(available_output) = available_output else {
            return Err(AiError::RequestBudgetExceeded {
                required: minimum,
                limit,
            });
        };
        llm_req.max_tokens =
            Some(available_output.min(DEFAULT_APP_MAX_OUTPUT_TOKENS as u64) as u32);
    }
    let estimated_units = model_service::estimate_chat_request(&llm_req);
    if let Some(limit) = req.max_units {
        if estimated_units > limit {
            return Err(AiError::RequestBudgetExceeded {
                required: estimated_units,
                limit,
            });
        }
    }

    let provider = model_service::build_chat(&cfg.provider, &model, cfg, None)
        .map_err(|error| AiError::Provider(app_provider_error(error)))?;
    let call = model_service::begin_app_call(
        authority,
        &req.app_id,
        &req.origin,
        app_cap,
        estimated_units,
        &llm_req,
    )?;
    let (llm_resp, actual_units, snapshot) = call.run(
        async {
            match output {
                Some(output) => stream_chat(provider.as_ref(), llm_req, output).await,
                None => provider.chat(llm_req).await.map_err(app_provider_error),
            }
        },
        |response| {
            let reported = model_service::chat_units(&response.usage);
            if output.is_some() && response.usage.input_tokens == 0 {
                // Some streaming providers omit usage. Do not refund the
                // reservation simply because the transport changed.
                estimated_units.max(reported)
            } else {
                reported
            }
        },
    ).await?;
    if let Some(limit) = req.max_units {
        if actual_units > limit {
            return Err(AiError::RequestBudgetExceeded {
                required: actual_units,
                limit,
            });
        }
    }

    async fn stream_chat(
        provider: &dyn LlmProvider,
        request: llm::types::ChatRequest,
        output: &tokio::sync::mpsc::Sender<String>,
    ) -> Result<llm::types::ChatResponse, String> {
        use futures_util::StreamExt;

        tokio::select! {
            biased;
            _ = output.closed() => Err("App chat stream closed".into()),
            result = async {
                let mut accumulator = llm::accumulate::Accumulator::new(&request.model);
                let mut events = provider.chat_stream(request).await.map_err(app_provider_error)?;
                let mut total = 0usize;
                while let Some(event) = events.next().await {
                    let event = event.map_err(app_provider_error)?;
                    if let StreamEvent::TextDelta { text } = &event {
                        total = total.saturating_add(text.len());
                        send_chat_text(output, text, total).await?;
                    }
                    if accumulator.feed(event) {
                        let reply = accumulator.finish();
                        // Some providers only supply a complete Message, not text deltas.
                        if total == 0 {
                            let text = chat_text(&reply);
                            send_chat_text(output, &text, text.len()).await?;
                        }
                        return Ok(reply);
                    }
                }
                Err("configured chat model did not finish its stream".into())
            } => result,
        }
    }

    let text = chat_text(&llm_resp);
    let input_tokens = llm_resp.usage.input_tokens;
    let output_tokens = llm_resp.usage.output_tokens;
    let (tool_calls, provider_state) = req.app_tools.proposals(&cfg.provider, &model, llm_resp)?;

    Ok(ChatResult {
        text,
        embedding: Vec::new(),
        output_path: None,
        model,
        provider: provider.name().to_string(),
        verb: verb.as_str().to_string(),
        usage: Usage {
            input_tokens,
            output_tokens,
            units: actual_units,
        },
        budget: BudgetReport {
            period: snapshot.period,
            units_used: snapshot.units_used,
            units_cap: app_cap,
        },
        review: ReviewReport {
            safety: safety_label(policy.safety),
            prompt_redacted,
        },
        tool_calls,
        provider_state,
    })
}

fn chat_text(response: &llm::types::ChatResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn app_provider_error(error: llm::LlmError) -> String {
    // Raw upstream errors can echo the private endpoint or credentials.
    match error {
        llm::LlmError::NotConfigured(_) => "configure a chat model in Yu OS settings".into(),
        llm::LlmError::Auth | llm::LlmError::CredentialStore { .. } => {
            "chat model authentication failed; check Yu OS settings".into()
        }
        llm::LlmError::Provider { status, .. } => format!("chat model API returned HTTP {status}"),
        llm::LlmError::RateLimited { .. } => "chat model is rate limited".into(),
        _ => "configured chat model request failed; check Yu OS settings".into(),
    }
}

/// Per-modality required-input check. Runs after origin validation so
/// the "missing prompt" error doesn't mask an earlier policy denial.
fn validate_inputs(req: &ChatRequest, m: Modality) -> Result<(), AiError> {
    if !req.tools.is_empty() {
        return Err(AiError::ToolsNotSupported);
    }
    use Modality::*;
    let label = m.label();
    let need_prompt = matches!(
        m,
        Chat | ChatUntrusted | Embed | ImageGenerate | AudioTts | VideoGenerate
    );
    if need_prompt && !has_prompt(req) {
        return Err(AiError::MissingInput {
            modality: label,
            field: "prompt",
        });
    }
    let need_input_file = match m {
        ImageAnalyze | VisionAnalyze => req.image_input.is_none(),
        AudioStt => req.audio_input.is_none(),
        VideoAnalyze => req.video_input.is_none(),
        _ => false,
    };
    if need_input_file {
        return Err(AiError::MissingInput {
            modality: label,
            field: "input_file",
        });
    }
    let need_output_file = match m {
        ImageGenerate => req.image_output.is_none(),
        AudioTts => req.audio_output.is_none(),
        VideoGenerate => req.video_output.is_none(),
        _ => false,
    };
    if need_output_file {
        return Err(AiError::MissingInput {
            modality: label,
            field: "output_file",
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Cached app lookup. `apps::discover` walks the apps directory and
/// re-parses every manifest on every call — that's fine for a CLI
/// invocation but pathological inside the gate, which is on the hot
/// path of every AI request. We cache the discovered map for up to
/// 60 seconds; this is short enough that `yuos apps add` is visible
/// within an interactive session and long enough to avoid re-walking
/// the disk on tight request loops.
///
/// `YUOS_APPS_DIR` is read as part of the cache key so test setups
/// that flip the env between cases (and `setup`/`teardown` blocks)
/// still see fresh results when they override the dir.
fn lookup_app(app_id: &str) -> Result<apps::App, AiError> {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    struct Entry {
        dir: std::path::PathBuf,
        map: BTreeMap<String, apps::App>,
        fetched_at: Instant,
    }
    static CACHE: OnceLock<Mutex<Option<Entry>>> = OnceLock::new();
    const TTL: Duration = Duration::from_secs(60);

    let apps_dir = std::env::var("YUOS_APPS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/usr/lib/yuos/apps"));

    let cache = CACHE.get_or_init(|| Mutex::new(None));
    // Fast path: read the cache and clone the requested app out.
    if let Ok(guard) = cache.lock() {
        if let Some(entry) = guard.as_ref() {
            if entry.dir == apps_dir && entry.fetched_at.elapsed() < TTL {
                return entry
                    .map
                    .get(app_id)
                    .cloned()
                    .ok_or_else(|| AiError::UnknownApp(app_id.to_string()));
            }
        }
    }

    // Slow path: rebuild the cache. We re-acquire the lock just
    // long enough to swap the new entry in; concurrent calls may
    // each re-discover once, which is benign (the second result
    // overwrites the first with identical data).
    let discovered = apps::discover_verified(&apps_dir);
    let result = discovered
        .get(app_id)
        .cloned()
        .ok_or_else(|| AiError::UnknownApp(app_id.to_string()));
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(Entry {
            dir: apps_dir,
            map: discovered,
            fetched_at: Instant::now(),
        });
    }
    result
}

fn parse_origin(s: &str) -> Result<PromptOrigin, AiError> {
    match s {
        "trusted" => Ok(PromptOrigin::Trusted),
        "user-input" | "user_input" => Ok(PromptOrigin::UserInput),
        "external-content" | "external_content" => Ok(PromptOrigin::ExternalContent),
        other => Err(AiError::BadOrigin(other.to_string())),
    }
}

fn origin_label(o: &PromptOrigin) -> String {
    match o {
        PromptOrigin::Trusted => "trusted".into(),
        PromptOrigin::UserInput => "user-input".into(),
        PromptOrigin::ExternalContent => "external-content".into(),
    }
}

fn safety_label(s: AiSafety) -> String {
    match s {
        AiSafety::Strict => "strict".into(),
        AiSafety::Standard => "standard".into(),
        AiSafety::Minimal => "minimal".into(),
    }
}

pub(super) fn apply_safety(prompt: &str, safety: AiSafety) -> (String, bool) {
    match safety {
        AiSafety::Minimal => (prompt.to_string(), false),
        AiSafety::Standard | AiSafety::Strict => {
            let redactor = Redactor::default_set();
            let redacted = redactor.redact(prompt);
            let changed = redacted != prompt;
            (redacted, changed)
        }
    }
}

fn chat_images(
    images: &[String],
    allowed: bool,
) -> Result<Vec<ContentBlock>, AiError> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    if !images.is_empty() && !allowed {
        return Err(AiError::InvalidInference("images require the App chat request format"));
    }
    if images.len() > 4 {
        return Err(AiError::InvalidInference("chat accepts up to four images"));
    }
    let mut total = 0;
    images.iter().map(|image| {
        let bytes = STANDARD.decode(image)
            .map_err(|_| AiError::InvalidInference("image must be standard padded base64"))?;
        total += bytes.len();
        if total > 512 * 1024 {
            return Err(AiError::InvalidInference("chat images exceed 512 KiB combined"));
        }
        let media_type = crate::agent::attachments::detect_media_type(
            &bytes,
            &["image/png", "image/jpeg", "image/webp"],
        )
        .ok_or(AiError::InvalidInference("unsupported image format; send PNG, JPEG or WebP"))?;
        Ok(ContentBlock::Image {
            media_type: media_type.into(),
            data: image.clone(),
        })
    }).collect()
}

fn build_chat_request(
    model: &str,
    user: &str,
    system: Option<&str>,
    max_tokens: u32,
) -> LlmChatRequest {
    LlmChatRequest {
        model: model.to_string(),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: user.to_string(),
            }],
        }],
        system: system.map(|s| s.to_string()),
        tools: Vec::new(),
        tool_choice: ToolChoice::None,
        max_tokens: Some(max_tokens),
        temperature: None,
        top_p: None,
        stop_sequences: Vec::new(),
        extra: serde_json::json!({"__yuos_initiator": "agent"}),
    }
}

// Silence "unused" warnings when this Arc only flows through the
// async pathway above.
#[allow(dead_code)]
fn _ensure_arc_send<T: ?Sized + Send + Sync>(_: &Arc<T>) {}

// ---------------------------------------------------------------------------
// System-agent provider wrapper.
//
// The kernel-resident agent (everything reachable via `yu agent ask`,
// `yu agent chat`, the yuos-agent-bridge HTTP service, the doctor,
// the vision/author/delegate paths, …) is NOT an installed app — it
// has no manifest, no `ai` block, no per-app allowlist. But it still
// spends real tokens against real providers, so it deserves the same
// kernel-level oversight that real apps get:
//
//   * `caps::require(Verb::AI_CHAT, name(model))` — defence-in-depth
//     check AND structured audit emission to `caps.jsonl`.
//   * `budget::reserve/settle` against the stable pseudo-app id
//     `system.agent`, capped by `agent.system_budget_monthly_units`
//     in `~/.config/yuos/config.json`.
//
// Implemented as a Provider decorator so call sites need only wrap
// the result of `llm::registry::build(...)`. The wrapper transparently
// forwards every Provider method to the inner; only `chat()` and
// `chat_stream()` get the additional caps/budget pipeline.
// ---------------------------------------------------------------------------

/// Wrap an LLM provider so the kernel-resident agent reaches the
/// upstream model through the same caps + budget + audit pipeline
/// real apps use. The wrapper is cheap (single `Arc` indirection)
/// and is the only sanctioned way for `core::agent::*` code to talk
/// to a model — direct `llm::registry::build(...)` consumption
/// should always be followed by this call.
pub fn wrap_for_system(inner: Arc<dyn LlmProvider>) -> Arc<dyn LlmProvider> {
    Arc::new(SystemGatedProvider { inner })
}

pub fn build_system_provider(
    cfg: &crate::config::AgentConfig,
) -> llm::Result<Arc<dyn LlmProvider>> {
    build_system_provider_with_observer(
        cfg,
        Arc::new(crate::agent::llm::attempt_observer::NoopProviderAttemptObserver),
    )
}

pub fn build_system_provider_with_observer(
    cfg: &crate::config::AgentConfig,
    extension_observer: Arc<dyn llm::attempt_observer::ProviderAttemptObserver>,
) -> llm::Result<Arc<dyn LlmProvider>> {
    use crate::agent::llm::attempt_observer::{
        AuditProviderAttemptObserver, CompositeProviderAttemptObserver, RequestMetadata,
    };

    // Provider construction is where the model transport (HTTP client,
    // streaming parser, credential resolution) enters a process. It is
    // reachable only from the unprivileged agent worker.
    crate::agentd::guard::ensure_agent_runtime_allowed("model provider construction")
        .map_err(llm::LlmError::NotConfigured)?;

    let context = llm::construction::ProviderBuildContext::from_process()?;
    let observer = Arc::new(CompositeProviderAttemptObserver::new(vec![
        Arc::new(AuditProviderAttemptObserver::new(
            crate::paths::agent_audit_log_path(),
            RequestMetadata::from_process(),
        )),
        extension_observer,
    ]));
    build_system_provider_with_context(cfg, &context, observer)
}

pub fn build_system_provider_with_context(
    cfg: &crate::config::AgentConfig,
    context: &llm::construction::ProviderBuildContext,
    observer: Arc<dyn llm::attempt_observer::ProviderAttemptObserver>,
) -> llm::Result<Arc<dyn LlmProvider>> {
    use crate::agent::llm::provider_chain::{ProviderChain, ProviderSlot};
    use std::collections::BTreeSet;

    if cfg.provider_fallbacks.len() > 8 {
        return Err(llm::LlmError::NotConfigured(format!(
            "provider fallback chain has {} fallbacks; maximum is 8",
            cfg.provider_fallbacks.len()
        )));
    }
    let primary = model_service::build_chat(&cfg.provider, &cfg.model, cfg, Some(context))?;
    let primary = Arc::new(llm::attempt_observer::ObservedProvider::new(
        primary,
        cfg.provider.clone(),
        observer.clone(),
    ));
    let mut slots = vec![ProviderSlot::new(
        wrap_for_system(primary),
        cfg.provider.clone(),
        cfg.model.clone(),
    )];
    let mut identities = BTreeSet::from([(cfg.provider.clone(), cfg.model.clone())]);
    for fallback in &cfg.provider_fallbacks {
        let provider = fallback.provider.trim();
        let model = fallback.model.trim();
        if provider.is_empty() || model.is_empty() {
            return Err(llm::LlmError::NotConfigured(
                "provider fallback entries require non-empty provider and model".to_string(),
            ));
        }
        if provider == "mock" && cfg.provider != "mock" {
            return Err(llm::LlmError::NotConfigured(
                "mock cannot be configured as a production provider fallback".to_string(),
            ));
        }
        if !identities.insert((provider.to_string(), model.to_string())) {
            return Err(llm::LlmError::NotConfigured(format!(
                "duplicate provider fallback: {provider}/{model}"
            )));
        }
        let fallback_cfg = fallback.apply_to(cfg);
        let built = model_service::build_chat(provider, model, &fallback_cfg, Some(context))?;
        let built = Arc::new(llm::attempt_observer::ObservedProvider::new(
            built,
            provider.to_string(),
            observer.clone(),
        ));
        slots.push(ProviderSlot::new(
            wrap_for_system(built),
            provider.to_string(),
            model.to_string(),
        ));
    }
    if slots.len() == 1 {
        Ok(slots.remove(0).provider)
    } else {
        Ok(Arc::new(ProviderChain::new_with_observer(slots, observer)?))
    }
}

struct SystemGatedProvider {
    inner: Arc<dyn LlmProvider>,
}

#[async_trait]
impl LlmProvider for SystemGatedProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn supported_models(&self) -> Vec<String> {
        self.inner.supported_models()
    }

    fn is_configured(&self) -> bool {
        self.inner.is_configured()
    }

    fn engine_info(&self) -> Option<EngineInfo> {
        self.inner.engine_info()
    }

    fn supports_prompt_cache(&self) -> bool {
        self.inner.supports_prompt_cache()
    }

    fn effective_provider_name(&self) -> String {
        self.inner.effective_provider_name()
    }

    fn effective_model_name(&self, requested: &str) -> String {
        self.inner.effective_model_name(requested)
    }

    fn fallback_state(&self) -> Option<llm::ProviderFallbackState> {
        self.inner.fallback_state()
    }

    async fn chat(&self, request: LlmChatRequest) -> LlmResult<LlmChatResponse> {
        model_service::system_chat(self.inner.as_ref(), request).await
    }

    async fn chat_stream(
        &self,
        request: LlmChatRequest,
    ) -> LlmResult<BoxStream<'static, LlmResult<StreamEvent>>> {
        model_service::system_chat_stream(self.inner.as_ref(), request).await
    }
}

const DEFAULT_APP_MAX_OUTPUT_TOKENS: u32 = 1024;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test/unit/ai/gate.rs"));
}
