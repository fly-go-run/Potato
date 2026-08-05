//! Tauri commands for opening conversation-produced local files with the OS:
//! system default application open, and reveal in the file manager.

use std::path::PathBuf;
use std::process::Command;

/// Open a local file (or directory) with the system default application.
#[tauri::command]
pub(crate) fn open_local_path(path: String) -> Result<(), String> {
    let path = validated_local_path(&path).inspect_err(|err| {
        log::warn!("[local-file] open rejected: {err}");
    })?;
    spawn_detached(open_command(&path))
}

/// Reveal a local file in the system file manager (Finder / Explorer).
#[tauri::command]
pub(crate) fn reveal_local_path(path: String) -> Result<(), String> {
    let path = validated_local_path(&path).inspect_err(|err| {
        log::warn!("[local-file] reveal rejected: {err}");
    })?;
    spawn_detached(reveal_command(&path))
}

/// The frontend normalizes paths to forward slashes; Windows tooling
/// (`explorer /select,`) only accepts backslashes, so convert back here.
fn validated_local_path(raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("local path is empty".into());
    }
    if raw.chars().any(char::is_control) {
        return Err("local path contains control characters".into());
    }
    let normalized = if cfg!(windows) {
        raw.replace('/', "\\")
    } else {
        raw.to_owned()
    };
    let path = PathBuf::from(normalized);
    if !path.is_absolute() {
        return Err("local path must be absolute".into());
    }
    if !path.exists() {
        return Err("local path does not exist".into());
    }
    Ok(path)
}

fn spawn_detached(mut command: Command) -> Result<(), String> {
    match command.spawn() {
        Ok(_) => Ok(()),
        Err(err) => {
            log::warn!("[local-file] launcher failed to start: {err}");
            Err(err.to_string())
        }
    }
}

#[cfg(target_os = "macos")]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("open");
    command.arg("-R").arg(path);
    command
}

#[cfg(target_os = "windows")]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("explorer.exe");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn reveal_command(path: &std::path::Path) -> Command {
    // `/select,<path>` must be a single argument.
    let mut argument = std::ffi::OsString::from("/select,");
    argument.push(path.as_os_str());
    let mut command = Command::new("explorer.exe");
    command.arg(argument);
    command
}

#[cfg(target_os = "linux")]
fn open_command(path: &std::path::Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

#[cfg(target_os = "linux")]
fn reveal_command(path: &std::path::Path) -> Command {
    // No portable reveal on Linux; opening the containing directory is the
    // closest widely-supported behavior.
    let target = path.parent().filter(|p| !p.as_os_str().is_empty());
    let mut command = Command::new("xdg-open");
    command.arg(target.unwrap_or(path));
    command
}
