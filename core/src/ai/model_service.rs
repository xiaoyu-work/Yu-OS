//! Metered dispatch to the OS's configured models.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{stream::BoxStream, StreamExt};

use crate::agent::llm::{
    self,
    construction::ProviderBuildContext,
    types::{ChatRequest, ChatResponse, StreamEvent, Usage},
    Provider,
};
use crate::agent::media::{
    factory, imagegen::ImageGenProvider, stt::SttProvider, tts::TtsProvider,
};
use crate::caps::{self, Scope, Verb};
use crate::config::AgentConfig;
use crate::config::{EmbedConfig, YuConfig};
use crate::model::tasks::{decision, embed};
use crate::yuosd::authority::Decision;

use super::budget::{BudgetError, Meter, Snapshot, Store};
use super::{gate::AiError, user_budget};

pub(crate) fn configured_tts(config: &YuConfig) -> Option<Arc<dyn TtsProvider>> {
    let alias = config.tts.provider.trim().to_ascii_lowercase();
    factory::tts_registry_from_cfg(config).get(match alias.as_str() {
        "edge-tts" => "edge",
        "gemini-tts" => "gemini",
        "noop" => return None,
        name => name,
    })
}

pub(crate) fn configured_stt(config: &YuConfig) -> Option<Arc<dyn SttProvider>> {
    let alias = config.stt.provider.trim().to_ascii_lowercase();
    if alias == "noop" {
        return None;
    }
    factory::stt_registry_from_cfg(config).get(&alias)
}

pub(crate) fn configured_imagegen(config: &YuConfig) -> Option<Arc<dyn ImageGenProvider>> {
    let alias = config.imagegen.provider.trim().to_ascii_lowercase();
    if alias == "noop" {
        return None;
    }
    factory::imagegen_registry_from_cfg(config).get(&alias)
}

pub(crate) fn speech_units(text: &str) -> u64 {
    (text.len() as u64).max(50)
}

pub(crate) fn transcription_units(bytes: usize) -> u64 {
    (bytes.div_ceil(1024) as u64).saturating_mul(50)
}

pub(crate) fn image_units(count: u32) -> u64 {
    u64::from(count) * 1000
}

pub(super) fn decision_units(
    request: &decision::DecisionRequest,
) -> Result<u64, serde_json::Error> {
    serde_json::to_vec(request).map(|encoded| encoded.len() as u64)
}

pub(super) fn build_chat(
    provider: &str,
    model: &str,
    config: &AgentConfig,
    context: Option<&ProviderBuildContext>,
) -> llm::Result<Arc<dyn Provider>> {
    match context {
        Some(context) => llm::registry::build_with_context(provider, model, config, context),
        None => llm::registry::build(provider, model, config),
    }
}

pub(super) struct AppCall<'a> {
    meter: Meter,
    authority: Option<&'a Decision>,
}

pub(super) fn begin_app_call<'a>(
    authority: Option<&'a Decision>,
    app: &str,
    origin: &str,
    cap: u64,
    input: &impl serde::Serialize,
) -> Result<AppCall<'a>, AiError> {
    let owner = authority.map(Decision::owner_uid);
    let store = match owner {
        Some(uid) => Store::open_for_owner(uid),
        None => Store::open(),
    }
    .map_err(AiError::Internal)?;
    let meter = Meter::app(store, app, owner, cap, || user_budget::limit(owner))
    .map_err(|error| match error {
        BudgetError::OwnerLimit { period, cap } => {
            if let Some(owner) = owner {
                user_budget::notify(owner, &period, Some(cap));
            }
            AiError::UserBudgetExceeded
        }
        BudgetError::Settings(error) => {
            tracing::warn!(?owner, %error, "could not read the App AI budget");
            AiError::Internal("Could not read Settings > AI Budget".into())
        }
        error => AiError::Budget(error),
    })?;
    if let Some(authority) = authority {
        check_live(authority)?;
        let session = authority
            .session_id()
            .ok_or_else(|| AiError::Internal("missing App session".into()))?;
        meter
            .record_input(session, origin, input)
            .map_err(AiError::Internal)?;
    }
    Ok(AppCall {
        meter,
        authority,
    })
}

impl AppCall<'_> {
    pub(super) async fn run<T, E: std::fmt::Display>(
        self,
        pending: impl Future<Output = Result<T, E>>,
        usage: impl FnOnce(&T) -> u64,
    ) -> Result<(T, u64, Snapshot), AiError> {
        if let Some(authority) = self.authority {
            check_live(authority)?;
        }
        let authorized = async {
            match self.authority {
                Some(authority) => await_live(authority, pending).await,
                None => pending.await.map_err(|e| AiError::Provider(e.to_string())),
            }
        };
        complete(self.meter, authorized, usage, AiError::Budget).await
    }
}

async fn complete<T, E>(
    meter: Meter,
    pending: impl Future<Output = Result<T, E>>,
    usage: impl FnOnce(&T) -> u64,
    budget_error: impl FnOnce(BudgetError) -> E,
) -> Result<(T, u64, Snapshot), E> {
    let value = pending.await?;
    let (units, snapshot) = meter.finish(usage(&value)).map_err(budget_error)?;
    Ok((value, units, snapshot))
}

fn check_live(authority: &Decision) -> Result<(), AiError> {
    authority
        .ensure_current()
        .map_err(|error| AiError::Denied(serde_json::json!({"error": error})))
}

pub(crate) async fn await_live<T, E: std::fmt::Display>(
    authority: &Decision,
    pending: impl Future<Output = Result<T, E>>,
) -> Result<T, AiError> {
    check_live(authority)?;
    tokio::pin!(pending);
    loop {
        tokio::select! {
            response = &mut pending => {
                check_live(authority)?;
                return response.map_err(|error| AiError::Provider(error.to_string()));
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => check_live(authority)?,
        }
    }
}

fn system_meter(cap: u64, owner: Option<u32>) -> llm::Result<Meter> {
    let store = match owner {
        Some(uid) => Store::open_for_owner(uid),
        None => Store::open(),
    }
    .map_err(|e| llm::LlmError::Internal(format!("system-agent budget store: {e}")))?;
    Meter::system(store, cap)
        .map_err(|e| llm::LlmError::InvalidRequest(format!("system-agent budget: {e}")))
}

pub(crate) async fn system_media<T, E>(
    verb: Verb,
    provider: &str,
    units: u64,
    pending: impl Future<Output = Result<T, E>>,
) -> Result<T, String> {
    caps::require(verb, Scope::name(provider)).map_err(|denial| denial.to_string())?;
    let cap = crate::config::current_snapshot()
        .agent
        .system_budget_monthly_units;
    system_inference(units, None, cap, async {
        pending.await.map_err(|_| "configured media model request failed")
    })
    .await
}

async fn system_inference<T, E: std::fmt::Display>(
    units: u64,
    owner: Option<u32>,
    cap: u64,
    pending: impl Future<Output = Result<T, E>>,
) -> Result<T, String> {
    let meter = system_meter(cap, owner).map_err(|error| error.to_string())?;
    let (value, _, _) = complete(
        meter,
        async { pending.await.map_err(|error| error.to_string()) },
        |_| units,
        |error| settlement_error(error).to_string(),
    )
    .await?;
    Ok(value)
}

pub(crate) async fn system_decide(
    model: &decision::DecisionModel,
    request: &decision::DecisionRequest,
) -> Result<decision::DecisionResult, String> {
    request.validate()?;
    let units = decision_units(request).map_err(|error| error.to_string())?;
    let cap = crate::config::current_snapshot()
        .agent
        .system_budget_monthly_units;
    system_inference(units, None, cap, model.decide(request)).await
}

struct SystemEmbedder {
    provider: Box<dyn embed::Embedder>,
    cap: u64,
    owner: Option<u32>,
}

pub(crate) fn build_system_embedder(
    config: &EmbedConfig,
    agent: &AgentConfig,
    owner: Option<u32>,
) -> Result<Option<Box<dyn embed::Embedder>>, String> {
    Ok(
        embed::build_from_with_agent(config, agent)?.map(|provider| {
            Box::new(SystemEmbedder {
                provider,
                cap: agent.system_budget_monthly_units,
                owner,
            }) as Box<dyn embed::Embedder>
        }),
    )
}

#[async_trait::async_trait]
impl embed::Embedder for SystemEmbedder {
    fn name(&self) -> &str {
        self.provider.name()
    }

    fn model(&self) -> &str {
        self.provider.model()
    }

    fn is_configured(&self) -> bool {
        self.provider.is_configured()
    }

    async fn embed(
        &self,
        request: embed::EmbedRequest,
    ) -> Result<embed::EmbedResponse, embed::EmbedError> {
        let units = request.inputs.iter().map(|text| text.len() as u64).sum();
        let meter = system_meter(self.cap, self.owner)
            .map_err(|error| embed::EmbedError::Transport(error.to_string()))?;
        let (reply, _, _) = complete(
            meter,
            self.provider.embed(request),
            |_| units,
            |error| embed::EmbedError::Transport(settlement_error(error).to_string()),
        )
        .await?;
        Ok(reply)
    }
}

fn settlement_error(error: BudgetError) -> llm::LlmError {
    llm::LlmError::InvalidRequest(format!("system-agent budget settlement: {error}"))
}

fn admit_system_chat(request: &ChatRequest) -> llm::Result<Meter> {
    check_output_overrides(request)?;
    caps::require(Verb::AI_CHAT, Scope::name(&request.model)).map_err(|denial| {
        llm::LlmError::InvalidRequest(format!(
            "system-agent caps denied for ai.chat: {}",
            denial.to_json()
        ))
    })?;
    system_meter(
        crate::config::current_snapshot()
            .agent
            .system_budget_monthly_units,
        None,
    )
}

pub(super) async fn system_chat(
    provider: &dyn Provider,
    request: ChatRequest,
) -> llm::Result<ChatResponse> {
    let meter = admit_system_chat(&request)?;
    let (response, _, _) = complete(
        meter,
        provider.chat(request),
        |response| chat_units(&response.usage),
        settlement_error,
    )
    .await?;
    Ok(response)
}

pub(super) async fn system_chat_stream(
    provider: &dyn Provider,
    request: ChatRequest,
) -> llm::Result<BoxStream<'static, llm::Result<StreamEvent>>> {
    let meter = admit_system_chat(&request)?;
    let stream = provider.chat_stream(request).await?;
    Ok(settle_stream(stream, meter))
}

pub(super) fn settle_stream(
    stream: BoxStream<'static, llm::Result<StreamEvent>>,
    meter: Meter,
) -> BoxStream<'static, llm::Result<StreamEvent>> {
    let mut meter = Some(meter);
    Box::pin(stream.map(move |item| {
        if let Ok(StreamEvent::Done { ref usage, .. }) = item {
            if let Some(meter) = meter.take() {
                meter.finish(chat_units(usage)).map_err(settlement_error)?;
            }
        }
        item
    }))
}

pub(super) fn chat_units(usage: &Usage) -> u64 {
    u64::from(usage.input_tokens) + u64::from(usage.output_tokens)
}

const REQUEST_TOKEN_OVERHEAD: u64 = 512;

fn check_output_overrides(req: &ChatRequest) -> llm::Result<()> {
    if let serde_json::Value::Object(extra) = &req.extra {
        const RESERVED_OUTPUT_KEYS: &[&str] = &[
            "max_tokens",
            "max_completion_tokens",
            "max_output_tokens",
            "maxOutputTokens",
            "generationConfig",
            "generation_config",
        ];
        if let Some(key) = RESERVED_OUTPUT_KEYS
            .iter()
            .find(|key| extra.contains_key(**key))
        {
            return Err(llm::LlmError::InvalidRequest(format!(
                "provider extra field `{key}` cannot override the requested output limit"
            )));
        }
    }
    if req.max_tokens == Some(0) {
        return Err(llm::LlmError::InvalidRequest(
            "max_tokens must be greater than zero".into(),
        ));
    }
    Ok(())
}

pub(super) fn estimate_chat_input(req: &ChatRequest) -> u64 {
    let mut input_only = req.clone();
    input_only.max_tokens = None;
    serde_json::to_vec(&input_only)
        .map(|encoded| (encoded.len() as u64).saturating_add(REQUEST_TOKEN_OVERHEAD))
        .unwrap_or(u64::MAX)
}
