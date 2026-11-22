use super::*;

fn with_tmp_config_dir<R>(label: &str, f: impl FnOnce() -> R) -> R {
    let tmp = std::env::temp_dir().join(format!(
        "yuos-user-budget-test-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    let prev = std::env::var_os("YUOS_USER_CONFIG_DIR");
    std::env::set_var("YUOS_USER_CONFIG_DIR", &tmp);
    let out = f();
    match prev {
        Some(v) => std::env::set_var("YUOS_USER_CONFIG_DIR", v),
        None => std::env::remove_var("YUOS_USER_CONFIG_DIR"),
    }
    let _ = fs::remove_dir_all(&tmp);
    out
}

#[test]
fn missing_file_is_unlimited() {
    with_tmp_config_dir("missing", || {
        let b = load().unwrap();
        assert_eq!(b.monthly_units, 0);
    });
}

#[test]
fn empty_file_is_unlimited() {
    with_tmp_config_dir("empty", || {
        let path = config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();
        assert_eq!(load().unwrap().monthly_units, 0);
    });
}

#[test]
fn explicit_zero_is_unlimited() {
    with_tmp_config_dir("zero", || {
        let path = config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"monthly_units": 0}"#).unwrap();
        assert_eq!(load().unwrap().monthly_units, 0);
    });
}

#[test]
fn nonzero_cap_loads() {
    with_tmp_config_dir("cap", || {
        let path = config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"monthly_units": 1234567}"#).unwrap();
        let b = load().unwrap();
        assert_eq!(b.monthly_units, 1234567);
    });
}

#[test]
fn unknown_field_is_ignored() {
    with_tmp_config_dir("extra", || {
        let path = config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"monthly_units": 42, "future_usd_axis": 99.99}"#,
        )
        .unwrap();
        let b = load().unwrap();
        assert_eq!(b.monthly_units, 42);
    });
}

#[test]
fn malformed_file_errors() {
    with_tmp_config_dir("bad", || {
        let path = config_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not json").unwrap();
        assert!(load().is_err());
    });
}

#[test]
fn settings_accepts_sqlite_max_and_does_not_replace_it_on_invalid_apply() {
    let _lock = crate::test_env::lock_env();
    with_tmp_config_dir("apply", || {
        let data = tempfile::tempdir().unwrap();
        let _data = crate::test_env::TestEnvVarGuard::set("YUOS_DATA_DIR", data.path());
        let mut store = Store::open().unwrap();
        let max = i64::MAX as u64;
        let saved = status_with(&mut store, None, Some(max)).unwrap();
        assert_eq!(saved.monthly_units, max);
        assert!(status_with(&mut store, None, Some(max + 1)).is_err());
        assert_eq!(status_with(&mut store, None, None).unwrap(), saved);
        assert_eq!(status_with(&mut store, None, Some(0)).unwrap().monthly_units, 0);
    });
}
