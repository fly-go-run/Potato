//! Working-directory resolution shared by the desktop sidecar and file APIs.

use std::path::PathBuf;

/// Resolve the Potato working directory the same way the Python sidecar does.
pub(crate) fn working_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("POTATO_WORKING_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("QWENPAW_WORKING_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("COPAW_WORKING_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = dirs::home_dir()?;
    let potato = home.join(".potato");
    let qwenpaw = home.join(".qwenpaw");
    let copaw = home.join(".copaw");
    if potato.exists() {
        Some(potato)
    } else if qwenpaw.exists() {
        Some(qwenpaw)
    } else if copaw.exists() {
        Some(copaw)
    } else {
        Some(potato)
    }
}

pub(crate) fn desktop_port_file() -> Option<PathBuf> {
    Some(working_dir()?.join("desktop_port"))
}

pub(crate) fn desktop_pid_file() -> Option<PathBuf> {
    Some(working_dir()?.join("desktop_backend.pid"))
}

pub(crate) fn read_desktop_port() -> Option<u16> {
    let text = std::fs::read_to_string(desktop_port_file()?).ok()?;
    let port: u16 = text.trim().parse().ok()?;
    (1024..=65535).contains(&port).then_some(port)
}

pub(crate) fn read_desktop_pid() -> Option<u32> {
    let text = std::fs::read_to_string(desktop_pid_file()?).ok()?;
    let pid: u32 = text.trim().parse().ok()?;
    (pid > 0).then_some(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn prefers_potato_working_dir_env() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var("POTATO_WORKING_DIR").ok();
        std::env::set_var("POTATO_WORKING_DIR", "/tmp/potato-paths-test");
        let dir = working_dir();
        match previous {
            Some(value) => std::env::set_var("POTATO_WORKING_DIR", value),
            None => std::env::remove_var("POTATO_WORKING_DIR"),
        }
        assert_eq!(dir, Some(PathBuf::from("/tmp/potato-paths-test")));
    }
}
