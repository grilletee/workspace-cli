use std::ffi::c_void;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, IsIconic, IsZoomed, SetWindowPos, ShowWindow, SWP_NOZORDER, SWP_SHOWWINDOW,
    SW_RESTORE,
};

pub fn set_window_position(
    hwnd_value: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<RECT> {
    let hwnd = HWND(hwnd_value as *mut c_void);

    // SAFETY: The numeric handle originated from a previously enumerated HWND. All calls are
    // synchronous and the handle is never stored outside this function.
    unsafe {
        if IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        SetWindowPos(
            hwnd,
            HWND::default(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_SHOWWINDOW,
        )
        .context("SetWindowPos failed")?;
    }

    let mut rect = RECT::default();
    // SAFETY: `rect` is a valid output buffer and `hwnd` is the handle being positioned.
    unsafe {
        GetWindowRect(hwnd, &mut rect).context("GetWindowRect after positioning failed")?;
    }

    Ok(rect)
}
