use anyhow::{anyhow, Result};
use serde::Serialize;
use thiserror::Error;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, RECT, STILL_ACTIVE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW, MONITORINFO,
};

const MONITORINFOF_PRIMARY: u32 = 1;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};

#[derive(Clone, Debug, Serialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowInfo {
    pub hwnd_value: isize,
    pub process_id: u32,
    pub title: String,
    pub class_name: String,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MonitorInfo {
    pub index: u32,
    pub is_primary: bool,
    pub device_name: String,
    pub rc_monitor: Rect,
    pub rc_work: Rect,
}

#[derive(Clone, Debug, Error)]
pub enum WinApiError {
    #[error("EnumWindows failed")]
    EnumerationFailed,
    #[error("GetWindowRect failed")]
    GetWindowRectFailed,
    #[error("CreateToolhelp32Snapshot failed")]
    ProcessSnapshotFailed,
    #[error("Process snapshot iteration failed")]
    ProcessIterationFailed,
    #[error("GetExitCodeProcess failed for PID {0}")]
    GetExitCodeProcessFailed(u32),
}

struct EnumerationContext {
    windows: Vec<WindowInfo>,
    error: Option<WinApiError>,
}

struct MonitorEnumerationContext {
    monitors: Vec<(HMONITOR, MONITORINFOEXW)>,
}

pub fn enumerate_visible_windows() -> Result<Vec<WindowInfo>, WinApiError> {
    let mut context = EnumerationContext {
        windows: Vec::new(),
        error: None,
    };

    // SAFETY: context remains valid for the synchronous EnumWindows call.
    let result = unsafe {
        EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut context as *mut _ as isize),
        )
    };

    if let Some(error) = context.error {
        return Err(error);
    }
    if result.is_err() {
        return Err(WinApiError::EnumerationFailed);
    }
    Ok(context.windows)
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam points to the live EnumerationContext supplied above.
    let context = unsafe { &mut *(lparam.0 as *mut EnumerationContext) };
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    match window_info(hwnd) {
        Ok(info) => context.windows.push(info),
        Err(error) => context.error = Some(error),
    }
    if context.error.is_some() { BOOL(0) } else { BOOL(1) }
}

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    let mut context = MonitorEnumerationContext { monitors: Vec::new() };

    // SAFETY: context remains valid for the synchronous EnumDisplayMonitors call; no HDC is
    // acquired because both the HDC and clipping rectangle parameters are None.
    let result = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitors_callback),
            LPARAM(&mut context as *mut _ as isize),
        )
    };
    if !result.as_bool() {
        return Err(anyhow!("EnumDisplayMonitors failed"));
    }

    let mut primary = Vec::new();
    let mut secondary = Vec::new();
    for entry in context.monitors {
        if entry.1.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0 {
            primary.push(entry);
        } else {
            secondary.push(entry);
        }
    }

    let mut ordered = primary;
    ordered.extend(secondary);
    Ok(ordered
        .into_iter()
        .enumerate()
        .map(|(index, (_, info))| MonitorInfo {
            index: index as u32 + 1,
            is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            device_name: String::from_utf16_lossy(
                &info.szDevice[..info.szDevice.iter().position(|value| *value == 0).unwrap_or(info.szDevice.len())],
            ),
            rc_monitor: rect_from_win32(info.monitorInfo.rcMonitor),
            rc_work: rect_from_win32(info.monitorInfo.rcWork),
        })
        .collect())
}

unsafe extern "system" fn enum_monitors_callback(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    // SAFETY: lparam points to the live MonitorEnumerationContext supplied above.
    let context = unsafe { &mut *(lparam.0 as *mut MonitorEnumerationContext) };
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    // SAFETY: info has the required cbSize and is a valid mutable output buffer.
    if !unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo as *mut _) }.as_bool() {
        tracing::warn!("GetMonitorInfoW failed for one monitor; omitting it");
        return BOOL(1);
    }
    context.monitors.push((monitor, info));
    BOOL(1)
}

pub fn is_process_alive(pid: u32) -> Result<bool, WinApiError> {
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(handle) => handle,
        Err(_) => return Ok(false),
    };
    let mut exit_code = 0u32;
    let result = unsafe { GetExitCodeProcess(process, &mut exit_code) };
    let _ = unsafe { CloseHandle(process) };
    result.map_err(|_| WinApiError::GetExitCodeProcessFailed(pid))?;
    Ok(exit_code == STILL_ACTIVE.0 as u32)
}

pub fn child_processes(parent_process_id: u32) -> Result<Vec<u32>, WinApiError> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|_| WinApiError::ProcessSnapshotFailed)?;
    let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut children = Vec::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
        let _ = unsafe { CloseHandle(snapshot) };
        return Err(WinApiError::ProcessIterationFailed);
    }
    loop {
        if entry.th32ParentProcessID == parent_process_id { children.push(entry.th32ProcessID); }
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() { break; }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    Ok(children)
}

fn window_info(hwnd: HWND) -> Result<WindowInfo, WinApiError> {
    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)); }
    let title = get_window_string(hwnd, true);
    let class_name = get_window_string(hwnd, false);
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return Err(WinApiError::GetWindowRectFailed);
    }
    Ok(WindowInfo {
        hwnd_value: hwnd.0 as isize,
        process_id,
        title,
        class_name,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

fn get_window_string(hwnd: HWND, title: bool) -> String {
    let mut buffer = [0u16; 1024];
    let length = unsafe {
        if title { GetWindowTextW(hwnd, &mut buffer) } else { GetClassNameW(hwnd, &mut buffer) }
    };
    String::from_utf16_lossy(&buffer[..length as usize])
}

fn rect_from_win32(rect: RECT) -> Rect {
    Rect { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }
}
