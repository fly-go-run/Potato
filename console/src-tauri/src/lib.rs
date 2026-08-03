//! Tauri desktop entry point and plugin/command registration.

mod backend;
mod backend_download;
mod external_link;
mod updates;
mod tray;
#[cfg(target_os = "macos")]
mod macos_icon;

use tauri::{Manager, RunEvent, WebviewWindow, WindowEvent};

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
    if state.claim_frontend_reveal() {
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
    // misleading blank QwenPaw window behind the real Potato window.
    let mut builder = tauri::Builder::default();
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
            updates::check_desktop_update,
            updates::install_desktop_update,
            updates::download_desktop_update,
            updates::install_downloaded_update,
            updates::check_cached_update,
            tray::minimize_to_tray,
            tray::quit_app,
            tray::set_tray_labels,
            tray::ack_close,
        ])
        .manage(backend::BackendState::default())
        .manage(tray::TrayState::default())
        .setup(|app| {
            backend::setup(app)?;
            tray::setup(app)?;
            #[cfg(target_os = "macos")]
            if let Some(theme) = app
                .get_webview_window("main")
                .and_then(|window| window.theme().ok())
            {
                macos_icon::set(theme);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if let WindowEvent::ThemeChanged(theme) = event {
                macos_icon::set(*theme);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                tray::request_close(window.app_handle());
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
                    if let Err(err) = tauri::async_runtime::block_on(backend::stop_and_wait(app_handle)) {
                        log::warn!("[backend] graceful shutdown did not complete: {err}");
                    }
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
            eprintln!("[QwenPaw Desktop] Fatal startup error: {err}");
            std::process::exit(1);
        }
    }
}
