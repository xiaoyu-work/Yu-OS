use super::*;
use crate::ai::budget::{Meter, Store};

#[test]
fn wire_codes_follow_error_semantics() {
    assert_eq!(
        AiError::UnknownApp("missing".into()).wire_code(),
        "UNKNOWN_APP"
    );
    assert_eq!(AiError::BadOrigin("bad".into()).wire_code(), "INVALID_ARGS");
    assert_eq!(
        AiError::Denied(serde_json::json!({"decision": "deny"})).wire_code(),
        "PERMISSION_DENIED"
    );
    assert_eq!(
        AiError::Budget(BudgetError::OverUnitCap {
            app: "notes".into(),
            used: 10,
            cap: 10,
        })
        .wire_code(),
        "BUDGET_EXCEEDED"
    );
    assert_eq!(
        AiError::Provider("offline".into()).wire_code(),
        "KERNEL_UNAVAILABLE"
    );
    assert_eq!(
        AiError::Internal("broken".into()).wire_code(),
        "INTERNAL_ERROR"
    );
}

#[test]
fn parse_origin_known() {
    assert_eq!(parse_origin("trusted").unwrap(), PromptOrigin::Trusted);
    assert_eq!(
        parse_origin("external-content").unwrap(),
        PromptOrigin::ExternalContent
    );
}

#[test]
fn modality_derive_chat_default() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        prompt: Some("hi".into()),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::Chat);
}

#[test]
fn modality_derive_chat_untrusted_from_external_origin() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "external-content".into(),
        prompt: Some("hi".into()),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::ChatUntrusted);
}

#[test]
fn modality_derive_embed() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        prompt: Some("hi".into()),
        embed: true,
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::Embed);
}

#[test]
fn modality_derive_image_generate_from_image_output() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        prompt: Some("a cat".into()),
        image_output: Some(PathBuf::from("/tmp/out.png")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::ImageGenerate);
}

#[test]
fn modality_derive_image_analyze_no_prompt() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        image_input: Some(PathBuf::from("/tmp/in.png")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::ImageAnalyze);
}

#[test]
fn modality_derive_vision_analyze_with_prompt() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        prompt: Some("describe this".into()),
        image_input: Some(PathBuf::from("/tmp/in.png")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::VisionAnalyze);
}

#[test]
fn modality_derive_audio_tts() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        prompt: Some("hello world".into()),
        audio_output: Some(PathBuf::from("/tmp/out.wav")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::AudioTts);
}

#[test]
fn modality_derive_audio_stt() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        audio_input: Some(PathBuf::from("/tmp/in.wav")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::AudioStt);
}

#[test]
fn modality_derive_video_generate() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        prompt: Some("a sunrise".into()),
        video_output: Some(PathBuf::from("/tmp/out.mp4")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::VideoGenerate);
}

#[test]
fn modality_derive_video_analyze() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        video_input: Some(PathBuf::from("/tmp/in.mp4")),
        ..Default::default()
    };
    assert_eq!(Modality::derive(&req).unwrap(), Modality::VideoAnalyze);
}

#[test]
fn modality_derive_rejects_conflicting_selectors() {
    let req = ChatRequest {
        app_id: "x".into(),
        origin: "trusted".into(),
        image_input: Some(PathBuf::from("/tmp/i.png")),
        audio_input: Some(PathBuf::from("/tmp/a.wav")),
        ..Default::default()
    };
    let err = Modality::derive(&req).unwrap_err();
    assert!(matches!(err, AiError::ModalityConflict(_)));
}

#[test]
fn modality_verbs_cover_every_variant() {
    // Sanity: every variant has a corresponding caps verb. If a
    // future modality is added without wiring caps, this matches
    // statement will fail at compile time.
    let all = [
        Modality::Chat,
        Modality::ChatUntrusted,
        Modality::Embed,
        Modality::ImageGenerate,
        Modality::ImageAnalyze,
        Modality::VisionAnalyze,
        Modality::AudioTts,
        Modality::AudioStt,
        Modality::VideoGenerate,
        Modality::VideoAnalyze,
    ];
    for m in all {
        // verb() always returns; label() always returns. Cover both.
        let _ = m.verb();
        assert!(!m.label().is_empty());
    }
}

#[test]
fn apply_safety_minimal_is_passthrough() {
    let (out, changed) = apply_safety("hello sk-FAKE", AiSafety::Minimal);
    assert_eq!(out, "hello sk-FAKE");
    assert!(!changed);
}

#[test]
fn apply_safety_strict_redacts() {
    let secret = "AKIAIOSFODNN7EXAMPLE";
    let (out, changed) = apply_safety(&format!("key={secret}"), AiSafety::Strict);
    assert!(changed);
    assert!(!out.contains(secret));
}

#[test]
fn app_provider_error_does_not_forward_upstream_body() {
    let error = llm::LlmError::Provider {
        status: 401,
        message: "do not disclose this provider credential diagnostic".into(),
    };
    assert_eq!(app_provider_error(error), "chat model API returned HTTP 401");
}

#[test]
fn chat_images_detect_their_format_from_bytes() {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let images: [&[u8]; 3] = [
        b"\x89PNG\r\n\x1a\nfixture",
        b"\xff\xd8\xff\xe0fixture\xff\xd9",
        b"RIFF\x0c\0\0\0WEBPVP8 ",
    ];
    let blocks = chat_images(&images.map(|bytes| STANDARD.encode(bytes)), true).unwrap();
    let media_types: Vec<_> = blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Image { media_type, .. } => media_type.as_str(),
            other => panic!("unexpected block {other:?}"),
        })
        .collect();
    assert_eq!(media_types, ["image/png", "image/jpeg", "image/webp"]);
    assert!(matches!(
        chat_images(&[STANDARD.encode(b"GIF89a fixture")], true),
        Err(AiError::InvalidInference("unsupported image format; send PNG, JPEG or WebP"))
    ));
}

// ---------- Model service budget hold ----------

/// Build a `Store` backed by a private on-disk SQLite file under
/// a tempdir. Returns the tempdir so the file outlives the
/// store; dropping it cleans up. We override `YUOS_DATA_DIR` so
/// `Store::open()` uses our tempdir instead of the system path.
fn ephemeral_budget_store_via_tempdir() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("YUOS_DATA_DIR", dir.path());
    let store = Store::open().expect("open store in tempdir");
    (dir, store)
}

#[test]
fn app_meter_admits_until_recorded_usage_reaches_the_limit() {
    let _lock = crate::test_env::lock_env();
    let dir = tempfile::tempdir().unwrap();
    let _data = crate::test_env::TestEnvVarGuard::set("YUOS_DATA_DIR", dir.path());
    let meter = Meter::app(Store::open().unwrap(), "yuos-edit", None, 1000, || Ok(0)).unwrap();
    meter.finish(999).unwrap();
    let meter = Meter::app(Store::open().unwrap(), "yuos-edit", None, 1000, || Ok(0)).unwrap();
    // The last admitted call may finish over the limit.
    meter.finish(50).unwrap();
    assert_eq!(Store::open().unwrap().current("yuos-edit").unwrap().units_used, 1049);
    assert!(Meter::app(Store::open().unwrap(), "yuos-edit", None, 1000, || Ok(0)).is_err());
}

/// A call that ends without reported usage records nothing.
#[test]
fn unfinished_meter_records_nothing() {
    let (_dir, store) = ephemeral_budget_store_via_tempdir();
    drop(Meter::app(store, "test.app", None, 10_000, || Ok(0)).expect("admit"));
    let after = Store::open().unwrap().current("test.app").unwrap().units_used;
    std::env::remove_var("YUOS_DATA_DIR");
    assert_eq!(after, 0);
}

#[tokio::test]
async fn system_stream_settles_reported_openai_usage_to_actuals() {
    use futures_util::StreamExt;

    let (_dir, store) = ephemeral_budget_store_via_tempdir();
    let meter = Meter::system(store, 10_000).expect("admit");
    let inner: futures_util::stream::BoxStream<'static, crate::agent::llm::Result<StreamEvent>> =
        Box::pin(futures_util::stream::iter(vec![Ok(StreamEvent::Done {
            finish: FinishReason::Stop,
            usage: crate::agent::llm::types::Usage {
                input_tokens: 17,
                output_tokens: 3,
                ..Default::default()
            },
        })]));

    let events: Vec<_> = model_service::settle_stream(inner, meter).collect().await;
    let charged = Store::open()
        .unwrap()
        .current(SYSTEM_AGENT_BUCKET)
        .unwrap()
        .units_used;
    std::env::remove_var("YUOS_DATA_DIR");

    assert!(matches!(
        events.last(),
        Some(Ok(StreamEvent::Done { usage, .. }))
            if usage.input_tokens == 17 && usage.output_tokens == 3
    ));
    assert_eq!(
        charged, 20,
        "streaming usage must be recorded as reported"
    );
}

#[tokio::test]
async fn system_stream_without_usage_records_nothing() {
    use futures_util::StreamExt;

    let (_dir, store) = ephemeral_budget_store_via_tempdir();
    let meter = Meter::system(store, 10_000).expect("admit");
    let inner: futures_util::stream::BoxStream<'static, crate::agent::llm::Result<StreamEvent>> =
        Box::pin(futures_util::stream::iter(vec![Ok(StreamEvent::Done {
            finish: FinishReason::Stop,
            usage: Default::default(),
        })]));

    let events: Vec<_> = model_service::settle_stream(inner, meter).collect().await;
    let charged = Store::open()
        .unwrap()
        .current(SYSTEM_AGENT_BUCKET)
        .unwrap()
        .units_used;
    std::env::remove_var("YUOS_DATA_DIR");

    assert!(events.last().is_some_and(Result::is_ok));
    assert_eq!(charged, 0);
}
