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
}
