//! `yu ai chat` — App-facing call to the configured chat model.
//!
//! `--request-stdin` carries the App prompt, images and function history.
//! The broker authenticates the registered session and derives authority from
//! the live grant. Apps route the other configured slots through `yu ai infer`.

use serde_json::Value;

use super::gate;

pub fn chat_cmd(args: &[String]) -> Result<Value, String> {
    let mut app: Option<String> = None;
    let mut prompt: Option<String> = None;
    let mut prompt_file: Option<String> = None;
    let mut origin = "trusted".to_string();
    let mut max_units: Option<u64> = None;
    let mut system: Option<String> = None;
    let mut system_file: Option<String> = None;
    let mut embed = false;
    let mut image_input: Option<std::path::PathBuf> = None;
    let mut image_output: Option<std::path::PathBuf> = None;
    let mut audio_input: Option<std::path::PathBuf> = None;
    let mut audio_output: Option<std::path::PathBuf> = None;
    let mut video_input: Option<std::path::PathBuf> = None;
    let mut video_output: Option<std::path::PathBuf> = None;
    let mut tools: Vec<String> = Vec::new();

    fn take_path(args: &[String], i: usize, flag: &str) -> Result<std::path::PathBuf, String> {
        args.get(i + 1)
            .cloned()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| super::invalid_args(format!("missing path for {flag}")))
    }

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" => {
                app = args.get(i + 1).cloned();
                i += 2;
            }
            "--prompt" => {
                prompt = args.get(i + 1).cloned();
                i += 2;
            }
            "--prompt-file" => {
                prompt_file = args.get(i + 1).cloned();
                i += 2;
            }
            "--origin" => {
                origin = args
                    .get(i + 1)
                    .cloned()
                    .ok_or_else(|| super::invalid_args("missing value for --origin"))?;
                i += 2;
            }
            "--max-units" => {
                max_units = Some(
                    args.get(i + 1)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| super::invalid_args("--max-units expects an integer"))?,
                );
                i += 2;
            }
            "--system" => {
                system = args.get(i + 1).cloned();
                i += 2;
            }
            "--system-file" => {
                system_file = args.get(i + 1).cloned();
                i += 2;
            }
            "--embed" => {
                embed = true;
                i += 1;
            }
            "--image-input" => {
                image_input = Some(take_path(args, i, "--image-input")?);
                i += 2;
            }
            "--image-output" => {
                image_output = Some(take_path(args, i, "--image-output")?);
                i += 2;
            }
            "--audio-input" => {
                audio_input = Some(take_path(args, i, "--audio-input")?);
                i += 2;
            }
            "--audio-output" => {
                audio_output = Some(take_path(args, i, "--audio-output")?);
                i += 2;
            }
            "--video-input" => {
                video_input = Some(take_path(args, i, "--video-input")?);
                i += 2;
            }
            "--video-output" => {
                video_output = Some(take_path(args, i, "--video-output")?);
                i += 2;
            }
            "--tools" => {
                let raw = args
                    .get(i + 1)
                    .ok_or_else(|| super::invalid_args("missing value for --tools"))?;
                tools = parse_tools_flag(raw);
                i += 2;
            }
            other => {
                return Err(super::invalid_args(format!(
                    "unknown flag for `yu ai chat`: {other}"
                )));
            }
        }
    }

    let unsupported = if embed {
        Some("embed")
    } else if image_output.is_some() {
        Some("image_generate")
    } else if image_input.is_some() && prompt.is_some() {
        Some("vision_analyze")
    } else if image_input.is_some() {
        Some("image_analyze")
    } else if audio_output.is_some() {
        Some("audio_tts")
    } else if audio_input.is_some() {
        Some("audio_stt")
    } else if video_output.is_some() {
        Some("video_generate")
    } else if video_input.is_some() {
        Some("video_analyze")
    } else {
        None
    };
    if let Some(modality) = unsupported {
        return Err(super::wire_error(
            "UNKNOWN_VERB",
            format!(
                "modality `{modality}` is not accepted by chat; use `yu ai infer` for configured embedding, speech or image generation"
            ),
            None,
        ));
    }

    let app = app.ok_or_else(|| super::invalid_args("--app is required"))?;

    let prompt_text: Option<String> = match (prompt, prompt_file) {
        (Some(_), Some(_)) => {
            return Err(super::invalid_args(
                "--prompt and --prompt-file are mutually exclusive",
            ));
        }
        (Some(p), None) => Some(p),
        (None, Some(path)) => Some(read_input_file(&path)?),
        (None, None) => None,
    };
    let system_text: Option<String> = match (system, system_file) {
        (Some(_), Some(_)) => {
            return Err(super::invalid_args(
                "--system and --system-file are mutually exclusive",
            ));
        }
        (Some(value), None) => Some(value),
        (None, Some(path)) => Some(read_input_file(&path)?),
        (None, None) => None,
    };

    let req = gate::ChatRequest {
        app_id: app,
        origin,
        prompt: prompt_text,
        system: system_text,
        max_units,
        embed,
        image_input,
        image_output,
        audio_input,
        audio_output,
        video_input,
        video_output,
        tools,
        ..Default::default()
    };

    broker_chat(req)
}

pub(crate) fn structured_cmd(stdin: Option<&[u8]>) -> Result<Value, String> {
    broker_request(crate::yuosd::routes::Command::AiChat, structured_params(stdin)?)
}

#[doc(hidden)]
pub fn structured_stream_cmd(
    stdin: &[u8],
    mut write_delta: impl FnMut(&str) -> std::io::Result<()>,
) -> Result<Value, String> {
    let mut params = structured_params(Some(stdin))?;
    params["stream"] = Value::Bool(true);
    let command = crate::yuosd::routes::Command::AiChat;
    (command.route().decode)(params.clone())
        .map_err(|_| super::invalid_args("invalid or oversized App AI stream request"))?;
    let response = crate::yuosd::client::request_chat_stream_for_session(
        crate::yuosd::protocol::Request::build(command, params),
        &mut write_delta,
    );
    broker_reply(response)
}

fn structured_params(stdin: Option<&[u8]>) -> Result<Value, String> {
    let input = stdin
        .filter(|input| input.len() <= crate::yuosd::wire::bounded::AI_CHAT_REQUEST_BYTES)
        .ok_or_else(|| super::invalid_args("chat requires bounded JSON on stdin"))?;
    let request = serde_json::from_slice::<Value>(input)
        .map_err(|_| super::invalid_args("chat requires valid JSON"))?;
    yuos_sdk::generated::validate_ai_chat_request(&request)
        .map_err(|error| super::invalid_args(error.to_string()))?;
    let app = std::env::var("YUOS_APP_ID")
        .map_err(|_| super::permission_denied("App identity is required", None))?;
    let session = std::env::var("YUOS_SESSION")
        .map_err(|_| super::permission_denied("App session is required", None))?;
    Ok(serde_json::json!({"session": session, "app_id": app, "request": request}))
}

fn read_input_file(path: &str) -> Result<String, String> {
    use std::io::Read;
    let limit = crate::yuosd::wire::bounded::FILE_TEXT_MAX_BYTES;
    let mut input = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take((limit + 1) as u64).read_to_string(&mut input))
        .map_err(|error| super::invalid_args(format!("AI input file: {error}")))?;
    if input.len() > limit {
        return Err(super::invalid_args("AI input file exceeds the text limit"));
    }
    Ok(input)
}

fn broker_chat(req: gate::ChatRequest) -> Result<Value, String> {
    // This is a routing identifier only. The broker authenticates the process
    // and resolves its live App grant; no registry or credentials enter workers.
    let session = std::env::var("YUOS_SESSION")
        .map_err(|_| super::permission_denied("App session is required", None))?;
    let params = serde_json::json!({
        "session": session, "app_id": req.app_id, "origin": req.origin,
        "prompt": req.prompt, "system": req.system, "max_units": req.max_units,
        "tools": req.tools,
    });
    broker_request(crate::yuosd::routes::Command::AiChat, params)
}

pub(super) fn broker_request(
    command: crate::yuosd::routes::Command,
    params: Value,
) -> Result<Value, String> {
    (command.route().decode)(params.clone())
        .map_err(|_| super::invalid_args("invalid or oversized App AI request"))?;
    broker_reply(crate::yuosd::client::request_for_session(
        crate::yuosd::protocol::Request::build(command, params),
    ))
}

fn broker_reply(
    response: Result<crate::yuosd::protocol::Response, crate::yuosd::client::ClientError>,
) -> Result<Value, String> {
    let response = response
        .map_err(|error| super::wire_error("KERNEL_UNAVAILABLE", error.to_string(), None))?;
    if response.ok {
        return response.result.ok_or_else(|| {
            super::wire_error("KERNEL_UNAVAILABLE", "AI broker returned no result", None)
        });
    }
    let error = response.error.ok_or_else(|| {
        super::wire_error("KERNEL_UNAVAILABLE", "AI broker returned no error", None)
    })?;
    if let Some(detail) = error.data.and_then(|data| data.get("ai_error").cloned()) {
        return Err(detail.to_string());
    }
    let code = match error.code.as_str() {
        "not_authorized" => "PERMISSION_DENIED",
        "invalid_params" => "INVALID_ARGS",
        _ => "KERNEL_UNAVAILABLE",
    };
    Err(super::wire_error(code, error.message, None))
}

/// Verify the caller is the App they claim to be.
///
/// `arg_app` is the `--app <id>` value parsed from the CLI; `env_app`
/// is the current process's `YUOS_APP_ID` claim (or `None` if unset).
/// [`enforce_identity_for`] additionally verifies the registered App
/// session and process ancestry.
///
/// A bare-process invocation (no `YUOS_APP_ID`) is rejected with a
/// dev-friendly hint; an env value that disagrees with `--app` is
/// rejected as cross-App impersonation.
fn enforce_identity(arg_app: &str, env_app: Option<&str>) -> Result<(), String> {
    match env_app {
        None => Err(format!(
            "`yu ai chat` must be invoked by the kernel via `yu app {arg_app} <op>`. \
             YUOS_APP_ID is not set, so the claimed App identity is missing."
        )),
        Some(env) if env != arg_app => Err(format!(
            "identity mismatch: --app={arg_app} but YUOS_APP_ID={env}. \
             An App may only request AI calls for its registered identity."
        )),
        Some(_) => Ok(()),
    }
}

/// Convenience wrapper used by sibling subcommands (`yu ai tool`)
/// that want the same identity enforcement as `yu ai chat`. Reads
/// `YUOS_APP_ID` from the current process env.
pub fn enforce_identity_for(arg_app: &str) -> Result<(), String> {
    enforce_identity(arg_app, std::env::var("YUOS_APP_ID").ok().as_deref())?;
    let session = crate::proc::current_session_info_for_caps()
        .ok_or_else(|| "App identity session is not registered".to_string())?;
    if session.app_id.as_deref() != Some(arg_app) {
        return Err(format!(
            "App identity mismatch: session `{}` is registered for {:?}, not `{arg_app}`",
            session.session_id, session.app_id
        ));
    }
    crate::caps::enforcement::require_current_session_identity(&session.session_id, session.pid)
        .map_err(|error| format!("App identity ancestry check failed: {error}"))
}

/// Normalize the legacy flag; the gate rejects nonempty lists.
fn parse_tools_flag(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test/unit/ai/chat.rs"));
}
