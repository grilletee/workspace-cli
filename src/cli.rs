use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "workspace-cli", version, about = "Automate a Windows development workspace")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect visible top-level windows.
    Inspect,
    /// Launch one application and resolve its resulting window.
    TestLaunch {
        /// Executable path to launch.
        path: String,
        /// Arguments passed to the executable.
        #[arg(long = "args", value_delimiter = ' ')]
        args: Vec<String>,
        /// Maximum time to wait for a visible window.
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },
}
