use anyhow::{anyhow, Result};

use crate::config::PositionSpec;
use crate::winapi_safe::{MonitorInfo, Rect};

pub fn resolve_absolute_rect(spec: &PositionSpec, monitors: &[MonitorInfo]) -> Result<Rect> {
    let monitor = match spec.monitor {
        Some(index) => monitors
            .iter()
            .find(|monitor| monitor.index == index)
            .ok_or_else(|| {
                anyhow!(
                    "monitor {} is not available; {} monitor(s) detected",
                    index,
                    monitors.len()
                )
            })?,
        None => monitors
            .iter()
            .find(|monitor| monitor.is_primary)
            .ok_or_else(|| anyhow!("no primary monitor is available"))?,
    };

    let rect = Rect {
        left: monitor.rc_work.left + spec.x,
        top: monitor.rc_work.top + spec.y,
        right: monitor.rc_work.left + spec.x + spec.width,
        bottom: monitor.rc_work.top + spec.y + spec.height,
    };

    if rect.left < monitor.rc_work.left
        || rect.top < monitor.rc_work.top
        || rect.right > monitor.rc_work.right
        || rect.bottom > monitor.rc_work.bottom
    {
        tracing::warn!(
            monitor = monitor.index,
            rect_left = rect.left,
            rect_top = rect.top,
            rect_right = rect.right,
            rect_bottom = rect.bottom,
            "requested window rectangle extends outside the selected monitor work area"
        );
    }

    Ok(rect)
}
