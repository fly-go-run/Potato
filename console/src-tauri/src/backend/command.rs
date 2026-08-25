//! Backend command construction for development and packaged builds.

use std::path::{Path, PathBuf};
#[cfg(debug_assertions)]
use std::process::{Command as StdCommand, Stdio};

#[cfg(not(debug_assertions))]
use tauri::Manager;
use tauri_plugin_shell::{process::Command, ShellExt};

/// Builds the command used to start the Python backend sidecar.
#[cfg(debug_assertions)]
pub(super) fn create(app: &tauri::AppHandle) -> Result<Command, String> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_path = repo_root.join("src");
    let command = if command_exists("uv") {
        log::info!(
            "[backend] dev command: uv run python -m potato.tauri.entry cwd={}",
            repo_root.display(),
        );
        app.shell()
            .command("uv")
            .args(["run", "python", "-m", "potato.tauri.entry"])
            .current_dir(repo_root)
            .env("PYTHONPATH", source_path.display().to_string())
    } else {
        let (python, prefix_args) = python_command(&repo_root);
        let mut args = prefix_args;
        args.extend(["-m", "potato.tauri.entry"]);
        log::info!(
            "[backend] dev command: {} {} cwd={}",
            python,
            args.join(" "),
            repo_root.display(),
        );
        app.shell()
            .command(python)
            .args(args)
            .current_dir(repo_root)
            .env("PYTHONPATH", source_path.display().to_string())
    };
    Ok(command)
}

/// Builds the command used to start the packaged Python backend sidecar.
#[cfg(not(debug_assertions))]
pub(super) fn create(app: &tauri::AppHandle) -> Result<Command, String> {
    if let Some(command) = packaged_cpython_command(app) {
        return Ok(command);
    }

    let backend = packaged_backend_executable(app)?;
    let backend_dir = backend
        .parent()
        .ok_or_else(|| format!("backend executable has no parent: {}", backend.display()))?
        .to_path_buf();
    log::info!(
        "[backend] packaged command: {} cwd={}",
        backend.display(),
        backend_dir.display(),
    );
    let mut command = app
        .shell()
        .command(backend)
        .current_dir(&backend_dir)
        .env(path_env_key(), path_with_backend_dir(&backend_dir)?);
    // Bundled standalone Python used by the backend to install third-party
    // plugin dependencies (sys.executable is the frozen backend, not Python).
    if let Some(python) = packaged_python_runtime(app) {
        log::info!("[backend] bundled python runtime: {}", python.display());
        command = command.env(
            "POTATO_DESKTOP_PY_RUNTIME",
            python.to_string_lossy().to_string(),
        );
    } else {
        log::warn!(
            "[backend] bundled python runtime not found; plugin dependency \
             installation will be unavailable"
        );
    }
    if let Some(node_runtime) = packaged_node_runtime(app) {
        log::info!("[backend] bundled node runtime: {}", node_runtime.display());
        command = command.env(
            "POTATO_DESKTOP_NODE_RUNTIME",
            node_runtime.to_string_lossy().to_string(),
        );
    } else {
        log::warn!("[backend] bundled node runtime not found");
    }
    Ok(command)
}

/// Prefer the bundled CPython interpreter when Potato is installed into it.
/// The frozen PyInstaller binary is an 18s-class cold start; CPython is ~1–4s.
#[cfg(not(debug_assertions))]
fn packaged_cpython_command(app: &tauri::AppHandle) -> Option<Command> {
    let python = packaged_python_runtime(app)?;
    if !packaged_potato_importable(&python) {
        log::info!(
            "[backend] bundled CPython has no Potato install; using frozen sidecar"
        );
        return None;
    }
    let cwd = python
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| python.parent().unwrap_or(Path::new(".")).to_path_buf());
    log::info!(
        "[backend] packaged CPython command: {} -m potato.tauri.entry cwd={}",
        python.display(),
        cwd.display(),
    );
    let mut command = app
        .shell()
        .command(&python)
        .args(["-m", "potato.tauri.entry"])
        .current_dir(&cwd);
    command = command.env(
        "POTATO_DESKTOP_PY_RUNTIME",
        python.to_string_lossy().to_string(),
    );
    if let Some(node_runtime) = packaged_node_runtime(app) {
        command = command.env(
            "POTATO_DESKTOP_NODE_RUNTIME",
            node_runtime.to_string_lossy().to_string(),
        );
    }
    Some(command)
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn packaged_potato_importable(python: &Path) -> bool {
    python_site_packages(python)
        .join("potato")
        .join("__init__.py")
        .is_file()
}

fn python_site_packages(python: &Path) -> PathBuf {
    // .../python/bin/python3 → .../python/lib/python3.11/site-packages
    // .../python/python.exe  → .../python/Lib/site-packages
    if cfg!(windows) {
        python
            .parent()
            .map(|dir| dir.join("Lib").join("site-packages"))
            .unwrap_or_default()
    } else {
        python
            .parent()
            .and_then(|bin| bin.parent())
            .map(|root| root.join("lib").join("python3.11").join("site-packages"))
            .unwrap_or_default()
    }
}

#[cfg(not(debug_assertions))]
fn packaged_python_runtime(app: &tauri::AppHandle) -> Option<PathBuf> {
    let base = app
        .path()
        .resource_dir()
        .ok()?
        .join("binaries")
        .join("python-runtime")
        .join("python");
    let candidates = if cfg!(windows) {
        vec![base.join("python.exe")]
    } else {
        vec![base.join("bin").join("python3"), base.join("bin").join("python")]
    };
    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(not(debug_assertions))]
fn packaged_node_runtime(app: &tauri::AppHandle) -> Option<PathBuf> {
    let root = app
        .path()
        .resource_dir()
        .ok()?
        .join("binaries")
        .join("node-runtime");
    let node = if cfg!(windows) {
        root.join("node.exe")
    } else {
        root.join("bin").join("node")
    };
    node.is_file().then_some(root)
}

#[cfg(not(debug_assertions))]
fn packaged_backend_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let executable_name = if cfg!(windows) {
        "potato-backend.exe"
    } else {
        "potato-backend"
    };
    let path = app
        .path()
        .resource_dir()
        .map_err(|err| format!("failed to resolve resource directory: {err}"))?
        .join("binaries")
        .join("potato-backend")
        .join(executable_name);

    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "backend executable not found at {}",
            path.display()
        ))
    }
}

#[cfg(not(debug_assertions))]
fn path_with_backend_dir(backend_dir: &Path) -> Result<String, String> {
    let mut paths = vec![backend_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os(path_env_key()) {
        paths.extend(std::env::split_paths(&existing));
    }

    std::env::join_paths(paths)
        .map_err(|err| format!("failed to join backend PATH entries: {err}"))?
        .into_string()
        .map_err(|_| "backend PATH contains non-Unicode data".to_string())
}

#[cfg(all(not(debug_assertions), windows))]
fn path_env_key() -> &'static str {
    "Path"
}

#[cfg(all(not(debug_assertions), not(windows)))]
fn path_env_key() -> &'static str {
    "PATH"
}

pub(super) fn cua_driver_binary(app: &tauri::AppHandle) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "cua-driver.exe"
    } else {
        "cua-driver"
    };
    #[cfg(debug_assertions)]
    {
        let _ = app;
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("cua-driver")
            .join(name);
        return path.is_file().then_some(path);
    }
    #[cfg(not(debug_assertions))]
    {
        let path = app
            .path()
            .resource_dir()
            .ok()?
            .join("binaries")
            .join("cua-driver")
            .join(name);
        path.is_file().then_some(path)
    }
}

#[cfg(debug_assertions)]
fn command_exists(command: &str) -> bool {
    StdCommand::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(debug_assertions)]
fn local_python(repo_root: &Path) -> Option<String> {
    let candidates = if cfg!(windows) {
        vec![
            repo_root.join(".venv/Scripts/python.exe"),
            repo_root.join("venv/Scripts/python.exe"),
        ]
    } else {
        vec![
            repo_root.join(".venv/bin/python"),
            repo_root.join("venv/bin/python"),
        ]
    };

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

#[cfg(debug_assertions)]
fn python_command(repo_root: &Path) -> (String, Vec<&'static str>) {
    if let Some(local) = local_python(repo_root) {
        return (local, vec![]);
    }
    #[cfg(windows)]
    {
        if command_exists("py") {
            return ("py".to_string(), vec!["-3"]);
        }
    }
    if command_exists("python3") {
        ("python3".to_string(), vec![])
    } else {
        ("python".to_string(), vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_site_packages_live_under_lib_python311() {
        if cfg!(windows) {
            let site = python_site_packages(Path::new(r"C:\app\python\python.exe"));
            assert!(site.ends_with(Path::new(r"Lib\site-packages")));
        } else {
            let site = python_site_packages(Path::new("/app/python/bin/python3"));
            assert_eq!(
                site,
                PathBuf::from("/app/python/lib/python3.11/site-packages")
            );
        }
    }
}
