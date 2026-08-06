//! Persist and restore the main desktop window geometry.
//!
//! Geometry is stored in physical pixels under the app config directory so
//! restore can run before the first `show()`. Defaults match `tauri.conf.json`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
};

const STATE_FILE_NAME: &str = "window-state.json";
const STATE_VERSION: u32 = 1;
const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 800.0;
const MIN_WIDTH_LOGICAL: f64 = 960.0;
const MIN_HEIGHT_LOGICAL: f64 = 600.0;
const DEBOUNCE_MS: u64 = 400;
const MAIN_LABEL: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowStateFile {
    version: u32,
    remember: bool,
    /// Physical outer position / size of the last non-maximized frame.
    x: Option<i32>,
    y: Option<i32>,
    width: Option<u32>,
    height: Option<u32>,
    maximized: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale_factor: Option<f64>,
}

impl Default for WindowStateFile {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            remember: true,
            x: None,
            y: None,
            width: None,
            height: None,
            maximized: false,
            scale_factor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowStatePreference {
    remember: bool,
    width: Option<u32>,
    height: Option<u32>,
    maximized: bool,
}

pub(crate) struct WindowStateManager {
    data: Mutex<WindowStateFile>,
    /// Bumped on every schedule/cancel/flush so stale debounce writers stand down.
    save_gen: AtomicU64,
}

impl WindowStateManager {
    fn new(data: WindowStateFile) -> Self {
        Self {
            data: Mutex::new(data),
            save_gen: AtomicU64::new(0),
        }
    }
}

/// Load state from disk and apply it to `main` before any reveal path runs.
/// Must run before `backend::setup`, which can show the window on failure.
pub(crate) fn init_and_restore(app: &AppHandle) {
    let path = state_path(app);
    let data = load_state(path.as_deref()).unwrap_or_default();
    let remember = data.remember;
    app.manage(WindowStateManager::new(data.clone()));

    if !remember {
        return;
    }
    let Some(window) = app.get_webview_window(MAIN_LABEL) else {
        return;
    };
    if let Err(err) = apply_state(&window, &data) {
        log::warn!("[window-state] restore failed: {err}");
    }
}

/// Cancel pending debounced writes and synchronously persist current geometry
/// when remembering is enabled. Safe to call from hide/quit/update exit paths.
pub(crate) fn flush_sync(app: &AppHandle) {
    let Some(manager) = app.try_state::<WindowStateManager>() else {
        return;
    };
    manager.save_gen.fetch_add(1, Ordering::SeqCst);

    let mut data = match manager.data.lock() {
        Ok(guard) => guard,
        Err(err) => {
            log::warn!("[window-state] flush lock poisoned: {err}");
            return;
        }
    };

    if !data.remember {
        // Still persist the remember flag so the next launch stays off.
        if let Err(err) = write_state(state_path(app).as_deref(), &data) {
            log::warn!("[window-state] flush remember=false failed: {err}");
        }
        return;
    }

    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        capture_into(&mut data, &window);
    }
    if let Err(err) = write_state(state_path(app).as_deref(), &data) {
        log::warn!("[window-state] flush failed: {err}");
    }
}

/// Debounced save after move/resize. No-op when remembering is disabled.
///
/// Normal bounds are captured into memory immediately so a quick maximize after
/// resize still keeps the latest non-maximized frame (the delayed disk write may
/// run while already maximized).
pub(crate) fn schedule_save(app: &AppHandle) {
    let Some(manager) = app.try_state::<WindowStateManager>() else {
        return;
    };
    {
        let Ok(mut data) = manager.data.lock() else {
            return;
        };
        if !data.remember {
            return;
        }
        if let Some(window) = app.get_webview_window(MAIN_LABEL) {
            capture_into(&mut data, &window);
        }
    }

    let gen = manager.save_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(DEBOUNCE_MS));
        let Some(manager) = app.try_state::<WindowStateManager>() else {
            return;
        };
        if manager.save_gen.load(Ordering::SeqCst) != gen {
            return;
        }

        let mut data = match manager.data.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if !data.remember {
            return;
        }
        // Refresh maximized flag (and normal bounds if still restored) before disk write.
        if let Some(window) = app.get_webview_window(MAIN_LABEL) {
            capture_into(&mut data, &window);
        }
        if let Err(err) = write_state(state_path(&app).as_deref(), &data) {
            log::warn!("[window-state] debounced save failed: {err}");
        }
    });
}

#[tauri::command]
pub(crate) fn get_window_state_preference(
    app: AppHandle,
) -> Result<WindowStatePreference, String> {
    let manager = app
        .try_state::<WindowStateManager>()
        .ok_or_else(|| "window state is not initialized".to_string())?;
    let data = manager
        .data
        .lock()
        .map_err(|_| "window state lock poisoned".to_string())?;
    Ok(WindowStatePreference {
        remember: data.remember,
        width: data.width,
        height: data.height,
        maximized: data.maximized,
    })
}

#[tauri::command]
pub(crate) fn set_window_state_preference(
    app: AppHandle,
    remember: bool,
) -> Result<(), String> {
    let manager = app
        .try_state::<WindowStateManager>()
        .ok_or_else(|| "window state is not initialized".to_string())?;
    // Cancel any in-flight debounce before mutating preference.
    manager.save_gen.fetch_add(1, Ordering::SeqCst);

    let mut data = manager
        .data
        .lock()
        .map_err(|_| "window state lock poisoned".to_string())?;
    data.remember = remember;
    if remember {
        if let Some(window) = app.get_webview_window(MAIN_LABEL) {
            capture_into(&mut data, &window);
        }
    }
    write_state(state_path(&app).as_deref(), &data)
}

#[tauri::command]
pub(crate) fn reset_window_state(app: AppHandle) -> Result<(), String> {
    let manager = app
        .try_state::<WindowStateManager>()
        .ok_or_else(|| "window state is not initialized".to_string())?;
    manager.save_gen.fetch_add(1, Ordering::SeqCst);

    let window = app
        .get_webview_window(MAIN_LABEL)
        .ok_or_else(|| "main window not found".to_string())?;

    apply_defaults(&window)?;

    let mut data = manager
        .data
        .lock()
        .map_err(|_| "window state lock poisoned".to_string())?;
    // Keep the remember preference; replace geometry with the just-applied defaults.
    capture_into(&mut data, &window);
    data.maximized = false;
    write_state(state_path(&app).as_deref(), &data)
}

fn apply_defaults(window: &WebviewWindow) -> Result<(), String> {
    let _ = window.unmaximize();
    window
        .set_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT))
        .map_err(|err| err.to_string())?;

    let size = window.outer_size().map_err(|err| err.to_string())?;
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let x = work.position.x + (work.size.width as i32 - size.width as i32) / 2;
        let y = work.position.y + (work.size.height as i32 - size.height as i32) / 2;
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn apply_state(window: &WebviewWindow, state: &WindowStateFile) -> Result<(), String> {
    let scale = window.scale_factor().unwrap_or(1.0).max(0.5);
    let min_w = (MIN_WIDTH_LOGICAL * scale).round().max(1.0) as u32;
    let min_h = (MIN_HEIGHT_LOGICAL * scale).round().max(1.0) as u32;

    let width = state.width.unwrap_or(0).max(min_w);
    let height = state.height.unwrap_or(0).max(min_h);
    if state.width.is_none() || state.height.is_none() {
        // No stored geometry — leave tauri.conf defaults in place.
        if state.maximized {
            let _ = window.maximize();
        }
        return Ok(());
    }

    // Always apply normal bounds first so unmaximize after a later restore
    // still has a sensible frame.
    window
        .set_size(PhysicalSize::new(width, height))
        .map_err(|err| err.to_string())?;

    if let (Some(x), Some(y)) = (state.x, state.y) {
        if is_position_visible(window, x, y, width, height) {
            window
                .set_position(PhysicalPosition::new(x, y))
                .map_err(|err| err.to_string())?;
        } else {
            // Size is usable; re-center so the window is not lost off-screen.
            let size = PhysicalSize::new(width, height);
            center_on_primary(window, size)?;
        }
    }

    if state.maximized {
        let _ = window.maximize();
    }
    Ok(())
}

fn center_on_primary(window: &WebviewWindow, size: PhysicalSize<u32>) -> Result<(), String> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let work = monitor.work_area();
    let x = work.position.x + (work.size.width as i32 - size.width as i32) / 2;
    let y = work.position.y + (work.size.height as i32 - size.height as i32) / 2;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|err| err.to_string())
}

fn capture_into(state: &mut WindowStateFile, window: &WebviewWindow) {
    let maximized = window.is_maximized().unwrap_or(false);
    let scale = window.scale_factor().ok();
    state.scale_factor = scale;
    state.maximized = maximized;

    // While maximized, outer bounds are the work area — keep the last normal frame.
    if maximized {
        return;
    }

    if let Ok(pos) = window.outer_position() {
        state.x = Some(pos.x);
        state.y = Some(pos.y);
    }
    if let Ok(size) = window.outer_size() {
        let scale = scale.unwrap_or(1.0).max(0.5);
        let min_w = (MIN_WIDTH_LOGICAL * scale).round().max(1.0) as u32;
        let min_h = (MIN_HEIGHT_LOGICAL * scale).round().max(1.0) as u32;
        state.width = Some(size.width.max(min_w));
        state.height = Some(size.height.max(min_h));
    }
}

/// True when the window center lies inside any monitor's work area.
fn is_position_visible(
    window: &WebviewWindow,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> bool {
    let monitors = match window.available_monitors() {
        Ok(list) if !list.is_empty() => list,
        _ => return true, // fail open when monitor list is unavailable
    };
    let cx = x + width as i32 / 2;
    let cy = y + height as i32 / 2;
    monitors.iter().any(|monitor| {
        let work = monitor.work_area();
        let left = work.position.x;
        let top = work.position.y;
        let right = left + work.size.width as i32;
        let bottom = top + work.size.height as i32;
        cx >= left && cx < right && cy >= top && cy < bottom
    })
}

fn state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(STATE_FILE_NAME))
}

fn load_state(path: Option<&Path>) -> Option<WindowStateFile> {
    let path = path?;
    let raw = std::fs::read_to_string(path).ok()?;
    let parsed: WindowStateFile = serde_json::from_str(&raw).ok()?;
    if parsed.version != STATE_VERSION {
        return None;
    }
    Some(sanitize_state(parsed))
}

fn write_state(path: Option<&Path>, state: &WindowStateFile) -> Result<(), String> {
    let path = path.ok_or_else(|| "cannot resolve app config directory".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|err| err.to_string())?;
    if let Err(err) = std::fs::rename(&tmp, path) {
        // Windows may refuse rename-over-existing; fall back to replace.
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp, path).map_err(|rename_err| {
            format!("failed to persist window state ({err} / {rename_err})")
        })?;
    }
    Ok(())
}

fn sanitize_state(mut state: WindowStateFile) -> WindowStateFile {
    state.version = STATE_VERSION;
    if let Some(width) = state.width {
        if width == 0 {
            state.width = None;
        }
    }
    if let Some(height) = state.height {
        if height == 0 {
            state.height = None;
        }
    }
    state
}

/// Pure helpers used by unit tests (and validation above).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_remembers_window() {
        let state = WindowStateFile::default();
        assert!(state.remember);
        assert!(!state.maximized);
        assert!(state.width.is_none());
    }

    #[test]
    fn serde_roundtrip_preserves_physical_bounds() {
        let state = WindowStateFile {
            version: STATE_VERSION,
            remember: true,
            x: Some(120),
            y: Some(80),
            width: Some(1440),
            height: Some(900),
            maximized: false,
            scale_factor: Some(2.0),
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: WindowStateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }

    #[test]
    fn sanitize_drops_zero_dimensions() {
        let state = sanitize_state(WindowStateFile {
            version: 99,
            remember: true,
            x: Some(0),
            y: Some(0),
            width: Some(0),
            height: Some(800),
            maximized: false,
            scale_factor: None,
        });
        assert_eq!(state.version, STATE_VERSION);
        assert!(state.width.is_none());
        assert_eq!(state.height, Some(800));
    }

    #[test]
    fn load_rejects_unknown_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        std::fs::write(
            &path,
            r#"{"version":999,"remember":true,"maximized":false}"#,
        )
        .unwrap();
        assert!(load_state(Some(&path)).is_none());
    }

    #[test]
    fn write_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let state = WindowStateFile {
            version: STATE_VERSION,
            remember: false,
            x: Some(10),
            y: Some(20),
            width: Some(1100),
            height: Some(700),
            maximized: true,
            scale_factor: Some(1.5),
        };
        write_state(Some(&path), &state).unwrap();
        let loaded = load_state(Some(&path)).unwrap();
        assert_eq!(loaded.remember, false);
        assert_eq!(loaded.width, Some(1100));
        assert!(loaded.maximized);
    }

    #[test]
    fn center_point_visibility_math() {
        // work area: (0,0)-(1920,1080); window at (-500,-500) 200x200 → center off
        let cx = -500 + 200 / 2;
        let cy = -500 + 200 / 2;
        assert!(cx < 0 || cy < 0);
        // window fully on screen
        let cx2 = 100 + 800 / 2;
        let cy2 = 100 + 600 / 2;
        assert!(cx2 >= 0 && cx2 < 1920 && cy2 >= 0 && cy2 < 1080);
    }
}
