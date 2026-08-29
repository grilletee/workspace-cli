mod cli;
mod launcher;
mod winapi_safe;

use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Serialize)]
struct TestLaunchSuccess {
    original_pid: u32,
    resolved_pid: u32,
    window: winapi_safe::WindowInfo,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct TestLaunchError {
    status: &'static str,
    message: String,
}

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
        Command::TestLaunch {
            path,
            args,
            timeout_ms,
        } => {
            let started = Instant::now();
            match launcher::spawn_detached(&path, &args)
                .and_then(|original_pid| {
                    launcher::resolve_window(original_pid, timeout_ms)
                        .map(|window| (original_pid, window))
                }) {
                Ok((original_pid, window)) => {
                    let result = TestLaunchSuccess {
                        original_pid,
                        resolved_pid: window.process_id,
                        window,
                        elapsed_ms: started.elapsed().as_millis(),
                    };
                    println!("{}", serde_json::to_string(&result)?);
                }
                Err(error) => {
                    let result = TestLaunchError {
                        status: "error",
                        message: format!("{error:#}"),
                    };
                    println!("{}", serde_json::to_string(&result)?);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
