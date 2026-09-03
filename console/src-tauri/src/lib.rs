//! Tauri desktop entry point and plugin/command registration.

mod backend;
mod backend_download;
mod external_link;
mod local_file;
mod updates;
mod tray;
mod window_state;
#[cfg(target_os = "macos")]
mod macos_icon;
#[cfg(target_os = "macos")]
mod macos_traffic_lights;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent, WebviewWindow, WindowEvent};

/// Whether the launch reveal (bundled app first paint) has already happened.
/// Once true, no later automatic path (frontend_ready after a backend
/// restart, the startup watchdog) may show or focus the window again —
/// the user has seen the app and may have deliberately hidden it.
pub(crate) static INITIAL_REVEAL_DONE: AtomicBool = AtomicBool::new(false);

/// Launched by the OS login item with `--background`: start the sidecar so
/// it is warm, but never reveal the window on our own. The user brings it up
/// from the tray, the Dock, or by launching Potato again (single-instance
/// callback). This turns the Windows cold start into a login-time prewarm.
const BACKGROUND_LAUNCH_ARG: &str = "--background";
/// Written once the login item has been registered on first launch, so a
/// user who later turns it off in Settings is not re-enabled.
const AUTOSTART_INIT_MARKER: &str = "autostart-initialized";

/// Register the login item on the very first launch. Errors are logged, not
/// fatal: the app works without autostart, it just cold-starts on click.
fn ensure_autostart_default(app: &tauri::AppHandle) {
    // Never register a `tauri dev` binary as a login item.
    if cfg!(debug_assertions) {
        return;
    }
    use tauri_plugin_autostart::ManagerExt;
    let Ok(dir) = app.path().app_data_dir() else { return };
    let marker = dir.join(AUTOSTART_INIT_MARKER);
    if marker.exists() {
        return;
    }
    if let Err(err) = std::fs::create_dir_all(&dir) {
        log::warn!("[autostart] cannot create app data dir: {err}");
        return;
    }
    match app.autolaunch().enable() {
        Ok(()) => log::info!("[autostart] login item registered (first launch)"),
        Err(err) => log::warn!("[autostart] failed to register login item: {err}"),
    }
    let _ = std::fs::write(&marker, b"1");
}

/// Opens the WebView DevTools. Gated by the hidden 8-click logo gesture in the
/// frontend so end users cannot open DevTools via the default context menu or
/// keyboard shortcuts in production builds.
#[tauri::command]
fn open_devtools(window: WebviewWindow) {
    window.open_devtools();
}

/// Reveal the native window only after the real React app has completed auth
/// and its first data initialization. A WebView page-load event is too early:
/// it can still paint the temporary auth-checking state on a slow cold start.
#[tauri::command]
fn frontend_ready(window: WebviewWindow, state: tauri::State<'_, backend::BackendState>) {
    if state.claim_frontend_reveal()
        && !INITIAL_REVEAL_DONE.swap(true, Ordering::SeqCst)
    {
        #[cfg(target_os = "macos")]
        macos_traffic_lights::align(&window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Build the desktop app, wire native plugins/commands, and stop the backend on exit.
pub fn run() {
    // Keep one desktop shell per app identifier. Without this guard, launching
    // a second copy starts another backend on the stable desktop port; the old
    // WebView can remain visible after its backend is reclaimed, leaving a
    // misleading blank Potato window behind the real Potato window.
    let background = std::env::args().skip(1).any(|arg| arg == BACKGROUND_LAUNCH_ARG);
    if background {
        // No automatic reveal path may show the window in this mode.
        INITIAL_REVEAL_DONE.store(true, Ordering::SeqCst);
    }
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec![BACKGROUND_LAUNCH_ARG]),
    ));
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    let build_result = builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_updater::Builder::new()
                .default_version_comparator(updates::is_remote_update_newer)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            frontend_ready,
            backend_download::download_backend_file,
            backend_download::read_workspace_binary_file,
            backend::backend_port,
            backend::backend_startup_error,
            backend::restart_backend,
            external_link::open_external_link,
            local_file::open_local_path,
            local_file::reveal_local_path,
            updates::check_desktop_update,
            updates::install_desktop_update,
            updates::download_desktop_update,
            updates::install_downloaded_update,
            updates::check_cached_update,
            tray::minimize_to_tray,
            tray::quit_app,
            tray::set_tray_labels,
            tray::ack_close,
            window_state::get_window_state_preference,
            window_state::set_window_state_preference,
            window_state::reset_window_state,
        ])
        .manage(backend::BackendState::default())
        .manage(tray::TrayState::default())
        .setup(move |app| {
            // Restore geometry before backend::setup: a sidecar failure path
            // calls show_main_window synchronously and would otherwise flash
            // the conf defaults.
            window_state::init_and_restore(&app.handle());
            backend::setup(app)?;
            tray::setup(app)?;
            ensure_autostart_default(&app.handle());
            if background {
                log::info!("[autostart] background launch: sidecar prewarm, window stays hidden");
            }
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                macos_traffic_lights::align(&window);
                if let Ok(theme) = window.theme() {
                    macos_icon::set(theme);
                }
            }
            Ok(())
        })
        .on_page_load(|webview, payload| {
            // Reveal as soon as the bundled Potato app has painted. The
            // sidecar keeps starting in the background; waiting for it
            // would leave the window invisible for the whole cold start.
            // Launch-time activation is user-initiated, so macOS grants
            // focus here. Claim the startup reveal so later
            // `frontend_ready` / watchdog paths never yank focus again.
            if !matches!(payload.event(), PageLoadEvent::Finished) {
                return;
            }
            if INITIAL_REVEAL_DONE.swap(true, Ordering::SeqCst) {
                return;
            }
            let app = webview.app_handle();
            let _ = app
                .state::<backend::BackendState>()
                .claim_frontend_reveal();
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                macos_traffic_lights::align(&window);
                let _ = window.show();
                let _ = window.set_focus();
            }
        })
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if let WindowEvent::ThemeChanged(theme) = event {
                macos_icon::set(*theme);
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    tray::request_close(window.app_handle());
                }
                WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
                    #[cfg(target_os = "macos")]
                    macos_traffic_lights::align_main(window.app_handle());
                    window_state::schedule_save(window.app_handle());
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!());

    match build_result {
        Ok(app) => {
            app.run(|app_handle, event| match event {
                // `code` is `None` only for OS-initiated quits (e.g. macOS
                // Cmd+Q / app menu Quit). On macOS we route those through the
                // same close prompt as the window's red button, so the choice
                // (minimize-to-tray vs. quit) stays consistent with Windows
                // Alt+F4. Programmatic exits from `quit_app` carry a `code` and
                // fall through to the normal shutdown path below.
                RunEvent::ExitRequested { api, code, .. } => {
                    #[cfg(target_os = "macos")]
                    if code.is_none() {
                        api.prevent_exit();
                        // The window may be hidden in the tray; bring it back so
                        // the close prompt is actually visible before asking.
                        tray::show_main_window(app_handle);
                        tray::request_close(app_handle);
                        return;
                    }
                    #[cfg(not(target_os = "macos"))]
                    let _ = (&api, &code);
                    window_state::flush_sync(app_handle);
                    // Leave the sidecar running so the next launch can adopt
                    // it instead of paying another packaged cold start.
                    // Updates and restart_backend still stop it explicitly.
                }
                // macOS emits this when the user clicks the Dock icon. Without
                // it, a window hidden via "minimize to tray" can only be
                // restored from the menu-bar icon, leaving a dead Dock icon.
                #[cfg(target_os = "macos")]
                RunEvent::Reopen { .. } => {
                    tray::show_main_window(app_handle);
                }
                _ => {}
            });
        }
        Err(err) => {
            eprintln!("[Potato Desktop] Fatal startup error: {err}");
            std::process::exit(1);
        }
    }
}
