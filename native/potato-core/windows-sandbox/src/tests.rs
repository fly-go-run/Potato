//! Real Windows tests: no mocked security APIs and no silent skips.
use super::*;
use std::{
    io::Read,
    time::{Duration, Instant},
};

fn fixture(command: &str) -> (tempfile::TempDir, Options) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for name in ["project", "scratch", "outside", "private"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let system = system_directory().unwrap();
    let scratch = root.join("scratch");
    let options = Options {
        program: system.join("WindowsPowerShell\\v1.0\\powershell.exe"),
        args: powershell_args(command),
        cwd: root.join("project"),
        project: root.join("project"),
        private: root.join("private"),
        scratch: scratch.clone(),
        denied: vec![],
        network: false,
        env: [
            (
                "SystemRoot".into(),
                system.parent().unwrap().display().to_string(),
            ),
            ("TEMP".into(), scratch.display().to_string()),
            ("TMP".into(), scratch.display().to_string()),
            ("USERPROFILE".into(), scratch.display().to_string()),
        ]
        .into(),
    };
    (dir, options)
}

fn collect(mut process: Process) -> (i32, String, String) {
    // Drain while running: startup diagnostics can exceed anonymous-pipe
    // capacity, and reading only after wait would deadlock the test itself.
    let read = |mut pipe: std::fs::File| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            String::from_utf8_lossy(&bytes).replace('\0', "")
        })
    };
    let out = read(process.stdout.take().unwrap());
    let err = read(process.stderr.take().unwrap());
    let deadline = Instant::now() + Duration::from_secs(20);
    let code = loop {
        if let Some(code) = process.try_wait().unwrap() {
            break Some(code);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    process.finish().unwrap();
    let (out, err) = (out.join().unwrap(), err.join().unwrap());
    assert!(
        code.is_some(),
        "sandbox test timed out; stdout={out:?}; stderr={err:?}"
    );
    (code.unwrap(), out, err)
}

#[test]
fn windows_enforcement_probe() {
    probe().unwrap();
}

#[test]
fn windows_reads_project_but_denies_secrets_writes_and_parent_environment() {
    let (_dir, options) = fixture("$ErrorActionPreference='Stop'; if ([IO.File]::ReadAllText('normal.txt') -ne 'readable') { exit 10 }; foreach ($p in @('.ENV', 'blocked.txt')) { try { [IO.File]::ReadAllText($p); exit 11 } catch [UnauthorizedAccessException] {} }; try { [IO.File]::WriteAllText('normal.txt','bad'); exit 12 } catch [UnauthorizedAccessException] {}; if ($env:OPENAI_API_KEY) { exit 13 }; [IO.File]::WriteAllText(($env:TEMP+'\\result.txt'),'ok'); Write-Output 'success'; exit 0");
    std::fs::write(options.project.join("normal.txt"), "readable").unwrap();
    std::fs::write(options.project.join(".ENV"), "secret").unwrap();
    std::fs::write(options.project.join("blocked.txt"), "denied").unwrap();
    let mut options = options;
    options.denied.push(options.project.join("blocked.txt"));
    let result = collect(Process::spawn(&options).unwrap());
    assert_eq!(result.0, 0, "{result:?}");
    assert!(result.1.contains("success"));
    assert_eq!(
        std::fs::read_to_string(options.project.join("normal.txt")).unwrap(),
        "readable"
    );
    assert_eq!(
        std::fs::read_to_string(options.scratch.join("result.txt")).unwrap(),
        "ok"
    );
}

#[test]
fn windows_refuses_existing_hardlinks_before_launch_and_releases_pins() {
    let (dir, options) = fixture("exit 99");
    let external = dir.path().join("outside/secret.txt");
    std::fs::write(&external, "secret").unwrap();
    std::fs::hard_link(external, options.project.join("alias")).unwrap();
    assert!(Process::spawn(&options)
        .err()
        .unwrap()
        .to_string()
        .contains("hardlink"));
    std::fs::rename(&options.project, dir.path().join("moved")).unwrap();
}

#[test]
fn windows_refuses_junctions_before_launch() {
    let (dir, options) = fixture("exit 99");
    let junction = options.project.join("junction");
    let status = std::process::Command::new(system_directory().unwrap().join("cmd.exe"))
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(dir.path().join("outside"))
        .status()
        .unwrap();
    assert!(status.success());
    assert!(Process::spawn(&options)
        .err()
        .unwrap()
        .to_string()
        .contains("reparse"));
    std::fs::remove_dir(junction).unwrap();
}

#[test]
fn windows_cancellation_before_resume_does_not_execute_user_code() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (dir, options) = fixture("[IO.File]::WriteAllText(($env:TEMP+'\\executed'),'bad')");
    let calls = AtomicUsize::new(0);
    let result = Process::spawn_cancellable(&options, || calls.fetch_add(1, Ordering::SeqCst) > 0);
    assert_eq!(
        result.err().unwrap().kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(!options.scratch.join("executed").exists());
    std::fs::rename(&options.project, dir.path().join("moved")).unwrap();
}

#[test]
fn windows_drop_kills_descendants_and_releases_project() {
    let (dir, options) = fixture("$p=Start-Process -FilePath ($env:SystemRoot+'\\System32\\WindowsPowerShell\\v1.0\\powershell.exe') -ArgumentList '-NoLogo -NoProfile -NonInteractive -Command Start-Sleep -Seconds 30' -PassThru; [IO.File]::WriteAllText(($env:TEMP+'\\pid'),[string]$p.Id); Start-Sleep -Seconds 30");
    let process = Process::spawn(&options).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !options.scratch.join("pid").exists() {
        assert!(
            process.try_wait().unwrap().is_none(),
            "shell exited before creating a descendant"
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid: u32 = std::fs::read_to_string(options.scratch.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    drop(process);
    unsafe {
        use windows_sys::Win32::{Foundation::*, System::Threading::*};
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !handle.is_null() {
            let result = WaitForSingleObject(handle, 1000);
            CloseHandle(handle);
            assert_eq!(result, WAIT_OBJECT_0, "descendant survived job close");
        }
    }
    std::fs::rename(&options.project, dir.path().join("moved")).unwrap();
}

#[test]
fn windows_network_is_blocked_and_capability_retry_retains_scratch() {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    let (_dir, mut options) = fixture(&format!("$ErrorActionPreference='Stop'; $c=New-Object Net.Sockets.TcpClient; try {{ $c.Connect('127.0.0.1',{port}); exit 19 }} catch {{ [IO.File]::WriteAllText(($env:TEMP+'\\network-denied'),'yes'); exit 0 }}"));
    let result = collect(Process::spawn(&options).unwrap());
    assert_eq!(result.0, 0, "{result:?}");
    assert!(options.scratch.join("network-denied").exists());
    options.network = true;
    options.args = powershell_args("$ErrorActionPreference='Stop'; [IO.File]::AppendAllText(($env:TEMP+'\\network-denied'),' retained'); exit 0");
    let result = collect(Process::spawn(&options).unwrap());
    assert_eq!(result.0, 0, "{result:?}");
    assert_eq!(
        std::fs::read_to_string(options.scratch.join("network-denied")).unwrap(),
        "yes retained"
    );
}

#[test]
fn windows_acl_cleanup_preserves_another_live_jobs_grants() {
    let (dir, options) = fixture("Start-Sleep -Seconds 30");
    std::fs::write(options.project.join("input"), "expected").unwrap();
    let before = acl::dacl_for_test(&options.project);
    let before_file = acl::dacl_for_test(&options.project.join("input"));
    let mut first = Process::spawn(&options).unwrap();
    let mut other = options.clone();
    other.scratch = dir.path().canonicalize().unwrap().join("second-scratch");
    std::fs::create_dir(&other.scratch).unwrap();
    other
        .env
        .insert("TEMP".into(), other.scratch.display().to_string());
    other.args = powershell_args("$ErrorActionPreference='Stop'; Start-Sleep -Seconds 2; if ([IO.File]::ReadAllText('input') -ne 'expected') { exit 8 }; exit 0");
    let second = Process::spawn(&other).unwrap();
    first.finish().unwrap();
    assert_ne!(acl::dacl_for_test(&options.project), before);
    let result = collect(second);
    assert_eq!(result.0, 0, "{result:?}");
    assert_eq!(acl::dacl_for_test(&options.project), before);
    assert_eq!(
        acl::dacl_for_test(&options.project.join("input")),
        before_file
    );
}

#[test]
fn windows_default_private_and_external_files_are_not_readable() {
    let (dir, mut options) = fixture("");
    let outside = dir.path().join("outside/private.txt");
    let private = options.private.join("credentials.txt");
    std::fs::write(&outside, "external").unwrap();
    std::fs::write(&private, "private").unwrap();
    options
        .env
        .insert("TEST_OUTSIDE".into(), outside.display().to_string());
    options
        .env
        .insert("TEST_PRIVATE".into(), private.display().to_string());
    options.args = powershell_args("$ErrorActionPreference='Stop'; foreach ($p in @($env:TEST_OUTSIDE,$env:TEST_PRIVATE)) { try { [IO.File]::ReadAllText($p); exit 9 } catch [UnauthorizedAccessException] {} }; exit 0");
    let result = collect(Process::spawn(&options).unwrap());
    assert_eq!(result.0, 0, "{result:?}");
}
