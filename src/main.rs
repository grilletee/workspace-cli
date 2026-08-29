mod cli;
mod config;
mod launcher;
mod orchestrator;
mod positioner;
mod winapi_safe;

use std::time::Instant;

use anyhow::{anyhow, Result};
use clap::Parser;
use cli::{Cli, Command};
use config::load_config;
use serde::Serialize;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Serialize)]
struct Position {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Serialize)]
struct TestLaunchSuccess {
    original_pid: u32,
    resolved_pid: u32,
    window: winapi_safe::WindowInfo,
    requested_position: Option<Position>,
    applied_position: Option<Position>,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct TestLaunchError {
    status: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct StartSuccess {
    status: &'static str,
    results: Vec<orchestrator::AppResult>,
}

#[derive(Debug, Serialize)]
struct StartError {
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
            x,
            y,
            width,
            height,
        } => run_test_launch(path, args, timeout_ms, x, y, width, height)?,
        Command::Start { profile, config } => return run_start(&profile, &config),
    }

    Ok(())
}

fn run_test_launch(
    path: String,
    args: Vec<String>,
    timeout_ms: u64,
    x: Option<i32>,
    y: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
) -> Result<()> {
    let requested_position = match (x, y, width, height) {
        (None, None, None, None) => None,
        (Some(x), Some(y), Some(width), Some(height)) => Some(Position {
            x,
            y,
            width,
            height,
        }),
        _ => return emit_test_launch_error(anyhow!(
            "--x, --y, --width and --height must be provided together"
        )),
    };

    let started = Instant::now();
    match launcher::spawn_detached(&path, &args).and_then(|original_pid| {
        launcher::resolve_window(original_pid, timeout_ms).and_then(|window| {
            let applied_position = requested_position.as_ref().map_or(Ok(None), |position| {
                positioner::set_window_position(
                    window.hwnd_value,
                    position.x,
                    position.y,
                    position.width,
                    position.height,
                )
                .map(|rect| {
                    Some(Position {
                        x: rect.left,
                        y: rect.top,
                        width: rect.right - rect.left,
                        height: rect.bottom - rect.top,
                    })
                })
            })?;

            Ok((original_pid, window, applied_position))
        })
    }) {
        Ok((original_pid, window, applied_position)) => {
            let result = TestLaunchSuccess {
                original_pid,
                resolved_pid: window.process_id,
                window,
                requested_position,
                applied_position,
                elapsed_ms: started.elapsed().as_millis(),
            };
            println!("{}", serde_json::to_string(&result)?);
        }
        Err(error) => return emit_test_launch_error(error),
    }

    Ok(())
}

fn run_start(profile_name: &str, config_path: &std::path::Path) -> Result<()> {
    let config = match load_config(config_path) {
        Ok(config) => config,
        Err(error) => return emit_start_error(2, error),
    };

    let profile = match config.profiles.get(profile_name) {
        Some(profile) => profile,
        None => {
            return emit_start_error(
                2,
                anyhow!("profile '{profile_name}' not found in {}", config_path.display()),
            )
        }
    };

    let results = orchestrator::run_profile(profile);
    let status = if results.iter().all(|result| result.status == "success") {
        "success"
    } else {
        "partial"
    };
    let exit_code = if status == "success" { 0 } else { 1 };

    println!(
        "{}",
        serde_json::to_string(&StartSuccess { status, results })?
    );
    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

fn emit_test_launch_error(error: anyhow::Error) -> Result<()> {
    let result = TestLaunchError {
        status: "error",
        message: format!("{error:#}"),
    };
    println!("{}", serde_json::to_string(&result)?);
    std::process::exit(1);
}

fn emit_start_error(exit_code: i32, error: anyhow::Error) -> Result<()> {
    let result = StartError {
        status: "error",
        message: format!("{error:#}"),
    };
    println!("{}", serde_json::to_string(&result)?);
    std::process::exit(exit_code);
}
