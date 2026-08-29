use serde::Serialize;
use thiserror::Error;
use windows::core::PWSTR;
use windows::Win32::Foundation::{BOOL, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, HWND,
};

#[derive(Debug, Serialize)]
pub struct WindowInfo {
    pub process_id: u32,
    pub title: String,
    pub class_name: String,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Error)]
pub enum WinApiError {
    #[error("EnumWindows failed")]
    EnumerationFailed,
    #[error("GetWindowRect failed")]
    GetWindowRectFailed,
}

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
            windows::Win32::Foundation::LPARAM(&mut context as *mut _ as isize),
        )
    };

    if let Some(error) = context.error {
        return Err(error);
    }

    if !result.as_bool() {
        return Err(WinApiError::EnumerationFailed);
    }

    Ok(context.windows)
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: isize) -> BOOL {
    // SAFETY: The caller provides a valid pointer to EnumerationContext for the duration
    // of EnumWindows, and this callback is invoked synchronously by that API.
    let context = unsafe { &mut *(lparam as *mut EnumerationContext) };

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
    if !unsafe { GetWindowRect(hwnd, &mut rect) }.as_bool() {
        return Err(WinApiError::GetWindowRectFailed);
    }

    Ok(WindowInfo {
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
    let mut buffer = vec![0u16; 1024];

    // SAFETY: `buffer` is writable, has the declared capacity, and its pointer is valid for
    // the duration of the synchronous Win32 call.
    let length = unsafe {
        if title {
            GetWindowTextW(hwnd, PWSTR(buffer.as_mut_ptr()), buffer.len() as i32)
        } else {
            GetClassNameW(hwnd, PWSTR(buffer.as_mut_ptr()), buffer.len() as i32)
        }
    };

    String::from_utf16_lossy(&buffer[..length as usize])
}
