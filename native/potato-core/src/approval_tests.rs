//! Exercise the actual tool dispatch and approval API without model services.
use crate::*;
use std::time::Duration;

#[test]
fn prompt_permissions_follow_effective_policy_and_file_mode() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(dir.path()).unwrap();
    runtime.db().unwrap().put("running", &json!({"approval_level":"STRICT","sandbox_mode":"read-only","reviewer":"model"})).unwrap();
    for level in ["AUTO", "STRICT", "NEVER"] {
        for mode in ["read-only", "workspace-write", "danger-full-access"] {
            let body = json!({"request_context":{"approval_level":level,"sandbox_mode":mode}});
            let text = runtime.approval_guidance(&body).unwrap();
            assert!(text.contains(&format!("Approval policy: {level}.")));
            assert!(text.contains(&format!("File access: {mode}.")));
            assert!(text.contains("Shell sandbox:"));
            assert_eq!(text.contains("Shell runs without OS file isolation"), mode == "danger-full-access");
            assert_eq!(text.contains("independent model auto-review"), level == "AUTO");
            assert_eq!(text.contains("interactive questions are rejected"), level == "NEVER");
            assert_eq!(text.contains("Project file and project memory writes are disabled"), mode == "read-only");
            assert_eq!(text.contains("reusable grants do not skip it"), level == "STRICT");
        }
    }
    let fallback = runtime.approval_guidance(&json!({})).unwrap();
    assert!(fallback.contains("Approval policy: STRICT."));
    assert!(fallback.contains("File access: read-only."));
}

#[tokio::test]
async fn office_template_dispatch_preserves_source_and_confines_reads() {
    let (_dir, r, mut body) = setup();
    let project = r.turn_project(&body).await.unwrap();
    tool(&r,&body,"create_office_file",json!({"file_path":"source.docx","format":"docx","document":{"title":"{{title}}","paragraphs":["客户：{{name}}"]}})).await.unwrap();
    let original = std::fs::read(project.join("source.docx")).unwrap();
    let args = json!({"file_path":"filled.docx","template_path":"source.docx","format":"docx","replacements":{"{{title}}":"报告","{{name}}":"研发团队"}});
    tool(&r, &body, "fill_office_template", args.clone())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(project.join("source.docx")).unwrap(),
        original
    );
    assert!(lock(&r.approvals).unwrap().is_empty());
    for source in ["../source.docx", ".git/source.docx"] {
        let mut bad = args.clone();
        bad["template_path"] = json!(source);
        bad["file_path"] = json!("bad.docx");
        assert!(tool(&r, &body, "fill_office_template", bad).await.is_err());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(project.join("source.docx"), project.join("alias.docx"))
            .unwrap();
        let mut bad = args.clone();
        bad["template_path"] = json!("alias.docx");
        bad["file_path"] = json!("bad.docx");
        assert_eq!(
            tool(&r, &body, "fill_office_template", bad)
                .await
                .unwrap_err()
                .status,
            403
        );
    }
    body["request_context"]["sandbox_mode"] = json!("read-only");
    assert_eq!(
        tool(&r, &body, "fill_office_template", args)
            .await
            .unwrap_err()
            .status,
        403
    );
}

#[tokio::test]
async fn office_artifacts_use_project_policy_and_never_overwrite() {
    let (_dir, r, mut body) = setup();
    let project = r.turn_project(&body).await.unwrap();
    let args = json!({"file_path":"report.docx","format":"docx","document":{"title":"中文报告","paragraphs":["验证内容"]}});
    let result = tool(&r, &body, "create_office_file", args.clone())
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(result["path"], json!(project.join("report.docx")));
    assert!(lock(&r.approvals).unwrap().is_empty());
    let before = std::fs::read(project.join("report.docx")).unwrap();
    assert_eq!(
        tool(&r, &body, "create_office_file", args.clone())
            .await
            .unwrap_err()
            .status,
        409
    );
    assert_eq!(before, std::fs::read(project.join("report.docx")).unwrap());
    for path in ["../outside.docx", ".git/report.docx"] {
        let mut bad = args.clone();
        bad["file_path"] = json!(path);
        assert!(tool(&r, &body, "create_office_file", bad).await.is_err());
    }
    body["request_context"]["sandbox_mode"] = json!("read-only");
    let mut readonly = args;
    readonly["file_path"] = json!("blocked.docx");
    assert_eq!(
        tool(&r, &body, "create_office_file", readonly)
            .await
            .unwrap_err()
            .status,
        403
    );
    assert!(!project.join("blocked.docx").exists());
}

#[tokio::test]
async fn project_memory_and_artifacts_are_files_but_runtime_config_stays_sensitive() {
    let (_dir, r, body) = setup();
    let project = r.turn_project(&body).await.unwrap();
    tool(
        &r,
        &body,
        "memory_write",
        json!({"scope":"project","path":"note.md","content":"discoverable fact"}),
    )
    .await
    .unwrap();
    tool(
        &r,
        &body,
        "read_file",
        json!({"file_path":".potato/memory/note.md"}),
    )
    .await
    .unwrap();
    tool(
        &r,
        &body,
        "edit_file",
        json!({"file_path":".potato/memory/note.md","old_text":"fact","new_text":"decision"}),
    )
    .await
    .unwrap();
    std::fs::create_dir(project.join(".potato/artifacts")).unwrap();
    std::fs::write(
        project.join(".potato/artifacts/result.txt"),
        "discoverable artifact",
    )
    .unwrap();
    std::fs::write(
        project.join(".potato/config.json"),
        "discoverable private config",
    )
    .unwrap();
    std::fs::write(
        project.join(".potato/memory/.env"),
        "discoverable credential",
    )
    .unwrap();
    for root in [".", ".potato/memory"] {
        let result = tool(
            &r,
            &body,
            "grep_search",
            json!({"path":root,"pattern":"discoverable"}),
        )
        .await
        .unwrap();
        assert!(result.contains("decision"), "{result}");
        assert!(
            !result.contains("discoverable private config")
                && !result.contains("discoverable credential")
        );
    }
    let found = tool(&r, &body, "glob_search", json!({"pattern":"**/*"}))
        .await
        .unwrap();
    assert!(
        found.contains("note.md") && found.contains("result.txt"),
        "{found}"
    );
    assert!(!found.contains("config.json") && !found.contains(".env"));
    let mut never = body.clone();
    never["request_context"]["approval_level"] = json!("NEVER");
    assert_eq!(
        tool(
            &r,
            &never,
            "read_file",
            json!({"file_path":".potato/config.json"})
        )
        .await
        .unwrap_err()
        .status,
        403
    );
    assert!(lock(&r.approvals).unwrap().is_empty());
}

fn setup() -> (tempfile::TempDir, Arc<Runtime>, Value) {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let runtime = Runtime::open(&dir.path().join("runtime")).unwrap();
    // These cases exercise explicit manual approval, independently of installation defaults.
    runtime.db().unwrap().put("running", &json!({"approval_level":"AUTO","sandbox_mode":"workspace-write","reviewer":"user"})).unwrap();
    let body = json!({"request_context":{"potato.coding_project_dir":project}});
    (dir, runtime, body)
}
async fn tool(r: &Runtime, body: &Value, name: &str, args: Value) -> Result<String> {
    tokio::time::timeout(
        Duration::from_secs(2),
        r.execute_tool(
            "s",
            name,
            &args,
            body,
            &CancellationToken::new(),
            &(Arc::new(|_| Ok(())) as Emit),
        ),
    )
    .await
    .expect("tool unexpectedly waited for approval")
}
fn pending_tool(
    r: &Arc<Runtime>,
    body: &Value,
    name: &str,
    args: Value,
) -> tokio::task::JoinHandle<Result<String>> {
    let r = r.clone();
    let body = body.clone();
    let name = name.to_owned();
    tokio::spawn(async move { tool(&r, &body, &name, args).await })
}
async fn pending(r: &Runtime) -> Value {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(a) = lock(&r.approvals).unwrap().values().next() {
                return a.view.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("expected an approval card")
}
async fn decide(r: &Runtime, a: &Value, action: &str, scope: &str) -> Result<Value> {
    r.request(
        "POST",
        &format!("/api/approval/{action}"),
        json!({"request_id":a["request_id"],"session_id":"s","user_id":"default","scope":scope}),
    )
    .await
}

#[tokio::test]
async fn auto_project_read_search_edit_has_zero_prompts() {
    let (_dir, r, body) = setup();
    for (name, args) in [
        (
            "write_file",
            json!({"file_path":"hello.txt","content":"before"}),
        ),
        ("read_file", json!({"file_path":"hello.txt"})),
        ("list_directory", json!({"path":"."})),
        ("grep_search", json!({"pattern":"before"})),
        ("glob_search", json!({"pattern":"*.txt"})),
        (
            "edit_file",
            json!({"file_path":"hello.txt","old_text":"before","new_text":"after"}),
        ),
        (
            "append_file",
            json!({"file_path":"hello.txt","content":"!"}),
        ),
        (
            "memory_write",
            json!({"path":"note.md","content":"Project fact","scope":"project"}),
        ),
    ] {
        tool(&r, &body, name, args).await.unwrap();
    }
    assert!(lock(&r.approvals).unwrap().is_empty());
    assert!(
        tool(&r, &body, "read_file", json!({"file_path":"hello.txt"}))
            .await
            .unwrap()
            .contains("after!")
    );
    let audit = r
        .request("GET", "/api/approval/audit?session_id=s", Value::Null)
        .await
        .unwrap();
    assert_eq!(audit.as_array().unwrap().len(), 9);
    assert!(audit
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["outcome"] == "allowed"));
}

#[tokio::test]
async fn never_rejects_sensitive_external_and_interactive_work_without_waiting() {
    let (dir, r, mut body) = setup();
    body["request_context"]["approval_level"] = json!("NEVER");
    let project = r.turn_project(&body).await.unwrap();
    std::fs::write(project.join(".env"), "secret fixture").unwrap();
    std::fs::write(dir.path().join("outside"), "outside fixture").unwrap();
    for (name, args) in [
        ("read_file", json!({"file_path":project.join(".env")})),
        ("read_file", json!({"file_path":dir.path().join("outside")})),
        (
            "write_file",
            json!({"file_path":"AGENTS.md","content":"new authority"}),
        ),
        (
            "execute_shell_command",
            json!({"command":"echo denied","sandbox_permissions":"require_escalated","justification":"test"}),
        ),
        (
            "memory_write",
            json!({"path":"global.md","content":"global fact"}),
        ),
        ("request_user_input", json!({"title":"question"})),
    ] {
        assert_eq!(
            tool(&r, &body, name, args).await.unwrap_err().status,
            403,
            "{name}"
        );
    }
    tool(
        &r,
        &body,
        "write_file",
        json!({"file_path":"ordinary.txt","content":"okay"}),
    )
    .await
    .unwrap();
    assert!(lock(&r.approvals).unwrap().is_empty());
    assert!(lock(&r.questions).unwrap().is_empty());
    assert!(!project.join("AGENTS.md").exists());
}

#[tokio::test]
async fn strict_once_is_not_remembered_and_identity_and_scope_are_checked() {
    let (_dir, r, mut body) = setup();
    body["request_context"]["approval_level"] = json!("STRICT");
    let args = json!({"path":"."});
    let run = pending_tool(&r, &body, "list_directory", args.clone());
    let a = pending(&r).await;
    for (session, user) in [("other", "default"), ("s", "other")] {
        assert_eq!(
            r.request(
                "POST",
                "/api/approval/approve",
                json!({"request_id":a["request_id"],"session_id":session,"user_id":user})
            )
            .await
            .unwrap_err()
            .status,
            403
        );
    }
    assert_eq!(
        decide(&r, &a, "approve", "session")
            .await
            .unwrap_err()
            .status,
        400
    );
    decide(&r, &a, "approve", "exact").await.unwrap();
    run.await.unwrap().unwrap();
    assert_eq!(
        decide(&r, &a, "approve", "exact").await.unwrap_err().status,
        404
    );
    let run = pending_tool(&r, &body, "list_directory", args);
    let a = pending(&r).await;
    decide(&r, &a, "deny", "exact").await.unwrap();
    assert_eq!(run.await.unwrap().unwrap_err().status, 403);
}

#[tokio::test]
async fn session_grant_is_exact_revocable_and_never_overrides_never() {
    let (dir, r, body) = setup();
    let path = dir.path().join("outside");
    std::fs::write(&path, "outside").unwrap();
    let args = json!({"file_path":path});
    let run = pending_tool(&r, &body, "read_file", args.clone());
    let a = pending(&r).await;
    decide(&r, &a, "approve", "session").await.unwrap();
    run.await.unwrap().unwrap();
    tool(&r, &body, "read_file", args.clone()).await.unwrap();
    let mut never = body.clone();
    never["request_context"]["approval_level"] = json!("NEVER");
    assert_eq!(
        tool(&r, &never, "read_file", args.clone())
            .await
            .unwrap_err()
            .status,
        403
    );
    let run = pending_tool(
        &r,
        &body,
        "read_file",
        json!({"file_path":path,"start_line":1}),
    );
    let a = pending(&r).await;
    decide(&r, &a, "deny", "exact").await.unwrap();
    run.await.unwrap().unwrap_err();
    r.request(
        "POST",
        "/api/approval/revoke-session",
        json!({"session_id":"s","user_id":"default"}),
    )
    .await
    .unwrap();
    let run = pending_tool(&r, &body, "read_file", args);
    let a = pending(&r).await;
    decide(&r, &a, "deny", "exact").await.unwrap();
    run.await.unwrap().unwrap_err();
}

#[tokio::test]
async fn global_memory_file_writes_require_approval_even_inside_project() {
    let (dir, r, _) = setup();
    let memory = r.memory_root().unwrap();
    let target = memory.join("MEMORY.md");
    let parent = dir.path().canonicalize().unwrap();
    for body in [
        json!({}),
        json!({"request_context":{"potato.coding_project_dir":parent}}),
    ] {
        let project = r.turn_project(&body).await.unwrap();
        let relative = target.strip_prefix(&project).unwrap();
        for (name, args) in [
            ("write_file", json!({"path":relative,"content":"changed"})),
            (
                "edit_file",
                json!({"path":target,"old_text":"original","new_text":"changed"}),
            ),
            ("append_file", json!({"path":relative,"content":"changed"})),
        ] {
            std::fs::write(&target, "original").unwrap();
            let mut never = body.clone();
            never["request_context"]["approval_level"] = json!("NEVER");
            assert_eq!(
                tool(&r, &never, name, args.clone())
                    .await
                    .unwrap_err()
                    .status,
                403
            );
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "original");
            let run = pending_tool(&r, &body, name, args);
            let a = pending(&r).await;
            assert_eq!(
                a["permission_increment"],
                "Change persistent user-wide memory"
            );
            assert_eq!(a["justification"], "");
            decide(&r, &a, "approve", "exact").await.unwrap();
            run.await.unwrap().unwrap();
            assert!(std::fs::read_to_string(&target)
                .unwrap()
                .contains("changed"));
        }
    }
}

#[tokio::test]
async fn deleting_chat_clears_session_approval_state() {
    let (dir, r, body) = setup();
    let chat = r.db().unwrap().ensure_chat("s", "delete me").unwrap();
    let path = dir.path().join("outside");
    std::fs::write(&path, "outside").unwrap();
    let run = pending_tool(&r, &body, "read_file", json!({"path":path}));
    let a = pending(&r).await;
    assert!(a["justification"].is_string());
    decide(&r, &a, "approve", "session").await.unwrap();
    run.await.unwrap().unwrap();
    assert_eq!(r.approval_grant_count("s").unwrap(), 1);
    assert!(!r
        .request("GET", "/api/approval/audit?session_id=s", Value::Null)
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    r.request(
        "DELETE",
        &format!("/api/chats/{}", string(&chat, "id")),
        Value::Null,
    )
    .await
    .unwrap();
    assert_eq!(r.approval_grant_count("s").unwrap(), 0);
    assert_eq!(
        r.request("GET", "/api/approval/audit?session_id=s", Value::Null)
            .await
            .unwrap(),
        json!([])
    );
}

#[tokio::test]
async fn approval_never_widens_read_only_or_project_confinement() {
    let (dir, r, mut body) = setup();
    body["request_context"]["sandbox_mode"] = json!("read-only");
    assert_eq!(
        tool(
            &r,
            &body,
            "write_file",
            json!({"file_path":"blocked","content":"x"})
        )
        .await
        .unwrap_err()
        .status,
        403
    );
    body["request_context"]["sandbox_mode"] = json!("danger-full-access");
    for path in [
        dir.path().join("escape"),
        r.turn_project(&body).await.unwrap().join(".git/config"),
    ] {
        assert!(tool(
            &r,
            &body,
            "write_file",
            json!({"file_path":path,"content":"x"})
        )
        .await
        .is_err());
        assert!(!path.exists());
    }
    assert!(lock(&r.approvals).unwrap().is_empty());
}

#[tokio::test]
async fn sensitive_write_keeps_conflict_check_and_policy_change_invalidates_prompt() {
    let (_dir, r, body) = setup();
    let path = r.turn_project(&body).await.unwrap().join("AGENTS.md");
    std::fs::write(&path, "before").unwrap();
    let run = pending_tool(
        &r,
        &body,
        "write_file",
        json!({"file_path":path,"content":"replacement"}),
    );
    let a = pending(&r).await;
    std::fs::write(&path, "human edit").unwrap();
    decide(&r, &a, "approve", "exact").await.unwrap();
    assert_eq!(run.await.unwrap().unwrap_err().status, 409);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "human edit");
    let run = pending_tool(&r, &body, "read_file", json!({"file_path":path}));
    let a = pending(&r).await;
    r.request(
        "PUT",
        "/api/workspace/running-config",
        json!({"approval_level":"NEVER"}),
    )
    .await
    .unwrap();
    assert_eq!(
        decide(&r, &a, "approve", "exact").await.unwrap_err().status,
        404
    );
    assert_eq!(run.await.unwrap().unwrap_err().status, 409);
}

#[cfg(unix)]
#[tokio::test]
async fn shell_escalation_is_once_and_symlinks_do_not_hide_sensitive_targets() {
    let (_dir, r, body) = setup();
    let project = r.turn_project(&body).await.unwrap();
    std::fs::write(project.join(".env"), "secret fixture").unwrap();
    std::os::unix::fs::symlink(project.join(".env"), project.join("ordinary")).unwrap();
    let mut never = body.clone();
    never["request_context"]["approval_level"] = json!("NEVER");
    assert_eq!(
        tool(&r, &never, "read_file", json!({"file_path":"ordinary"}))
            .await
            .unwrap_err()
            .status,
        403
    );
    let search = tool(&r, &body, "grep_search", json!({"pattern":"secret"}))
        .await
        .unwrap();
    assert!(!search.contains("secret fixture"));
    assert_eq!(
        tool(
            &r,
            &never,
            "execute_shell_command",
            json!({"command":"printf approved"})
        )
        .await
        .unwrap_err()
        .status,
        403
    );
    let run = pending_tool(
        &r,
        &body,
        "execute_shell_command",
        json!({"command":"printf approved","timeout":1,"sandbox_permissions":"require_escalated","justification":"Test process execution"}),
    );
    let a = pending(&r).await;
    decide(&r, &a, "approve", "exact").await.unwrap();
    assert!(run.await.unwrap().unwrap().contains("approved"));
    assert_eq!(r.file_mode(&body).unwrap(), "workspace-write");
}

#[tokio::test]
async fn cancellation_revocation_expiry_and_bad_config_fail_closed() {
    let (_dir, r, mut body) = setup();
    body["request_context"]["approval_level"] = json!("STRICT");
    let token = CancellationToken::new();
    let r2 = r.clone();
    let b = body.clone();
    let t = token.clone();
    let run = tokio::spawn(async move {
        r2.execute_tool(
            "s",
            "list_directory",
            &json!({"path":"."}),
            &b,
            &t,
            &(Arc::new(|_| Ok(())) as Emit),
        )
        .await
    });
    let a = pending(&r).await;
    token.cancel();
    assert!(run.await.unwrap().is_err());
    assert_eq!(
        decide(&r, &a, "approve", "exact").await.unwrap_err().status,
        404
    );
    let run = pending_tool(&r, &body, "list_directory", json!({"path":"."}));
    let a = pending(&r).await;
    lock(&r.approvals)
        .unwrap()
        .get_mut(a["request_id"].as_str().unwrap())
        .unwrap()
        .view["created_at"] = json!(0);
    assert_eq!(
        decide(&r, &a, "approve", "exact").await.unwrap_err().status,
        409
    );
    run.await.unwrap().unwrap_err();
    let run = pending_tool(&r, &body, "list_directory", json!({"path":"."}));
    let a = pending(&r).await;
    r.revoke_approval_grants("s").unwrap();
    run.await.unwrap().unwrap_err();
    assert_eq!(
        decide(&r, &a, "approve", "exact").await.unwrap_err().status,
        404
    );
    for level in [json!("OFF"), json!("bogus"), json!(true)] {
        assert_eq!(
            r.request(
                "PUT",
                "/api/workspace/running-config",
                json!({"approval_level":level})
            )
            .await
            .unwrap_err()
            .status,
            400
        );
    }
}

async fn directory_reply(
    r: &Runtime,
    a: &Value,
    scope: &str,
    directory: &std::path::Path,
) -> Result<Value> {
    r.request("POST","/api/approval/approve",json!({"request_id":a["request_id"],"session_id":"s","user_id":"default","scope":scope,"directory":directory,"recursive":true})).await
}

#[tokio::test]
async fn directory_grants_reuse_reads_aliases_search_and_survive_restart() {
    let (dir, r, body) = setup();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("a.txt"), "one\ntwo\nthree").unwrap();
    std::fs::write(outside.join("b.txt"), "another").unwrap();
    let first = pending_tool(
        &r,
        &body,
        "read_file",
        json!({"file_path":outside.join("a.txt")}),
    );
    let card = pending(&r).await;
    assert_eq!(card["allow_directory"], true);
    let second = pending_tool(
        &r,
        &body,
        "read_file",
        json!({"path":outside.join("b.txt")}),
    );
    tokio::task::yield_now().await;
    assert_eq!(lock(&r.approvals).unwrap().len(), 1);
    directory_reply(&r, &card, "persistent_directory", &outside)
        .await
        .unwrap();
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    for (name, args) in [
        (
            "read_file",
            json!({"path":outside.join("a.txt"),"start_line":2}),
        ),
        ("list_directory", json!({"path":outside})),
        ("grep_search", json!({"path":outside,"pattern":"one"})),
        ("grep_search", json!({"path":outside,"pattern":"two"})),
        ("glob_search", json!({"path":outside,"pattern":"*.txt"})),
    ] {
        tool(&r, &body, name, args).await.unwrap();
    }
    let root = r.root.clone();
    drop(r);
    let r = Runtime::open(&root).unwrap();
    tool(
        &r,
        &body,
        "read_file",
        json!({"file_path":outside.join("b.txt")}),
    )
    .await
    .unwrap();
    let rules = r.permission_rules_api("GET", &Value::Null).unwrap();
    r.permission_rules_api("DELETE", &json!({"id":rules["rules"][0]["id"]}))
        .unwrap();
    let denied = pending_tool(
        &r,
        &body,
        "read_file",
        json!({"path":outside.join("b.txt")}),
    );
    let card = pending(&r).await;
    decide(&r, &card, "deny", "exact").await.unwrap();
    assert!(denied.await.unwrap().is_err());
}

#[tokio::test]
async fn session_directories_never_cross_sessions_or_override_strict_never_or_writes() {
    let (dir, r, body) = setup();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let file = outside.join("a");
    std::fs::write(&file, "hi").unwrap();
    let run = pending_tool(&r, &body, "read_file", json!({"path":file}));
    let card = pending(&r).await;
    directory_reply(&r, &card, "session_directory", &outside)
        .await
        .unwrap();
    run.await.unwrap().unwrap();
    assert!(r
        .directory_decision("other", "read_file", file.to_str().unwrap())
        .unwrap()
        .is_none());
    assert!(r
        .directory_decision("s", "write_file", file.to_str().unwrap())
        .unwrap()
        .is_none());
    assert!(r
        .directory_decision("s", "execute_shell_command", outside.to_str().unwrap())
        .unwrap()
        .is_none());
    let mut strict = body.clone();
    strict["request_context"]["approval_level"] = json!("STRICT");
    let run = pending_tool(&r, &strict, "read_file", json!({"path":file}));
    let card = pending(&r).await;
    assert_eq!(card["allow_directory"], false);
    decide(&r, &card, "deny", "exact").await.unwrap();
    assert!(run.await.unwrap().is_err());
    strict["request_context"]["approval_level"] = json!("NEVER");
    assert_eq!(
        tool(&r, &strict, "read_file", json!({"path":file}))
            .await
            .unwrap_err()
            .status,
        403
    );
    r.revoke_approval_grants("s").unwrap();
    assert!(r
        .directory_decision("s", "read_file", file.to_str().unwrap())
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn directory_rules_protect_prefixes_sensitive_paths_and_denied_subtrees() {
    let (dir, r, body) = setup();
    let outside = dir.path().join("outside");
    let similar = dir.path().join("outside-similar");
    std::fs::create_dir_all(outside.join("private")).unwrap();
    std::fs::create_dir(&similar).unwrap();
    std::fs::write(outside.join(".env"), "secret").unwrap();
    r.permission_rules_api("POST", &json!({"path":outside}))
        .unwrap();
    assert!(r
        .directory_decision("s", "read_file", similar.to_str().unwrap())
        .unwrap()
        .is_none());
    assert!(r
        .directory_decision("s", "read_file", outside.join(".env").to_str().unwrap())
        .unwrap()
        .is_none());
    r.permission_rules_api(
        "POST",
        &json!({"path":outside.join("private"),"decision":"deny","operations":["read"]}),
    )
    .unwrap();
    assert_eq!(
        tool(
            &r,
            &body,
            "grep_search",
            json!({"path":outside,"pattern":"secret"})
        )
        .await
        .unwrap_err()
        .status,
        403
    );
    assert!(r
        .permission_rules_api("POST", &json!({"path":outside,"operations":["write"]}))
        .is_err());
    assert!(r
        .permission_rules_api("POST", &json!({"path":r.root}))
        .is_err());
}

#[tokio::test]
async fn pending_directory_grants_cannot_revive_after_policy_change_or_revoke() {
    let (dir, r, body) = setup();
    let file = dir.path().join("a");
    std::fs::write(&file, "hello").unwrap();
    for policy_change in [false, true] {
        let run = pending_tool(&r, &body, "read_file", json!({"path":file}));
        let card = pending(&r).await;
        if policy_change {
            r.request(
                "PUT",
                "/api/workspace/running-config",
                json!({"approval_level":"AUTO"}),
            )
            .await
            .unwrap();
        } else {
            r.revoke_approval_grants("s").unwrap();
        }
        assert!(
            directory_reply(&r, &card, "persistent_directory", dir.path())
                .await
                .is_err()
        );
        assert!(run.await.unwrap().is_err());
        assert!(r.persistent_rules().unwrap().is_empty());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn directory_replacement_and_symlinks_invalidate_approval_and_saved_grants() {
    let (dir, r, body) = setup();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let file = outside.join("a");
    std::fs::write(&file, "original").unwrap();
    let run = pending_tool(&r, &body, "read_file", json!({"path":file}));
    let card = pending(&r).await;
    std::fs::rename(&outside, dir.path().join("old")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(&file, "changed").unwrap();
    directory_reply(&r, &card, "persistent_directory", &outside)
        .await
        .unwrap();
    assert_eq!(run.await.unwrap().unwrap_err().status, 409);
    assert!(r.persistent_rules().unwrap().is_empty());
    r.permission_rules_api("POST", &json!({"path":outside}))
        .unwrap();
    let escape = dir.path().join("escape");
    std::fs::create_dir(&escape).unwrap();
    std::fs::write(escape.join("private"), "secret").unwrap();
    std::os::unix::fs::symlink(&escape, outside.join("link")).unwrap();
    let run = pending_tool(
        &r,
        &body,
        "read_file",
        json!({"path":outside.join("link/private")}),
    );
    let card = pending(&r).await;
    decide(&r, &card, "deny", "exact").await.unwrap();
    assert!(run.await.unwrap().is_err());
    std::fs::rename(&outside, dir.path().join("old2")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(&file, "new").unwrap();
    assert!(r
        .directory_decision("s", "read_file", file.to_str().unwrap())
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn typed_permissions_migrate_full_access_and_require_configured_reviewer() {
    let (_dir, r, _body) = setup();
    let saved = r
        .request(
            "PUT",
            "/api/workspace/running-config",
            json!({"sandbox_mode":"full-access"}),
        )
        .await
        .unwrap();
    assert_eq!(saved["sandbox_mode"], "danger-full-access");
    assert_eq!(saved["reviewer"], "user");
    let value = r
        .request("GET", "/api/workspace/running-config", Value::Null)
        .await
        .unwrap();
    assert_eq!(value["sandbox_mode"], "danger-full-access");
    assert_eq!(
        r.request(
            "PUT",
            "/api/workspace/running-config",
            json!({"reviewer":"model"})
        )
        .await
        .unwrap_err()
        .status,
        400
    );
}

#[tokio::test]
async fn nonrecursive_rules_and_directory_scope_validation_stay_narrow() {
    let (dir, r, body) = setup();
    let root = dir.path().join("outside");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("a"), "one").unwrap();
    std::fs::write(root.join("nested/b"), "two").unwrap();
    r.permission_rules_api("POST", &json!({"path":root,"recursive":false}))
        .unwrap();
    tool(&r, &body, "read_file", json!({"path":root.join("a")}))
        .await
        .unwrap();
    assert!(r
        .directory_decision("s", "read_file", root.join("nested/b").to_str().unwrap())
        .unwrap()
        .is_none());
    assert!(r
        .directory_decision("s", "grep_search", root.to_str().unwrap())
        .unwrap()
        .is_none());
    let run = pending_tool(
        &r,
        &body,
        "grep_search",
        json!({"path":root,"pattern":"two"}),
    );
    let card = pending(&r).await;
    assert_eq!(r.request("POST","/api/approval/approve",json!({"request_id":card["request_id"],"session_id":"s","user_id":"default","scope":"persistent_directory","directory":root,"recursive":false})).await.unwrap_err().status,400);
    decide(&r, &card, "deny", "exact").await.unwrap();
    assert!(run.await.unwrap().is_err());
    r.permission_rules_api(
        "POST",
        &json!({"path":root,"recursive":false,"decision":"deny"}),
    )
    .unwrap();
    assert!(matches!(
        r.directory_decision("s", "read_file", root.join("a").to_str().unwrap())
            .unwrap(),
        Some((crate::permissions::Decision::Deny, _))
    ));
    assert!(r
        .directory_decision("s", "read_file", root.join("nested/b").to_str().unwrap())
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn cancelled_directory_approval_never_persists_and_session_grants_do_not_restart() {
    let (dir, r, body) = setup();
    let file = dir.path().join("outside");
    std::fs::write(&file, "text").unwrap();
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let rr = r.clone();
    let bb = body.clone();
    let ff = file.clone();
    let run = tokio::spawn(async move {
        rr.execute_tool(
            "s",
            "read_file",
            &json!({"path":ff}),
            &bb,
            &task_cancel,
            &(Arc::new(|_| Ok(())) as Emit),
        )
        .await
    });
    let card = pending(&r).await;
    cancel.cancel();
    let _ = directory_reply(&r, &card, "persistent_directory", dir.path()).await;
    assert!(run.await.unwrap().is_err());
    assert!(r.persistent_rules().unwrap().is_empty());
    let run = pending_tool(&r, &body, "read_file", json!({"path":file}));
    let card = pending(&r).await;
    directory_reply(&r, &card, "session_directory", dir.path())
        .await
        .unwrap();
    run.await.unwrap().unwrap();
    let root = r.root.clone();
    drop(r);
    let r = Runtime::open(&root).unwrap();
    assert!(r
        .directory_decision("s", "read_file", file.to_str().unwrap())
        .unwrap()
        .is_none());
}
