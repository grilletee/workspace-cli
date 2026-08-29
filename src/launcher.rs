use std::os::windows::process::CommandExt;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use tracing::warn;
use windows::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
};

use crate::winapi_safe::{self, WindowInfo};

pub fn spawn_detached(path: &str, args: &[String]) -> Result<u32> {
    let flags = CREATE_NEW_PROCESS_GROUP.0
        | DETACHED_PROCESS.0
        | CREATE_BREAKAWAY_FROM_JOB.0;

    match Command::new(path)
        .args(args)
        .creation_flags(flags)
        .spawn()
    {
        Ok(child) => Ok(child.id()),
        Err(first_error) => {
            warn!(
                %first_error,
                "process could not break away from parent job; retrying attached to the parent job"
            );

            let fallback_flags = CREATE_NEW_PROCESS_GROUP.0 | DETACHED_PROCESS.0;
            Command::new(path)
                .args(args)
                .creation_flags(fallback_flags)
                .spawn()
                .with_context(|| {
                    format!(
                        "failed to launch '{path}' with or without CREATE_BREAKAWAY_FROM_JOB"
                    )
                })
                .map(|child| child.id())
        }
    }
}

pub fn resolve_window(initial_pid: u32, timeout_ms: u64) -> Result<WindowInfo> {
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut candidate_pids = vec![initial_pid];
    let mut discovered_children = false;
    let mut delay = Duration::from_millis(100);

    loop {
        if let Some(window) = find_window_for_pids(&candidate_pids)? {
            return Ok(window);
        }

        if !discovered_children && !winapi_safe::is_process_alive(initial_pid)? {
            let children = winapi_safe::child_processes(initial_pid)?;
            if !children.is_empty() {
                candidate_pids.extend(children);
                discovered_children = true;
            }
        }

        if started.elapsed() >= timeout {
            return Err(anyhow!(
                "timed out after {} ms waiting for a visible window from process {} or its children",
                timeout_ms,
                initial_pid
            ));
        }

        let remaining = timeout.saturating_sub(started.elapsed());
        thread::sleep(delay.min(remaining));
        delay = (delay * 2).min(Duration::from_secs(1));
    }
}

fn find_window_for_pids(candidate_pids: &[u32]) -> Result<Option<WindowInfo>> {
    let windows = winapi_safe::enumerate_visible_windows()
        .map_err(|error| anyhow!("failed to enumerate windows: {error}"))?;

    Ok(windows
        .into_iter()
        .find(|window| candidate_pids.contains(&window.process_id)))
}
