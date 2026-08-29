use serde::Serialize;
use thiserror::Error;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, RECT, STILL_ACTIVE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

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

#[derive(Debug, Clone)]
struct EnumerationContext {
    windows: Vec<WindowInfo>,
    error: Option<WinApiError>,
}

/// Enumerates visible top-level windows and returns ownership-safe metadata.
pub fn enumerate_visible_windows() -> Result<Vec<WindowInfo>, WinApiError> {
    let mut context = EnumerationContext {
        windows: Vec::new(),
        error: None,
    };

    // SAFETY: `context` remains alive for the synchronous EnumWindows call. The callback
    // receives the pointer we supplied and never stores it beyond that call.
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
    // SAFETY: The caller provides a valid pointer to EnumerationContext for the duration
    // of EnumWindows, and this callback is invoked synchronously by that API.
    let context = unsafe { &mut *(lparam.0 as *mut EnumerationContext) };

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    match window_info(hwnd) {
        Ok(info) => context.windows.push(info),
        Err(error) => context.error = Some(error),
    }

    if context.error.is_some() {
        BOOL(0)
    } else {
        BOOL(1)
    }
}

pub fn is_process_alive(pid: u32) -> Result<bool, WinApiError> {
    // SAFETY: The requested access is read-only and the PID is supplied by the caller.
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return Ok(false),
    };

    let mut exit_code = 0u32;
    // SAFETY: `process` is a valid handle returned by OpenProcess and `exit_code` is writable.
    let result = unsafe { GetExitCodeProcess(process, &mut exit_code) };
    // SAFETY: The handle is owned by this function and is closed exactly once.
    let _ = unsafe { CloseHandle(process) };

    result
        .map_err(|_| WinApiError::GetExitCodeProcessFailed(pid))?;
    Ok(exit_code == STILL_ACTIVE.0 as u32)
}

pub fn child_processes(parent_process_id: u32) -> Result<Vec<u32>, WinApiError> {
    // SAFETY: This requests a process snapshot owned by this function.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|_| WinApiError::ProcessSnapshotFailed)?;

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut children = Vec::new();

    // SAFETY: `snapshot` is valid and `entry` has its required size initialized.
    let first_result = unsafe { Process32FirstW(snapshot, &mut entry) };
    if first_result.is_err() {
        // SAFETY: The snapshot handle is owned by this function.
        let _ = unsafe { CloseHandle(snapshot) };
        return Err(WinApiError::ProcessIterationFailed);
    }

    loop {
        if entry.th32ParentProcessID == parent_process_id {
            children.push(entry.th32ProcessID);
        }

        // SAFETY: `snapshot` and `entry` remain valid for the iteration.
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }

    // SAFETY: The snapshot handle is owned by this function and is closed exactly once.
    let _ = unsafe { CloseHandle(snapshot) };
    Ok(children)
}

fn window_info(hwnd: HWND) -> Result<WindowInfo, WinApiError> {
    let mut process_id = 0u32;
    // SAFETY: `hwnd` is supplied by EnumWindows and `process_id` is a valid output pointer.
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }

    let title = get_window_string(hwnd, true);
    let class_name = get_window_string(hwnd, false);
    let mut rect = RECT::default();

    // SAFETY: `hwnd` is supplied by EnumWindows and `rect` is a valid output pointer.
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

    // SAFETY: `buffer` is writable and remains valid for the duration of the synchronous call.
    let length = unsafe {
        if title {
            GetWindowTextW(hwnd, &mut buffer)
        } else {
            GetClassNameW(hwnd, &mut buffer)
        }
    };

    String::from_utf16_lossy(&buffer[..length as usize])
}
