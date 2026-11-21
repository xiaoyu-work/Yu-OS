use super::*;

#[test]
fn overlay_flags_publish_the_activation_payload() {
    let flags = Flags {
        overlay: true,
        activation: Some(OverlayActivation::default()),
        ..Flags::default()
    };
    assert!(flags.action().is_some());
}

#[test]
fn socket_context_becomes_private_transient_activation() {
    let expected = OverlayActivation {
        context: Some(r#"{"app":"yuos-files"}"#.into()),
        ..OverlayActivation::default()
    };
    let input = serde_json::to_vec(&expected).unwrap();
    let parsed = yuos_runtime::ask_yu::parse_ui_arguments([
        "--overlay",
        "--context-socket",
        "--activation-fd",
        "3",
    ]);
    let launch_mode = UiLaunchMode::from_arguments(&parsed);
    let activation = parsed
        .activation(std::io::Cursor::new(input))
        .unwrap()
        .unwrap();
    let flags = Flags {
        overlay: true,
        context: activation.context.clone(),
        activation: Some(activation),
        ..Flags::default()
    };

    assert_eq!(
        flags.action().and_then(|value| value.context.as_deref()),
        Some(r#"{"app":"yuos-files"}"#)
    );
    assert_eq!(launch_mode, UiLaunchMode::PrivateOverlay);
    assert!(launch_mode.exits_on_close());
}

#[test]
fn conflicting_context_sources_do_not_produce_activation() {
    let parsed = yuos_runtime::ask_yu::parse_ui_arguments([
        "--overlay",
        "--context-socket",
        "--context",
        r#"{"app":"legacy"}"#,
    ]);
    let input = serde_json::to_vec(&OverlayActivation {
        context: Some(r#"{"app":"socket"}"#.into()),
        ..OverlayActivation::default()
    })
    .unwrap();

    assert!(matches!(
        parsed.activation(std::io::Cursor::new(input)),
        Err(yuos_runtime::ask_yu::ActivationInputError::ConflictingContext)
    ));
}

#[test]
fn malformed_and_oversize_socket_data_do_not_produce_activation() {
    let parsed = yuos_runtime::ask_yu::parse_ui_arguments(["--overlay", "--context-socket"]);
    assert!(matches!(
        parsed.activation(std::io::Cursor::new(b"{not-json")),
        Err(yuos_runtime::ask_yu::ActivationInputError::Malformed(_))
    ));
    assert!(matches!(
        parsed.activation(std::io::Cursor::new(vec![
            b'x';
            yuos_runtime::ask_yu::MAX_ACTIVATION_BYTES
                + 1
        ])),
        Err(yuos_runtime::ask_yu::ActivationInputError::TooLarge { .. })
    ));
}

#[test]
fn context_free_overlay_keeps_single_instance_behavior() {
    let parsed = yuos_runtime::ask_yu::parse_ui_arguments(["--overlay"]);
    let launch_mode = UiLaunchMode::from_arguments(&parsed);
    assert_eq!(launch_mode, UiLaunchMode::SharedOverlay);
    assert!(!launch_mode.exits_on_close());
}
