//! Sudo-style reuse of an administrator's authentication in one terminal.
//!
//! After an administrator approves a third-party App's system permission
//! through the `auth_admin` helper, the same owner's further approvals from
//! the same terminal session need no password for five minutes, like sudo's
//! tty ticket. The ticket is derived from the kernel-verified peer: owner uid,
//! controlling terminal, session id and the session leader's start time, so
//! another terminal, another owner or a reused pid never inherits it. A caller
//! without a controlling terminal has no ticket and is always asked. Tickets
//! live only in this process; a restart asks again.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::client_identity::ClientIdentity;

/// sudo's default `timestamp_timeout`.
const WINDOW: Duration = Duration::from_secs(5 * 60);

static TICKETS: Mutex<BTreeMap<Ticket, Instant>> = Mutex::new(BTreeMap::new());

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Ticket {
    owner: u32,
    tty: u64,
    session: u32,
    leader_start: u64,
}

/// An administrator just approved for `owner` from `client`'s terminal.
pub(super) fn record(owner: u32, client: &ClientIdentity) {
    if let Some(ticket) = ticket(owner, client) {
        let now = Instant::now();
        let mut tickets = TICKETS.lock().unwrap_or_else(PoisonError::into_inner);
        tickets.retain(|_, issued| valid(*issued, now));
        tickets.insert(ticket, now);
    }
}

/// Did an administrator approve for `owner` from `client`'s terminal
/// session within the last five minutes?
pub(super) fn reusable(owner: u32, client: &ClientIdentity) -> bool {
    ticket(owner, client).is_some_and(|ticket| {
        TICKETS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&ticket)
            .is_some_and(|issued| valid(*issued, Instant::now()))
    })
}

fn valid(issued: Instant, now: Instant) -> bool {
    now.duration_since(issued) < WINDOW
}

fn ticket(owner: u32, client: &ClientIdentity) -> Option<Ticket> {
    let (session, tty, start) = stat(client.pid?)?;
    if Some(start) != client.start_time_ticks || tty == 0 {
        return None;
    }
    let (_, _, leader_start) = stat(session)?;
    Some(Ticket {
        owner,
        tty,
        session,
        leader_start,
    })
}

fn stat(pid: u32) -> Option<(u32, u64, u64)> {
    parse_stat(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// Session id, controlling terminal and start time: fields 6, 7 and 22.
/// The command name may contain spaces and parentheses, so fields are
/// counted after its closing parenthesis.
fn parse_stat(stat: &str) -> Option<(u32, u64, u64)> {
    let fields: Vec<&str> = stat.get(stat.rfind(')')? + 1..)?.split_whitespace().collect();
    Some((
        fields.get(3)?.parse().ok()?,
        fields.get(4)?.parse().ok()?,
        fields.get(19)?.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test/unit/yuosd/administrator.rs"
    ));
}
