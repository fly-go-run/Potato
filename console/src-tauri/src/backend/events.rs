//! Sidecar process event handling and stderr capture.

use std::time::Duration;

use serde::Deserialize;
use tauri::Manager;
use tauri_plugin_shell::process::{CommandEvent, TerminatedPayload};
use tokio::sync::watch;

use super::BackendState;
use crate::tray;

const MAX_CAPTURED_STDERR_CHARS: usize = 4000;
const STDERR_TRUNCATION_MARKER: &str = "\n[...stderr truncated...]\n";
const BACKEND_READY_PREFIX: &str = "POTATO_BACKEND_READY ";
const BACKEND_HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const BACKEND_HEALTH_TIMEOUT: Duration = Duration::from_secs(180);
const BACKEND_HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Deserialize)]
struct BackendReadyPayload {
    port: u16,
}

/// Watches sidecar output and reports failures for the current process generation.
pub(super) fn watch(
    app: tauri::AppHandle,
    generation: u64,
    mut rx: tauri::async_runtime::Receiver<CommandEvent>,
    terminated: watch::Sender<bool>,
) {
    tauri::async_runtime::spawn(async move {
        let mut last_stderr = String::new();
        log::info!("[backend] watching process generation={generation}");
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let text = String::from_utf8_lossy(&line);
                    log::info!("[backend:{generation}] stdout: {}", text.trim_end());
                    if let Some(port) = ready_port_from_stdout(&text) {
                        log::info!("[backend:{generation}] ready port={port}");
                        let state = app.state::<BackendState>();
                        if state.is_current(generation) {
                            state.set_port_if_current(generation, port);
                            confirm_backend_health(app.clone(), generation, port);
                        }
                    }
                }
                CommandEvent::Stderr(line) => {
                    record_stderr(generation, &mut last_stderr, &line);
                }
                CommandEvent::Error(message) => {
                    log::error!("[backend:{generation}] process event error: {message}");
                    app.state::<BackendState>().set_error_if_current(
                        generation,
                        format!("backend process error: {message}"),
                    );
                    if app
                        .state::<BackendState>()
                        .claim_frontend_reveal_if_current(generation)
                    {
                        tray::show_main_window(&app);
                    }
                }
                CommandEvent::Terminated(payload) => {
                    let message = termination_message(payload, &last_stderr);
                    let state = app.state::<BackendState>();
                    let stopping = !state.is_current(generation);
                    terminated.send_replace(true);
                    if stopping {
                        log::info!(
                            "[backend:{generation}] process terminated after shutdown request"
                        );
                    } else {
                        log::warn!("[backend:{generation}] {message}");
                        state.set_error_if_current(generation, message);
                        if state.claim_frontend_reveal_if_current(generation) {
                            tray::show_main_window(&app);
                        }
                    }
                }
                _ => {}
            }
        }

        log::warn!("[backend:{generation}] process event stream closed");
        app.state::<BackendState>()
            .clear_child_if_current(generation);
    });
}

/// Confirm the sidecar answers HTTP after it prints the ready line.
/// The WebView already shows the bundled Potato app; do not navigate away.
fn confirm_backend_health(app: tauri::AppHandle, generation: u64, port: u16) {
    tauri::async_runtime::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(BACKEND_HEALTH_REQUEST_TIMEOUT)
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                report_backend_startup_failure(
                    &app,
                    generation,
                    format!("failed to create backend health client: {err}"),
                );
                return;
            }
        };
        let health_url = format!("http://127.0.0.1:{port}/api/version");
        let deadline = tokio::time::Instant::now() + BACKEND_HEALTH_TIMEOUT;

        loop {
            if !app.state::<BackendState>().is_current(generation) {
                return;
            }

            if client
                .get(&health_url)
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                log::info!("[backend:{generation}] healthy port={port}");
                return;
            }

            if tokio::time::Instant::now() >= deadline {
                report_backend_startup_failure(
                    &app,
                    generation,
                    format!(
                        "backend did not become healthy within {} seconds",
                        BACKEND_HEALTH_TIMEOUT.as_secs()
                    ),
                );
                return;
            }
            tokio::time::sleep(BACKEND_HEALTH_POLL_INTERVAL).await;
        }
    });
}

fn report_backend_startup_failure(app: &tauri::AppHandle, generation: u64, message: String) {
    log::error!("[backend:{generation}] {message}");
    app.state::<BackendState>()
        .set_error_if_current(generation, message);
    if !crate::INITIAL_REVEAL_DONE.load(std::sync::atomic::Ordering::SeqCst) {
        tray::show_main_window(app);
    }
}

fn ready_port_from_stdout(text: &str) -> Option<u16> {
    text.lines().find_map(|line| {
        let payload = line.trim().strip_prefix(BACKEND_READY_PREFIX)?;
        serde_json::from_str::<BackendReadyPayload>(payload)
            .ok()
            .map(|ready| ready.port)
    })
}

fn record_stderr(generation: u64, buffer: &mut String, line: &[u8]) {
    let text = String::from_utf8_lossy(line).to_string();
    log::error!("[backend:{generation}] stderr: {text}");
    buffer.push_str(&text);
    trim_captured_stderr(buffer);
}

fn trim_captured_stderr(text: &mut String) {
    let total = text.chars().count();
    if total <= MAX_CAPTURED_STDERR_CHARS {
        return;
    }

    let marker_len = STDERR_TRUNCATION_MARKER.chars().count();
    let keep_chars = MAX_CAPTURED_STDERR_CHARS.saturating_sub(marker_len);
    let head_chars = keep_chars / 2;
    let tail_chars = keep_chars - head_chars;
    let head = first_chars(text, head_chars);
    let tail = last_chars(text, tail_chars);
    *text = format!("{head}{STDERR_TRUNCATION_MARKER}{tail}");
}

fn first_chars(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

fn last_chars(text: &str, count: usize) -> String {
    let mut chars = text.chars().rev().take(count).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn termination_message(payload: TerminatedPayload, last_stderr: &str) -> String {
    let mut message = match (payload.code, payload.signal) {
        (Some(code), _) => format!("backend process exited unexpectedly with code {code}"),
        (_, Some(signal)) => format!("backend process exited unexpectedly by signal {signal}"),
        _ => "backend process exited unexpectedly".to_string(),
    };

    let stderr = last_stderr.trim();
    if !stderr.is_empty() {
        message.push_str("\n\nLast stderr:\n");
        message.push_str(stderr);
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_captured_stderr_preserves_head_and_tail() {
        let mut text = format!("{}middle{}", "head".repeat(1200), "tail".repeat(1200));

        trim_captured_stderr(&mut text);

        assert!(text.chars().count() <= MAX_CAPTURED_STDERR_CHARS);
        assert!(text.starts_with("head"));
        assert!(text.contains(STDERR_TRUNCATION_MARKER));
        assert!(text.ends_with("tail"));
        assert!(!text.contains("middle"));
    }

    #[test]
    fn ready_port_from_stdout_parses_protocol_line() {
        let text = "INFO before\nPOTATO_BACKEND_READY {\"port\":54321}\n";

        assert_eq!(ready_port_from_stdout(text), Some(54321));
    }

    #[test]
    fn ready_port_from_stdout_ignores_other_output() {
        assert_eq!(ready_port_from_stdout("POTATO_BACKEND_READY nope"), None);
        assert_eq!(ready_port_from_stdout("ordinary stdout"), None);
    }
}
