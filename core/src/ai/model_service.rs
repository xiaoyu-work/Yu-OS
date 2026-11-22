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

use super::budget::{BudgetError, Reservation, Snapshot, Store};
use super::{gate::AiError, user_budget};

static APP_IN_FLIGHT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

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
    _permit: tokio::sync::SemaphorePermit<'static>,
    reservation: Reservation,
    authority: Option<&'a Decision>,
}

pub(super) fn begin_app_call<'a>(
    authority: Option<&'a Decision>,
    app: &str,
    origin: &str,
    cap: u64,
    estimate: u64,
    input: &impl serde::Serialize,
) -> Result<AppCall<'a>, AiError> {
    let permit = APP_IN_FLIGHT
        .try_acquire()
        .map_err(|_| AiError::Provider("AI service is busy".into()))?;
    let owner = authority.map(Decision::owner_uid);
    let store = match owner {
        Some(uid) => Store::open_for_owner(uid),
        None => Store::open(),
    }
    .map_err(AiError::Internal)?;
    let reservation = Reservation::app(store, app, owner, estimate, cap, || {
        user_budget::limit(owner)
    })
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
        reservation
            .record_input(session, origin, input)
            .map_err(AiError::Internal)?;
    }
    Ok(AppCall {
        _permit: permit,
        reservation,
        authority,
    })
}

impl AppCall<'_> {
    pub(super) async fn run<T, E: std::fmt::Display>(
        mut self,
        pending: impl Future<Output = Result<T, E>>,
        usage: impl FnOnce(&T) -> u64,
    ) -> Result<(T, u64, Snapshot), AiError> {
        if let Some(authority) = self.authority {
            check_live(authority)?;
        }
        // Once dispatch starts, cancellation cannot prove the provider spent nothing.
        self.reservation.retain_estimate_on_drop();
        let authorized = async {
            match self.authority {
                Some(authority) => await_live(authority, pending).await,
                None => pending.await.map_err(|e| AiError::Provider(e.to_string())),
            }
        };
        complete(self.reservation, authorized, usage, AiError::Budget).await
    }
}

async fn complete<T, E>(
    mut reservation: Reservation,
    pending: impl Future<Output = Result<T, E>>,
    usage: impl FnOnce(&T) -> u64,
    budget_error: impl FnOnce(BudgetError) -> E,
) -> Result<(T, u64, Snapshot), E> {
    let value = pending.await?;
    reservation.retain_estimate_on_drop();
    let (units, snapshot) = reservation.finish(usage(&value)).map_err(budget_error)?;
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

fn system_hold(estimate: u64, cap: u64, owner: Option<u32>) -> llm::Result<Reservation> {
    let store = match owner {
        Some(uid) => Store::open_for_owner(uid),
        None => Store::open(),
    }
    .map_err(|e| llm::LlmError::Internal(format!("system-agent budget store: {e}")))?;
    Reservation::system(store, estimate, cap)
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
    let hold = dispatched_system_hold(units, cap, owner).map_err(|error| error.to_string())?;
    let (value, _, _) = complete(
        hold,
        async { pending.await.map_err(|error| error.to_string()) },
        |_| units,
        |error| settlement_error(error).to_string(),
    )
    .await?;
    Ok(value)
}

fn dispatched_system_hold(units: u64, cap: u64, owner: Option<u32>) -> llm::Result<Reservation> {
    let mut hold = system_hold(units, cap, owner)?;
    hold.retain_estimate_on_drop();
    Ok(hold)
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
        let hold = dispatched_system_hold(units, self.cap, self.owner)
            .map_err(|error| embed::EmbedError::Transport(error.to_string()))?;
        let (reply, _, _) = complete(
            hold,
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

fn admit_system_chat(request: &mut ChatRequest) -> llm::Result<Reservation> {
    normalize_output_limit(request)?;
    caps::require(Verb::AI_CHAT, Scope::name(&request.model)).map_err(|denial| {
        llm::LlmError::InvalidRequest(format!(
            "system-agent caps denied for ai.chat: {}",
            denial.to_json()
        ))
    })?;
    system_hold(
        estimate_chat_request(request),
        crate::config::current_snapshot()
            .agent
            .system_budget_monthly_units,
        None,
    )
}

pub(super) async fn system_chat(
    provider: &dyn Provider,
    mut request: ChatRequest,
) -> llm::Result<ChatResponse> {
    let reservation = admit_system_chat(&mut request)?;
    let (response, _, _) = complete(
        reservation,
        provider.chat(request),
        |response| chat_units(&response.usage),
        settlement_error,
    )
    .await?;
    Ok(response)
}

pub(super) async fn system_chat_stream(
    provider: &dyn Provider,
    mut request: ChatRequest,
) -> llm::Result<BoxStream<'static, llm::Result<StreamEvent>>> {
    let reservation = admit_system_chat(&mut request)?;
    let stream = provider.chat_stream(request).await?;
    Ok(settle_stream(stream, reservation))
}

pub(super) fn settle_stream(
    stream: BoxStream<'static, llm::Result<StreamEvent>>,
    mut reservation: Reservation,
) -> BoxStream<'static, llm::Result<StreamEvent>> {
    reservation.retain_estimate_on_drop();
    let mut reservation = Some(reservation);
    Box::pin(stream.map(move |item| {
        if let Ok(StreamEvent::Done { ref usage, .. }) = item {
            if let Some(hold) = reservation.take() {
                hold.finish(chat_units(usage)).map_err(settlement_error)?;
            }
        }
        item
    }))
}

pub(super) fn chat_units(usage: &Usage) -> u64 {
    u64::from(usage.input_tokens) + u64::from(usage.output_tokens)
}

const DEFAULT_SYSTEM_MAX_OUTPUT_TOKENS: u32 = 4096;
const REQUEST_TOKEN_OVERHEAD: u64 = 512;

fn normalize_output_limit(req: &mut ChatRequest) -> llm::Result<()> {
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
                "provider extra field `{key}` cannot override the budgeted output limit"
            )));
        }
    }
    match req.max_tokens {
        Some(0) => Err(llm::LlmError::InvalidRequest(
            "max_tokens must be greater than zero".into(),
        )),
        Some(_) => Ok(()),
        None => {
            req.max_tokens = Some(DEFAULT_SYSTEM_MAX_OUTPUT_TOKENS);
            Ok(())
        }
    }
}

pub(super) fn estimate_chat_input(req: &ChatRequest) -> u64 {
    let mut input_only = req.clone();
    input_only.max_tokens = None;
    serde_json::to_vec(&input_only)
        .map(|encoded| (encoded.len() as u64).saturating_add(REQUEST_TOKEN_OVERHEAD))
        .unwrap_or(u64::MAX)
}

pub(super) fn estimate_chat_request(req: &ChatRequest) -> u64 {
    estimate_chat_input(req).saturating_add(u64::from(
        req.max_tokens.unwrap_or(DEFAULT_SYSTEM_MAX_OUTPUT_TOKENS),
    ))
}
