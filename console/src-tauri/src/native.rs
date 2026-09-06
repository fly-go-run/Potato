//! Thin IPC adapter. Business logic is independently testable in potato-core.
use std::sync::Arc;
use potato_core::{Error, Runtime};
use serde_json::Value;
use tauri::{Emitter, Manager, State, WebviewWindow};

pub(crate) struct NativeState(pub Arc<Runtime>);

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Deliberately separate from the Python data home until migration is
    // verified. Switching builds never rewrites legacy data.
    let root = std::env::var_os("POTATO_NATIVE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or(app.path().app_data_dir()?.join("native-v1"));
    let runtime = Runtime::open(&root)?;
    let driver_name=if cfg!(windows){"cua-driver.exe"}else{"cua-driver"};
    let driver=app.path().resource_dir()?.join("binaries/native-cua-driver").join(driver_name);
    let driver=if driver.is_file(){driver}else{std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries/native-cua-driver").join(driver_name)};
    runtime.configure_computer_driver(driver,app.config().identifier.clone())?;
    let handle=app.handle().clone();
    runtime.set_background_listener(Arc::new(move |event| {
        handle.emit("native-background-updated",event).map_err(|_|Error::new(499,"Desktop listener disconnected"))
    }))?;
    tauri::async_runtime::spawn(runtime.clone().serve_scheduler());
    app.manage(NativeState(runtime));
    Ok(())
}

#[tauri::command]
pub(crate) async fn native_request(state: State<'_, NativeState>, method: String, path: String, body: Option<Value>) -> Result<Value, Error> {
    state.0.request(&method, &path, body.unwrap_or(Value::Null)).await
}

#[tauri::command]
pub(crate) async fn native_chat_start(window: WebviewWindow, state: State<'_, NativeState>, request_id: String, body: Value) -> Result<(), Error> {
    uuid::Uuid::parse_str(&request_id).map_err(|_| Error::new(400, "Invalid request id"))?;
    let event = format!("native-chat-{request_id}");
    state.0.start(request_id, body, Arc::new(move |frame| {
        window.emit(&event, frame).map_err(|_| Error::new(499, "Desktop stream disconnected"))
    }))
}

#[tauri::command]
pub(crate) fn native_chat_cancel(state: State<'_, NativeState>, request_id: String) -> Result<bool, Error> {
    state.0.cancel(&request_id)
}

#[tauri::command]
pub(crate) async fn native_transcribe(state: State<'_, NativeState>, filename: String, mime: String, bytes: Vec<u8>) -> Result<Value, Error> {
    state.0.transcribe(filename, mime, bytes).await
}

#[tauri::command]
pub(crate) async fn native_voice_start(window: WebviewWindow, state: State<'_, NativeState>, request_id: String) -> Result<(), Error> {
    uuid::Uuid::parse_str(&request_id).map_err(|_| Error::new(400, "Invalid voice request id"))?;
    let event = format!("native-voice-{request_id}");
    state.0.voice_start(request_id, Arc::new(move |frame| {
        window.emit(&event, frame).map_err(|_| Error::new(499, "Voice window disconnected"))
    })).await
}

#[tauri::command]
pub(crate) fn native_voice_audio(state: State<'_, NativeState>, request_id: String, bytes: Vec<u8>) -> Result<(), Error> {
    state.0.voice_audio(&request_id, bytes)
}

#[tauri::command]
pub(crate) async fn native_voice_end(state: State<'_, NativeState>, request_id: String, cancel: bool) -> Result<(), Error> {
    state.0.voice_end(&request_id, cancel).await
}
