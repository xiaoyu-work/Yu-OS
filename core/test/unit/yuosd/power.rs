use super::*;

#[test]
fn confirmation_is_required_for_power_actions() {
    assert!(validate_action("reboot", false).is_err());
    validate_action("reboot", true).unwrap();
    assert!(validate_action("status", true).is_err());
}

#[test]
fn upower_percentages_are_numeric() {
    assert_eq!(parse_upower_value("72.5%"), json!(72.5));
    assert_eq!(parse_upower_value("yes"), json!(true));
}

#[test]
fn missing_power_backend_is_unavailable() {
    let error = tool_path(&[], "logind")
        .map_err(backend_unavailable)
        .unwrap_err();
    assert_eq!(
        error.kind,
        crate::yuosd::protocol::BrokerErrorKind::Unavailable
    );
    let response = crate::yuosd::protocol::Response::handler_error(
        crate::yuosd::protocol::RequestId::unknown(),
        error,
    );
    assert_eq!(response.error.unwrap().code, "unavailable");
}

#[test]
fn invalid_power_operation_is_an_execution_failure() {
    let error = validate_action("hibernate-now", false)
        .map_err(BrokerError::execution)
        .unwrap_err();
    assert_eq!(
        error.kind,
        crate::yuosd::protocol::BrokerErrorKind::Execution
    );
    let response = crate::yuosd::protocol::Response::handler_error(
        crate::yuosd::protocol::RequestId::unknown(),
        error,
    );
    assert_eq!(response.error.unwrap().code, "execution_failed");
}
