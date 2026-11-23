use super::*;

fn this_process(start_time_ticks: Option<u64>) -> ClientIdentity {
    let mut client = ClientIdentity::unknown();
    client.pid = Some(std::process::id());
    client.start_time_ticks = start_time_ticks;
    client
}

#[test]
fn an_administrator_ticket_lasts_five_minutes() {
    let issued = Instant::now();
    assert!(valid(issued, issued + Duration::from_secs(299)));
    assert!(!valid(issued, issued + WINDOW));
}

#[test]
fn the_terminal_session_is_read_after_the_command_name() {
    let line =
        "4242 (yu (app) x) S 4200 4242 4100 34816 4242 4194304 1 0 0 0 0 0 0 0 20 0 1 0 987654 0 0";
    assert_eq!(parse_stat(line), Some((4100, 34816, 987654)));
    assert_eq!(parse_stat("4242 (yu) S 4200"), None);
}

#[test]
fn a_ticket_binds_the_owner_and_the_verified_terminal_session() {
    const OWNER: u32 = 4_000_000_001;
    let (session, tty, start) = stat(std::process::id()).expect("read this process");
    let client = this_process(Some(start));
    let expected = (tty != 0).then(|| Ticket {
        owner: OWNER,
        tty,
        session,
        leader_start: stat(session).expect("read the session leader").2,
    });
    assert_eq!(ticket(OWNER, &client), expected);
    // A recycled pid or an unverified peer never has a terminal session.
    assert_eq!(ticket(OWNER, &this_process(Some(start + 1))), None);
    assert_eq!(ticket(OWNER, &ClientIdentity::unknown()), None);

    record(OWNER, &client);
    assert_eq!(reusable(OWNER, &client), tty != 0);
    assert!(!reusable(OWNER + 1, &client));
}
