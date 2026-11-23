use super::*;

#[test]
fn chat_opts_chaining() {
    let opts = ChatOpts::default()
        .origin("external-content")
        .max_units(2000)
        .app("test-app")
        .tools(["fs.read_text", "kv.get"]);
    assert_eq!(opts.origin.as_deref(), Some("external-content"));
    assert_eq!(opts.max_units, Some(2000));
    assert_eq!(opts.app_id.as_deref(), Some("test-app"));
    assert_eq!(opts.tools.as_ref().unwrap().len(), 2);
}

#[test]
fn chat_rejects_blank_prompt() {
    let err = chat("", ChatOpts::default()).unwrap_err();
    assert!(matches!(err, AiError::InvalidArg(_)));
}

#[test]
fn classify_budget_error_by_code() {
    let payload = serde_json::json!({"error": "out of units", "code": "BUDGET_EXCEEDED"});
    let err = classify_ai_error("out of units", &payload);
    assert!(matches!(err, AiError::BudgetExceeded(_)));
}

#[test]
fn classify_safety_error_by_code() {
    let payload = serde_json::json!({"error": "opaque", "code": "SAFETY_VIOLATION"});
    let err = classify_ai_error("opaque", &payload);
    assert!(matches!(err, AiError::SafetyViolation(_)));
}

#[test]
fn error_messages_do_not_override_stable_codes() {
    let payload = serde_json::json!({
        "error": "budget safety blocked",
        "code": "PERMISSION_DENIED"
    });
    let err = classify_ai_error("budget safety blocked", &payload);
    assert!(matches!(err, AiError::Denied { .. }));
    let unavailable = classify_ai_error("opaque", &serde_json::json!({"code": "KERNEL_UNAVAILABLE"}));
    assert!(matches!(unavailable, AiError::ServiceUnavailable(_)));
    let invalid = classify_ai_error("opaque", &serde_json::json!({"code": "INVALID_ARGS"}));
    assert!(matches!(invalid, AiError::InvalidArg(_)));
}

#[test]
fn malformed_tool_call_fails_the_response() {
    let error = parse_response(serde_json::json!({
        "text": "hello",
        "model": "m",
        "provider": "p",
        "verb": "ai.chat",
        "usage": {"input_tokens": 1, "output_tokens": 1, "units": 2},
        "budget": {"period": "2026-08", "units_used": 2, "units_cap": 100},
        "review": {"safety": "strict", "prompt_redacted": false},
        "tool_calls": [{"id": "c1", "input": {}}]
    }))
    .unwrap_err();
    assert!(matches!(
        error,
        AiError::Unavailable(message)
            if message.contains("WIRE_REQUIRED") && message.contains("$.tool_calls[0].name")
    ));
}

#[test]
fn response_accepts_mathematical_integers_and_unrestricted_tool_input() {
    let response = parse_response(
        serde_json::from_str(
            r#"{
                "text":"hello","model":"m","provider":"p","verb":"ai.chat",
                "usage":{"input_tokens":1.0,"output_tokens":1e0,"units":18446744073709551615},
                "budget":{"period":"2026-08","units_used":2e0,"units_cap":100.0},
                "review":{"safety":"strict","prompt_redacted":false},
                "tool_calls":[{"id":"c1","name":"echo","input":0.12345678901234567890}]
            }"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(response.usage.input_tokens, 1);
    assert_eq!(response.usage.output_tokens, 1);
    assert_eq!(response.units, u64::MAX);
    assert_eq!(response.units, response.usage.units);
    assert_eq!(response.output.text, response.text);
    assert_eq!(
        serde_json::to_string(&response.tool_calls[0].input).unwrap(),
        "0.12345678901234567890"
    );
}

#[test]
fn scalar_root_has_stable_decode_error() {
    let error = parse_response(serde_json::Value::Null).unwrap_err();
    assert!(matches!(
        error,
        AiError::Unavailable(message)
            if message.contains("WIRE_TYPE") && message.contains(" at $:")
    ));
}

#[test]
fn media_outputs_decode_to_bytes() {
    let speech: Speech = serde_json::from_value(serde_json::json!({
        "kind": "tts", "format": "wav", "audio_base64": "UklGRg==", "sample_rate_hz": 24000
    }))
    .unwrap();
    assert_eq!((speech.audio, speech.sample_rate_hz), (b"RIFF".to_vec(), Some(24000)));
    let image: Image = serde_json::from_value(serde_json::json!({
        "kind": "imagegen", "format": "png", "image_base64": "iVBORw=="
    }))
    .unwrap();
    assert_eq!(image.image, b"\x89PNG");
    assert!(serde_json::from_value::<Image>(serde_json::json!({
        "format": "png", "image_base64": "not base64"
    }))
    .is_err());
}

#[test]
#[cfg(unix)]
fn stt_sends_audio_bytes_as_base64() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("yu");
    let request = dir.path().join("request.json");
    let reply = r#"{"ok":true,"wire_version":1,"data":{"provider":"p","model":"m","units":50,"output":{"kind":"stt","text":"hello"}}}"#;
    std::fs::write(
        &bin,
        format!("#!/bin/sh\ncat > '{}'\nprintf '%s\\n' '{reply}'\n", request.display()),
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let wav = b"RIFF\x24\0\0\0WAVEfmt ";
    std::env::set_var("YUOS_KERNEL_BIN", &bin);
    let transcript = stt(wav, ModelOpts::default());
    std::env::remove_var("YUOS_KERNEL_BIN");

    assert_eq!(transcript.unwrap().output.text, "hello");
    let sent: serde_json::Value =
        serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
    assert_eq!(
        sent["input"],
        serde_json::json!({"kind": "stt", "audio_base64": STANDARD.encode(wav)})
    );
}

// Silence unused-import warnings for HashMap when feature combos
// exclude every consumer (current code-shape doesn't use it, but
// we leave the import to keep the module ready for future per-call
// metadata).
#[allow(dead_code)]
fn _placate() {}
