use super::*;
use clap::Parser;

#[test]
fn cli_parses_ask_subcommand() {
    let cli = Cli::try_parse_from(["yuos-cli", "ask", "hello", "world"]).unwrap();
    match cli.cmd {
        Some(CliCmd::Ask { prompt }) => assert_eq!(prompt, vec!["hello", "world"]),
        _ => panic!("expected Ask"),
    }
}

#[test]
fn cli_parses_status_subcommand() {
    let cli = Cli::try_parse_from(["yuos-cli", "status"]).unwrap();
    assert!(matches!(cli.cmd, Some(CliCmd::Status)));
}

#[test]
fn cli_parses_agent_passthrough() {
    let cli = Cli::try_parse_from(["yuos-cli", "agent", "model", "list"]).unwrap();
    match cli.cmd {
        Some(CliCmd::Agent { args }) => assert_eq!(args, vec!["model", "list"]),
        _ => panic!("expected Agent"),
    }
}

#[test]
fn cli_defaults_to_repl_when_no_subcommand() {
    let cli = Cli::try_parse_from(["yuos-cli"]).unwrap();
    assert!(cli.cmd.is_none());
}

#[test]
fn yu_bin_env_default() {
    let cli = Cli::try_parse_from(["yuos-cli"]).unwrap();
    // Default value when env var unset.
    assert_eq!(cli.yu_bin, "yu");
}

#[test]
fn one_shot_ask_rejects_empty_prompt() {
    let err = one_shot_ask("yu", vec!["   ".into()]).unwrap_err();
    assert!(err.to_string().contains("non-empty"));
}
