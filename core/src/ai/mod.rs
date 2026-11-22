//! Configured model dispatch for the System Agent and authenticated Apps.
//!
//! The App gate applies exact capability, policy and consent checks.
//! The model service accounts for App and System Agent usage separately.

pub(crate) mod app_tools;
pub mod budget;
pub mod chat;
pub mod consent;
pub mod gate;
pub mod inference;
pub(crate) mod model_service;
pub mod overrides;
pub mod tools;
pub mod user_budget;

pub(super) fn wire_error(
    code: &'static str,
    message: impl Into<String>,
    detail: Option<serde_json::Value>,
) -> String {
    let mut error = serde_json::json!({
        "error": message.into(),
        "code": code,
    });
    if let Some(detail) = detail.filter(serde_json::Value::is_object) {
        error["detail"] = detail;
    }
    error.to_string()
}

pub(super) fn invalid_args(message: impl Into<String>) -> String {
    wire_error("INVALID_ARGS", message, None)
}

pub(super) fn permission_denied(
    message: impl Into<String>,
    detail: Option<serde_json::Value>,
) -> String {
    wire_error("PERMISSION_DENIED", message, detail)
}

/// Dispatcher for `yu ai <command>`. Exposes:
///   * `chat` — single-shot, gated, modality-derived LLM call.
///   * `tool` — single Tool invocation from the App-facing catalog
///     (see [`tools::CATALOG`]).
///   * `tools` — print the catalog of separate App primitives.
pub fn run(command: &str, args: &[String]) -> Result<serde_json::Value, String> {
    match command {
        "chat" => chat::chat_cmd(args),
        "tool" => tool_cmd(args),
        "tools" => tools_list_cmd(args),
        other => Err(wire_error(
            "UNKNOWN_VERB",
            format!("unknown command: yu ai {other}. try: chat | infer | settings | tool | tools"),
            None,
        )),
    }
}

/// Implements `yu ai tool <name> --app <id> [--args <json>|--args-file <p>]`.
///
/// Identity is enforced via the same helper `yu ai chat` uses:
/// env claim, registered `app_id`, and nearest App ancestry must agree.
fn tool_cmd(args: &[String]) -> Result<serde_json::Value, String> {
    let mut name: Option<String> = None;
    let mut app: Option<String> = None;
    let mut args_json: Option<String> = None;
    let mut args_file: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" => {
                app = args.get(i + 1).cloned();
                i += 2;
            }
            "--args" => {
                args_json = args.get(i + 1).cloned();
                i += 2;
            }
            "--args-file" => {
                args_file = args.get(i + 1).cloned();
                i += 2;
            }
            other if !other.starts_with("--") && name.is_none() => {
                name = Some(other.to_string());
                i += 1;
            }
            other => {
                return Err(invalid_args(format!(
                    "unknown flag for `yu ai tool`: {other}"
                )));
            }
        }
    }

    let name = name.ok_or_else(|| {
        invalid_args("missing tool name. usage: yu ai tool <name> --app <id> --args <json>")
    })?;
    let app = app.ok_or_else(|| invalid_args("--app is required"))?;

    chat::enforce_identity_for(&app).map_err(|error| permission_denied(error, None))?;

    let raw = match (args_json, args_file) {
        (Some(s), _) => s,
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map_err(|e| invalid_args(format!("--args-file {path}: {e}")))?,
        (None, None) => "{}".to_string(),
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| invalid_args(format!("--args is not valid JSON: {e}")))?;

    let result = tools::execute(&name, &app, &parsed).map_err(|error| error.to_wire_error())?;
    serde_json::to_value(result)
        .map_err(|error| wire_error("INTERNAL_ERROR", error.to_string(), None))
}

/// Implements `yu ai tools` — print the App-facing Tool catalog.
fn tools_list_cmd(args: &[String]) -> Result<serde_json::Value, String> {
    if !args.is_empty() {
        return Err(invalid_args(format!(
            "`yu ai tools` takes no arguments; got: {}",
            args.join(" ")
        )));
    }
    let entries: Result<Vec<_>, String> = tools::CATALOG
        .iter()
        .map(|t| {
            let args_schema = serde_json::from_str::<serde_json::Value>(t.args_schema)
                .map_err(|error| wire_error("INTERNAL_ERROR", error.to_string(), None))?;
            let returns_schema = serde_json::from_str::<serde_json::Value>(t.returns_schema)
                .map_err(|error| wire_error("INTERNAL_ERROR", error.to_string(), None))?;
            Ok(serde_json::json!({
                "name": t.name,
                "summary": t.summary,
                "verb": t.verb.as_str(),
                "stability": match t.stability {
                    tools::Stability::Stable => "stable",
                    tools::Stability::Experimental => "experimental",
                },
                "args_schema": args_schema,
                "returns_schema": returns_schema,
            }))
        })
        .collect();
    Ok(serde_json::json!({ "tools": entries? }))
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test/unit/ai.rs"));
}
