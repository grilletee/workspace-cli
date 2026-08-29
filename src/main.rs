mod cli;
mod winapi_safe;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use tracing_subscriber::EnvFilter;

fn main() {
    let exit_code = match run() {
        Ok(()) => 0,
        Err(error) => {
            tracing::error!(%error, "workspace-cli failed");
            1
        }
    };

    std::process::exit(exit_code);
}

fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Inspect => {
            let windows = winapi_safe::enumerate_visible_windows()?;
            println!("{}", serde_json::to_string(&windows)?);
        }
    }

    Ok(())
}
