use crate::{Error, Result};
use process_wrap::tokio::*;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

struct OwnedProcess(Option<Box<dyn ChildWrapper>>);
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.start_kill();
        }
    }
}

async fn drain(
    mut input: impl AsyncRead + Unpin,
    destination: Option<PathBuf>,
) -> std::io::Result<(String, bool)> {
    let mut archive = match destination {
        Some(path) => Some(
            tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .await?,
        ),
        None => None,
    };
    let mut total = 0usize;
    let mut tail = Vec::new();
    let limit: usize = if archive.is_some() { 12_000 } else { 1_000_000 };
    let mut kept = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let n = input.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n);
        if let Some(file) = &mut archive {
            if total > 64_000_000 {
                return Err(std::io::Error::other(
                    "Command output exceeded the 64 MB per-stream archive limit; partial output is retained",
                ));
            }
            file.write_all(&buffer[..n]).await?;
            tail.extend_from_slice(&buffer[..n]);
            if tail.len() > 4000 {
                tail.drain(..tail.len() - 4000);
            }
        }
        let take = n.min(limit.saturating_sub(kept.len()));
        kept.extend_from_slice(&buffer[..take]);
        truncated |= take < n;
    }
    if let Some(file) = &mut archive {
        file.sync_all().await?;
    }
    let mut text = String::from_utf8_lossy(&kept).into_owned();
    if truncated && archive.is_some() {
        text.push_str("\n[… full stream available through job_output …]\n");
        text.push_str(&String::from_utf8_lossy(&tail));
    }
    Ok((text, truncated))
}

#[cfg(all(test, unix))]
pub(crate) async fn execute(
    command: &str,
    cwd: &Path,
    timeout: u64,
    cancel: &CancellationToken,
) -> Result<Value> {
    let plan = crate::sandbox::Plan::new(
        command.into(),
        cwd.into(),
        cwd.into(),
        cwd.join("private"),
        "danger-full-access".into(),
        cwd.into(),
    );
    execute_spooled(&plan, timeout, cancel, None).await
}

pub(crate) async fn execute_spooled(
    plan: &crate::sandbox::Plan,
    timeout: u64,
    cancel: &CancellationToken,
    archive: Option<&Path>,
) -> Result<Value> {
    let command = &plan.command;
    if cancel.is_cancelled() {
        return Err(Error::new(499, "Command cancelled before spawn"));
    }
    if command.is_empty() || command.len() > 64_000 || !(1..=3600).contains(&timeout) {
        return Err(Error::new(400, "Command or timeout is invalid"));
    }
    #[cfg(windows)]
    if !plan.unsandboxed {
        return execute_windows(plan, timeout, cancel, archive).await;
    }
    let launch_plan = plan.clone();
    let (program, args) = tokio::task::spawn_blocking(move || launch_plan.launch())
        .await
        .map_err(|e| Error::new(500, e.to_string()))??;
    if cancel.is_cancelled() {
        return Err(Error::new(499, "Command cancelled before spawn"));
    }
    let mut cmd = CommandWrap::with_new(program, |cmd| {
        cmd.args(args);
    });
    cmd.command_mut()
        .current_dir(&plan.cwd)
        .env_clear()
        .envs(&plan.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // Do not inherit already-open credentials or sockets into the child.
        // CLOEXEC preserves Rust's spawn error pipe until exec succeeds/fails.
        let max_fd = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
        if !(3..=1_048_576).contains(&max_fd) {
            return Err(Error::new(503, "Cannot bound inherited descriptor cleanup"));
        }
        unsafe {
            cmd.command_mut().pre_exec(move || {
                for fd in 3..max_fd as i32 {
                    libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
                Ok(())
            });
        }
    }
    cmd.wrap(KillOnDrop);
    #[cfg(windows)]
    cmd.command_mut().creation_flags(0x08000000); // CREATE_NO_WINDOW
    #[cfg(unix)]
    cmd.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    cmd.wrap(JobObject);
    let mut process = OwnedProcess(Some(cmd.spawn()?));
    let child = process.0.as_mut().unwrap();
    let stdout = child.stdout().take().unwrap();
    let stderr = child.stderr().take().unwrap();
    let output = async {
        let (status, out, err) = tokio::try_join!(
            child.wait(),
            drain(stdout, archive.map(|p| p.join("stdout"))),
            drain(stderr, archive.map(|p| p.join("stderr")))
        )?;
        Ok::<_, std::io::Error>((status, out, err))
    };
    let result = tokio::select! {
        _ = cancel.cancelled() => Err(Error::new(499, "Command cancelled")),
        result = tokio::time::timeout(Duration::from_secs(timeout), output) => match result {
            Ok(Ok((status, out, err))) => {
                #[cfg(unix)]
                let signal = { use std::os::unix::process::ExitStatusExt; status.signal() };
                #[cfg(not(unix))]
                let signal: Option<i32> = None;
                Ok(json!({"stdout":out.0,"stderr":err.0,"return_code":status.code(),"exit_code":status.code(),"signal":signal,"truncated":out.1 || err.1}))
            },
            Ok(Err(error)) => Err(error.into()),
            Err(_) => Err(Error::new(408, "Command timed out")),
        }
    };
    if result.is_err() {
        let child = process.0.as_mut().unwrap();
        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    }
    process.0.take();
    result
}

#[cfg(windows)]
async fn execute_windows(
    plan: &crate::sandbox::Plan,
    timeout: u64,
    cancel: &CancellationToken,
    archive: Option<&Path>,
) -> Result<Value> {
    use potato_windows_sandbox::{powershell_args, system_directory, Options, Process};
    let options = Options {
        program: system_directory()
            .map_err(|e| Error::new(503, e.to_string()))?
            .join("WindowsPowerShell\\v1.0\\powershell.exe"),
        args: powershell_args(&plan.command),
        cwd: plan.cwd.clone(),
        project: plan.project.clone(),
        private: plan.private.clone(),
        scratch: plan.scratch.clone(),
        denied: plan.denied.clone(),
        env: plan.env.clone(),
        network: plan.network,
    };
    // A detached spawn_blocking task must not launch a command after cancellation.
    // The blocking launcher checks this flag immediately before resuming the
    // suspended child; dropping its returned Process kills the whole job.
    let spawn_cancel = cancel.child_token();
    let _cancel_on_drop = spawn_cancel.clone().drop_guard();
    let mut process = tokio::task::spawn_blocking(move || {
        Process::spawn_cancellable(&options, || spawn_cancel.is_cancelled())
    })
    .await
    .map_err(|e| Error::new(500, e.to_string()))?
    .map_err(|e| Error::new(if cancel.is_cancelled() { 499 } else { 503 }, e.to_string()))?;
    let stdout = tokio::fs::File::from_std(process.stdout.take().unwrap());
    let stderr = tokio::fs::File::from_std(process.stderr.take().unwrap());
    let waiting = &mut process;
    let wait = async move {
        loop {
            if let Some(code) = waiting.try_wait()? {
                // Retire detached children too; otherwise they can keep the
                // output pipes and sandbox permissions alive indefinitely.
                waiting.terminate_tree()?;
                return Ok::<_, std::io::Error>(code);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let output = async {
        tokio::try_join!(
            wait,
            drain(stdout, archive.map(|p| p.join("stdout"))),
            drain(stderr, archive.map(|p| p.join("stderr")))
        )
    };
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::new(499, "Command cancelled")),
        result = tokio::time::timeout(Duration::from_secs(timeout), output) => match result {
            Ok(Ok((code,out,err))) => Ok(json!({"stdout":out.0,"stderr":err.0,"return_code":code,"exit_code":code,"signal":null,"truncated":out.1 || err.1})),
            Ok(Err(error)) => Err(error.into()),
            Err(_) => Err(Error::new(408, "Command timed out")),
        }
    };
    // Cleanup failures must not be classified as pre-execution sandbox failures:
    // user code already ran, so a blind automatic replay would be incorrect.
    let cleanup = tokio::task::spawn_blocking(move || process.finish())
        .await
        .map_err(|e| Error::new(500, e.to_string()))?;
    match (result, cleanup) {
        (Ok(mut value), Err(error)) => {
            value["cleanup_error"] = json!(error.to_string());
            Ok(value)
        }
        (result, _) => result,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn captures_both_streams_and_cancels_child_group() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let out = execute(
            "printf hello; printf error >&2; exit 7",
            dir.path(),
            10,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(out["stdout"], "hello");
        assert_eq!(out["stderr"], "error");
        assert_eq!(out["return_code"], 7);
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            trigger.cancel();
        });
        assert!(
            execute("(sleep 1; touch survived) & wait", dir.path(), 10, &cancel)
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert!(!dir.path().join("survived").exists());
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    fn fixture(command: &str) -> (tempfile::TempDir, crate::sandbox::Plan) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for name in ["project", "scratch", "archive"] {
            std::fs::create_dir(root.join(name)).unwrap();
        }
        let plan = crate::sandbox::Plan::new(
            command.into(),
            root.join("project"),
            root.join("project"),
            root.join("private"),
            "workspace-write".into(),
            root.join("scratch"),
        );
        (dir, plan)
    }

    #[tokio::test]
    async fn windows_spools_both_streams_past_pipe_capacity() {
        let (dir, plan) = fixture(
            "[Console]::Out.Write(('中文' + ('o' * 100000))); [Console]::Error.Write(('e' * 100000)); exit 7",
        );
        let out = execute_spooled(
            &plan,
            20,
            &CancellationToken::new(),
            Some(&dir.path().join("archive")),
        )
        .await
        .unwrap();
        assert_eq!(out["exit_code"], 7);
        assert_eq!(out["truncated"], true);
        assert!(out["stdout"].as_str().unwrap().starts_with("中文"));
        assert_eq!(
            std::fs::metadata(dir.path().join("archive/stdout"))
                .unwrap()
                .len(),
            100006
        );
        assert_eq!(
            std::fs::metadata(dir.path().join("archive/stderr"))
                .unwrap()
                .len(),
            100000
        );
    }

    #[tokio::test]
    async fn windows_shell_timeout_and_cancellation_release_job() {
        let (dir, plan) = fixture("Start-Sleep -Seconds 30");
        assert_eq!(
            execute_spooled(&plan, 1, &CancellationToken::new(), None)
                .await
                .unwrap_err()
                .status,
            408
        );
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            trigger.cancel();
        });
        assert_eq!(
            execute_spooled(&plan, 20, &cancel, None)
                .await
                .unwrap_err()
                .status,
            499
        );
        std::fs::rename(&plan.project, dir.path().join("released-project")).unwrap();
    }
}
