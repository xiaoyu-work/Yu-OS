//! Command-line and desktop functions for a Yu OS App.
//!
//! ```no_run
//! use yuos_sdk::app::App;
//!
//! fn main() -> Result<(), yuos_sdk::app::AppError> {
//!     App::new()
//!         // yu app <id> summarize "text"
//!         .command("summarize", |args| Ok(serde_json::json!({"summary": args[0]})))
//!         // App launcher or yu app <id> --gui
//!         .desktop(|_files| Ok(()))
//!         .run()
//! }
//! ```
//!
//! Declare the command under `operations` and the window under `desktop` in
//! `app.json`.

use std::collections::BTreeMap;
use std::io::Write;

/// Error returned by an App function.
pub type FunctionError = Box<dyn std::error::Error + Send + Sync>;

type CommandFn = Box<dyn FnOnce(Vec<String>) -> Result<serde_json::Value, FunctionError>>;
type DesktopFn = Box<dyn FnOnce(Vec<String>) -> Result<(), FunctionError>>;

/// The App's command-line and desktop functions.
#[derive(Default)]
pub struct App {
    commands: BTreeMap<String, CommandFn>,
    desktop: Option<DesktopFn>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("the App has no function for command `{0}`")]
    NoCommand(String),
    #[error("the App has no desktop function")]
    NoDesktop,
    #[error("YUOS_ARGS_JSON must be an array of strings: {0}")]
    Args(String),
    #[error(transparent)]
    Function(FunctionError),
    #[error("write the command result: {0}")]
    Output(std::io::Error),
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run `function` for `yu app <id> <name>`. Its value is printed as JSON.
    pub fn command<F>(mut self, name: &str, function: F) -> Self
    where
        F: FnOnce(Vec<String>) -> Result<serde_json::Value, FunctionError> + 'static,
    {
        assert!(!name.is_empty(), "command name must be nonempty");
        let previous = self.commands.insert(name.to_owned(), Box::new(function));
        assert!(previous.is_none(), "command `{name}` already has a function");
        self
    }

    /// Run `function` with the opened files to open the App's desktop window.
    pub fn desktop<F>(mut self, function: F) -> Self
    where
        F: FnOnce(Vec<String>) -> Result<(), FunctionError> + 'static,
    {
        let previous = self.desktop.replace(Box::new(function));
        assert!(previous.is_none(), "the desktop window already has a function");
        self
    }

    /// Call the function Yu OS started this process for.
    pub fn run(mut self) -> Result<(), AppError> {
        let args = match std::env::var("YUOS_ARGS_JSON") {
            Ok(raw) => {
                serde_json::from_str(&raw).map_err(|error| AppError::Args(error.to_string()))?
            }
            Err(std::env::VarError::NotPresent) => Vec::new(),
            Err(error) => return Err(AppError::Args(error.to_string())),
        };
        if crate::gui::is_gui_launch() {
            let function = self.desktop.ok_or(AppError::NoDesktop)?;
            return function(args).map_err(AppError::Function);
        }
        let name = std::env::var("YUOS_COMMAND").unwrap_or_default();
        let function = self
            .commands
            .remove(&name)
            .ok_or(AppError::NoCommand(name))?;
        let result = function(args).map_err(AppError::Function)?;
        if result.is_null() {
            return Ok(());
        }
        writeln!(std::io::stdout(), "{result}").map_err(AppError::Output)
    }
}
