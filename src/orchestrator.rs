use std::thread;
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

use crate::config::{AppSpec, Profile};
use crate::{launcher, monitor_resolver, positioner, winapi_safe};

#[derive(Debug, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Serialize)]
pub struct AppResult {
    pub name: String,
    pub status: String,
    pub pid: Option<u32>,
    pub applied_position: Option<Rect>,
    pub error: Option<String>,
}

pub fn run_profile(profile: &Profile) -> Vec<AppResult> {
    let monitors = match winapi_safe::enumerate_monitors() {
        Ok(monitors) => monitors,
        Err(error) => {
            return profile
                .apps
                .iter()
                .map(|app| AppResult::error(app.name.clone(), format!("{:#?}", error)))
                .collect();
        }
    };

    let handles: Vec<_> = profile
        .apps
        .iter()
        .map(|app| {
            let app = app.clone();
            let monitors = monitors.clone();
            thread::spawn(move || run_app(&app, &monitors).map_err(|error| error))
        })
        .collect();

    handles
        .into_iter()
        .enumerate()
        .map(|(index, handle)| match handle.join() {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => AppResult::error(
                profile
                    .apps
                    .get(index)
                    .map(|app| app.name.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                format!("{:#?}", error),
            ),
            Err(payload) => AppResult::error(
                profile
                    .apps
                    .get(index)
                    .map(|app| app.name.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                panic_message(payload),
            ),
        })
        .collect()
}

fn run_app(app: &AppSpec, monitors: &[winapi_safe::MonitorInfo]) -> Result<AppResult> {
    if app.launch_delay_ms > 0 {
        thread::sleep(Duration::from_millis(app.launch_delay_ms));
    }

    let pid = launcher::spawn_detached(&app.path, &app.args)?;
    let window = launcher::resolve_window(pid, app.timeout_ms)?;
    let applied_position = app.position.as_ref().map_or(Ok(None), |position| {
        let absolute = monitor_resolver::resolve_absolute_rect(position, monitors)?;
        positioner::set_window_position(
            window.hwnd_value,
            absolute.left,
            absolute.top,
            absolute.right - absolute.left,
            absolute.bottom - absolute.top,
        )
        .map(|rect| {
            Some(Rect {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            })
        })
    })?;

    Ok(AppResult {
        name: app.name.clone(),
        status: "success".to_string(),
        pid: Some(window.process_id),
        applied_position,
        error: None,
    })
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "el hilo de lanzamiento entró en pánico".to_string()
    }
}

impl AppResult {
    fn error(name: String, error: String) -> Self {
        Self {
            name,
            status: "error".to_string(),
            pid: None,
            applied_position: None,
            error: Some(error),
        }
    }
}
