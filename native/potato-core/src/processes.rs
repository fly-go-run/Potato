use crate::{Error, Result};
use process_wrap::tokio::*;
use serde_json::{json, Value};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::sync::CancellationToken;

struct OwnedProcess(Option<Box<dyn ChildWrapper>>);
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.start_kill();
        }
    }
}

async fn drain(mut input: impl AsyncRead + Unpin) -> std::io::Result<(String, bool)> {
    let mut kept = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let n = input.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        let take = n.min(1_000_000usize.saturating_sub(kept.len()));
        kept.extend_from_slice(&buffer[..take]);
        truncated |= take < n;
    }
    Ok((String::from_utf8_lossy(&kept).into_owned(), truncated))
}

pub(crate) async fn execute(
    command: &str,
    cwd: &Path,
    timeout: u64,
    cancel: &CancellationToken,
) -> Result<Value> {
    if command.is_empty() || command.len() > 64_000 || !(1..=3600).contains(&timeout) {
        return Err(Error::new(400, "Command or timeout is invalid"));
    }
    #[cfg(unix)]
    let mut cmd = CommandWrap::with_new("/bin/sh", |cmd| {
        cmd.args(["-c", command]);
    });
    #[cfg(windows)]
    let mut cmd = CommandWrap::with_new("powershell.exe", |cmd| {
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            command,
        ]);
    });
    cmd.command_mut()
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.wrap(KillOnDrop);
    #[cfg(unix)]
    cmd.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    cmd.wrap(JobObject);
    let mut process = OwnedProcess(Some(cmd.spawn()?));
    let child = process.0.as_mut().unwrap();
    let stdout = child.stdout().take().unwrap();
    let stderr = child.stderr().take().unwrap();
    let output = async {
        let (status, out, err) = tokio::try_join!(child.wait(), drain(stdout), drain(stderr))?;
        Ok::<_, std::io::Error>((status, out, err))
    };
    let result = tokio::select! {
        _ = cancel.cancelled() => Err(Error::new(499, "Command cancelled")),
        result = tokio::time::timeout(Duration::from_secs(timeout), output) => match result {
            Ok(Ok((status, out, err))) => Ok(json!({"stdout":out.0,"stderr":err.0,"return_code":status.code(),"truncated":out.1 || err.1})),
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
