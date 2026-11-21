// Single process-wide mutex serialising tests across the whole
// `caps` test surface. `bootstrap.rs` and `enforcement.rs` both
// mutate the same set of environment variables (`YUOS_SESSION`,
// `YUOS_DATA_DIR`, `YUOS_PERMS_MODE`, `YUOS_USER_DATA_DIR`, …); without
// a shared mutex their tests race each other when cargo runs them
// in parallel and produce spurious panics like
// `assert_eq!(env::var("YUOS_SESSION").unwrap(), …)` failing because
// a sibling test cleared the variable mid-flight.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the shared env lock, recovering from poisoning so a panic
/// in one test doesn't cascade-fail every subsequent test.
pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
