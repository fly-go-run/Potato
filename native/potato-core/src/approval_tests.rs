//! Exercise the actual tool dispatch and approval API without model services.
use crate::*;
use std::time::Duration;

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
    decide(&r, &a, "approve", "exact").await.unwrap();
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
            &body,
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
