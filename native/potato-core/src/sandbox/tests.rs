use super::*;
#[cfg(target_os = "macos")]
use tokio_util::sync::CancellationToken;

#[test]
fn execution_scope_changes_with_permissions_but_not_private_scratch_identity() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut p = Plan::new(
        "printf hello".into(),
        root.clone(),
        root.clone(),
        root.join("private"),
        "workspace-write".into(),
        root.join("scratch"),
    );
    let first = p.context();
    let other = Plan::new(
        p.command.clone(),
        root.clone(),
        root.clone(),
        root.join("private"),
        p.mode.clone(),
        root.join("other"),
    );
    assert_eq!(first["scope_digest"], other.context()["scope_digest"]);
    assert_ne!(first["plan_digest"], other.context()["plan_digest"]);
    p.network = true;
    assert_ne!(first["scope_digest"], p.context()["scope_digest"]);
    p.unsandboxed = true;
    assert_eq!(p.context()["backend"], "none");
    assert!(!p.env.contains_key("SSH_AUTH_SOCK"));
    assert!(!p.env.contains_key("OPENAI_API_KEY"));
}

#[test]
fn failures_trigger_diagnosis_not_authorization() {
    assert!(!likely_denied(
        &json!({"exit_code":0,"stderr":"permission denied"})
    ));
    assert!(!likely_denied(
        &json!({"exit_code":1,"stderr":"assertion failed"})
    ));
    assert!(likely_denied(
        &json!({"exit_code":1,"stderr":"Operation not permitted"})
    ));
    assert!(needs_diagnosis("touch first; curl example.invalid"));
    assert!(needs_diagnosis("echo x >> log"));
    assert!(!needs_diagnosis("curl example.invalid"));
}

#[test]
fn windows_denials_enter_recovery_without_treating_normal_errors_as_denials() {
    for code in [0xc0000022u32, 0xc0000135, 0xc0000142] {
        assert!(likely_denied(&json!({"exit_code":code,"stderr":""})));
        assert!(likely_denied(&json!({"exit_code":code as i32,"stderr":""})));
    }
    for error in [
        "Access to the path is denied",
        "UnauthorizedAccessException",
        "拒绝访问",
    ] {
        assert!(likely_denied(&json!({"exit_code":1,"stderr":error})));
        assert!(!likely_denied(&json!({"exit_code":0,"stderr":error})));
    }
    let network = json!({"exit_code":1,"stderr":"An attempt was made to access a socket in a way forbidden by its access permissions"});
    assert!(likely_denied(&network));
    assert!(network_denial(&network));
    assert!(!likely_denied(
        &json!({"exit_code":1,"stderr":"ParserError: Missing closing ')'"})
    ));
}

#[cfg(windows)]
#[test]
fn windows_scope_discloses_effective_read_only_shell_permissions() {
    let root = std::env::temp_dir();
    let plan = Plan::new(
        "Get-ChildItem".into(),
        root.clone(),
        root.clone(),
        root.join("private"),
        "workspace-write".into(),
        root.join("scratch"),
    );
    assert_eq!(plan.context()["backend"], "windows-lpac");
    assert_eq!(plan.context()["file_mode"], "workspace-write");
    assert_eq!(plan.context()["effective_file_mode"], "read-only");
    assert!(plan.launch().is_err()); // Never accidentally reach Seatbelt/host spawn.
}

#[cfg(target_os = "macos")]
fn fixture() -> (tempfile::TempDir, Plan) {
    assert!(
        available(),
        "Real OS tests must run outside a parent sandbox: {}",
        status()
    );
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for name in ["project", "private", "scratch", "outside"] {
        std::fs::create_dir(root.join(name)).unwrap();
    }
    let p = Plan::new(
        String::new(),
        root.join("project"),
        root.join("project"),
        root.join("private"),
        "workspace-write".into(),
        root.join("scratch"),
    );
    (dir, p)
}

#[cfg(target_os = "macos")]
async fn execute(plan: &mut Plan, command: String) -> Value {
    plan.command = command;
    crate::processes::execute_spooled(plan, 5, &CancellationToken::new(), None)
        .await
        .unwrap()
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires a real macOS host outside the calling agent sandbox"]
async fn seatbelt_enforces_files_children_renames_and_read_only() {
    let (dir, mut plan) = fixture();
    let root = dir.path().canonicalize().unwrap();
    let outside = root.join("outside/secret");
    std::fs::write(&outside, "fixture-secret").unwrap();
    std::fs::write(plan.project.join(".env"), "fake-token").unwrap();
    std::os::unix::fs::symlink(&outside, plan.project.join("alias")).unwrap();
    std::fs::hard_link(&outside, plan.project.join("hardlink")).unwrap();
    std::fs::create_dir(plan.project.join(".git")).unwrap();
    std::fs::hard_link(&outside, plan.project.join(".git/hidden-link")).unwrap();
    for command in [
        "printf hello",
        "touch ordinary",
        "mkdir directory",
        "mv directory renamed",
    ] {
        let out = execute(&mut plan, command.into()).await;
        assert_eq!(out["exit_code"], 0, "{command}: {out}");
    }
    #[cfg(target_arch = "aarch64")]
    if std::path::Path::new("/Library/Apple/usr/libexec/oah/libRosettaRuntime").exists() {
        let out = execute(
            &mut plan,
            "/usr/bin/arch -x86_64 /bin/sh -c 'printf translated'".into(),
        )
        .await;
        assert_eq!(out["exit_code"], 0, "Rosetta launch: {out}");
        assert_eq!(out["stdout"], "translated");
    }
    for command in [
        format!("cat '{}'", outside.display()),
        "cat alias".into(),
        "cat hardlink".into(),
        "cat .git/hidden-link".into(),
        "cat .env".into(),
        "cat .ENV".into(),
        format!("touch '{}/escape'", root.display()),
        "mkdir .git".into(),
        "mkdir .agents".into(),
        "sh -c 'cat .env'".into(),
        format!(
            "mv '{}' '{}-moved'",
            plan.project.display(),
            plan.project.display()
        ),
    ] {
        let out = execute(&mut plan, command.clone()).await;
        assert_ne!(out["exit_code"], 0, "{command}: {out}");
        assert!(!out["stdout"].as_str().unwrap().contains("fixture-secret"));
    }
    plan.mode = "read-only".into();
    assert_ne!(
        execute(&mut plan, "touch forbidden".into()).await["exit_code"],
        0
    );
    assert!(!plan.project.join("forbidden").exists());
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "fixture-secret");
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires a real macOS host outside the calling agent sandbox"]
async fn seatbelt_network_is_separate_from_file_access() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (_dir, mut plan) = fixture();
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut client, _) = server.accept().await.unwrap();
        let mut buf = [0; 4096];
        let _ = client.read(&mut buf).await;
        client
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
    });
    let command = format!("/usr/bin/curl -fsS --connect-timeout 1 --max-time 2 http://{address}/");
    let blocked = execute(&mut plan, command.clone()).await;
    assert_ne!(blocked["exit_code"], 0, "{blocked}");
    assert!(likely_denied(&blocked), "{blocked}");
    plan.network = true;
    let out = execute(&mut plan, command).await;
    assert_eq!(out["exit_code"], 0, "{out}");
    assert_eq!(out["stdout"], "ok");
    let secret = plan.private.join("secret");
    std::fs::write(&secret, "private-fixture").unwrap();
    assert_ne!(
        execute(&mut plan, format!("cat '{}'", secret.display())).await["exit_code"],
        0
    );
    task.abort();
}
