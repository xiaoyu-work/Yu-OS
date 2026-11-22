//! App AI runs in the broker's owner-scoped gate, never in an App sandbox.

use serde_json::Value;

use super::{authority::Decision, client_identity::ClientIdentity, protocol::BrokerError};
use super::wire::requests::AiChat;
use crate::ai::{gate, inference};

const MODEL_PERMISSION_REQUIRED: &str = "model_permission_required";

const CHAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
static IN_FLIGHT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub async fn chat(
    params: Value,
    client: &ClientIdentity,
    authority: &Decision,
) -> Result<Value, BrokerError> {
    let body: AiChat =
        serde_json::from_value(params).map_err(|_| BrokerError::execution("invalid AI request"))?;
    let app_id = match &body {
        AiChat::Text(body) => body.app_id.as_str(),
        AiChat::Request(body) => body.app_id.as_str(),
    }.to_owned();
    require_app(authority, &app_id)?;
    // Worker relays call handlers directly, so enforce bounds here too.
    let _permit = IN_FLIGHT
        .try_acquire()
        .map_err(|_| BrokerError::unavailable("AI gate is busy"))?;
    let home = client.require_home_dir()?;
    let req = match body {
        AiChat::Text(body) => gate::ChatRequest {
            app_id: app_id.clone(),
            origin: body.origin.as_str().into(),
            prompt: Some(body.prompt.as_str().into()),
            system: body.system.map(|s| s.as_str().into()),
            max_units: body.max_units,
            tools: body.tools.as_slice().to_vec(),
            ..Default::default()
        },
        AiChat::Request(body) => body.request.into_request(app_id.clone()),
    };
    crate::paths::with_user_override(authority.owner_uid(), home, async {
        // Only daemon configuration is consulted. Client environment fields
        // are neither transported nor installed in this request scope.
        crate::config::with_snapshot(crate::config::load_user_config(), async {
            tokio::time::timeout(CHAT_TIMEOUT, gate::chat_authorized(req, authority))
                .await
                .map_err(|_| BrokerError::unavailable("AI request deadline exceeded"))?
                .map_err(|error| error_reply(&app_id, error))
                .and_then(|result| {
                    serde_json::to_value(result)
                        .map_err(|error| BrokerError::execution(error.to_string()))
                })
        })
        .await
    })
    .await
}

pub async fn infer(
    params: Value,
    client: &ClientIdentity,
    authority: &Decision,
) -> Result<Value, BrokerError> {
    let body: super::wire::requests::AiInference = serde_json::from_value(params)
        .map_err(|_| BrokerError::execution("invalid inference request"))?;
    require_app(authority, body.app_id.as_str())?;
    let _permit = IN_FLIGHT
        .try_acquire()
        .map_err(|_| BrokerError::unavailable("AI gate is busy"))?;
    let home = client.require_home_dir()?;
    crate::paths::with_user_override(authority.owner_uid(), home.clone(), async {
        crate::config::with_snapshot(crate::config::load_user_config(), async {
            tokio::time::timeout(
                CHAT_TIMEOUT,
                inference::run(body.app_id.as_str(), body.request, authority, &home),
            )
            .await
            .map_err(|_| BrokerError::unavailable("AI request deadline exceeded"))?
            .map_err(|error| error_reply(body.app_id.as_str(), error))
        })
        .await
    })
    .await
}

fn require_app(authority: &Decision, app_id: &str) -> Result<(), BrokerError> {
    authority
        .require_app(app_id)
        .map_err(BrokerError::authorization)?;
    let session = authority.session().map_err(BrokerError::authorization)?;
    if session.pending_bind || session.app_id.as_deref() != authority.app_id() {
        return Err(BrokerError::authorization(
            "AI requires the registered, bound App session",
        ));
    }
    Ok(())
}

fn error_reply(app_id: &str, error: gate::AiError) -> BrokerError {
    let mut detail = match &error {
        gate::AiError::Denied(detail) => detail.clone(),
        _ => serde_json::json!({}),
    };
    if error.wire_code() == "PERMISSION_DENIED" {
        detail["reason"] = gate::denial_reason_token(&error).into();
        detail["app_id"] = app_id.into();
    }
    let settings_required = if let gate::AiError::ModelSettingsRequired { verb, .. } = &error {
        detail["verb"] = verb.as_str().into();
        detail["settings_page"] = "applications".into();
        true
    } else {
        false
    };
    let data = serde_json::json!({
        "ai_error": {
            "error": error.to_string(), "code": error.wire_code(), "detail": detail,
        }
    });
    if settings_required {
        BrokerError::authorization_required(error.to_string(), data)
            .classified(MODEL_PERMISSION_REQUIRED)
    } else {
        BrokerError::with_data(error.to_string(), data)
    }
}

// Only an explicit refusal from the AI gate can carry Settings guidance
// to a verified GUI without an exercised model capability.
pub(super) fn may_return_without_spend(route: &str, result: &Result<Value, BrokerError>) -> bool {
    matches!(route, "ai.chat" | "ai.infer")
        && result.as_ref().is_err_and(|error| {
            error.kind == super::protocol::BrokerErrorKind::Unauthorized
                && error.audit_class == Some(MODEL_PERMISSION_REQUIRED)
        })
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/yuosd/ai.rs"
    ));
}
