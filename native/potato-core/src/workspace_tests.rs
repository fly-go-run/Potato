use super::*;

#[test]
fn only_exact_legacy_defaults_upgrade_and_keep_language_and_metadata() {
    for language in ["zh", "en", "id", "ru"] {
        let dir = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(dir.path()).unwrap();
        runtime.db().unwrap().put("language", &json!("en")).unwrap();
        runtime.db().unwrap().put("workspace_documents", &json!({
            "AGENTS.md": {"content":legacy_template("AGENTS.md",language),"created_time":42,"modified_time":43},
            "PROFILE.md": {"content":"Custom preference", "created_time":44,"modified_time":45}
        })).unwrap();
        runtime
            .db()
            .unwrap()
            .put("system_prompt_files", &json!(["PROFILE.md", "AGENTS.md"]))
            .unwrap();
        let docs = runtime.documents(false).unwrap();
        assert_eq!(
            docs["AGENTS.md"]["content"],
            template("AGENTS.md", language)
        );
        assert_eq!(docs["AGENTS.md"]["created_time"], 42);
        assert_eq!(docs["PROFILE.md"]["modified_time"], 45);
        assert_eq!(docs["PROFILE.md"]["content"], "Custom preference");
        assert_eq!(
            runtime
                .db()
                .unwrap()
                .get("system_prompt_files", Value::Null)
                .unwrap(),
            json!(["PROFILE.md", "AGENTS.md"])
        );
        drop(runtime);
        let runtime = Runtime::open(dir.path()).unwrap();
        assert_eq!(runtime.documents(false).unwrap(), docs);

        let custom = format!("{}\nCustom rule", legacy_template("AGENTS.md", language));
        runtime
            .document_request(
                "PUT",
                "/api/workspace/files/AGENTS.md",
                &json!({"content":custom}),
            )
            .unwrap();
        let saved = runtime.documents(false).unwrap();
        assert_eq!(saved["AGENTS.md"]["content"], custom);
        runtime.system_prompt().unwrap();
        assert_eq!(runtime.documents(false).unwrap(), saved);
    }
}

#[test]
fn core_survives_document_selection_and_empty_or_deleted_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(dir.path()).unwrap();
    runtime
        .document_request("PUT", "/api/workspace/system-prompt-files", &json!([]))
        .unwrap();
    assert_eq!(runtime.system_prompt().unwrap(), crate::prompts::CORE);
    runtime
        .document_request("DELETE", "/api/workspace/files/AGENTS.md", &Value::Null)
        .unwrap();
    assert!(runtime.documents(false).unwrap().get("AGENTS.md").is_none());
    assert_eq!(runtime.system_prompt().unwrap(), crate::prompts::CORE);
}

#[test]
fn prompt_limit_counts_document_headers_as_well_as_content() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(dir.path()).unwrap();
    let header = "\n# Workspace guidance: AGENTS.md\n";
    let text = "x".repeat(MAX_CONTENT - crate::prompts::CORE.len() - header.len() - 1);
    runtime
        .document_request(
            "PUT",
            "/api/workspace/files/AGENTS.md",
            &json!({"content":text}),
        )
        .unwrap();
    assert_eq!(runtime.system_prompt().unwrap().len(), MAX_CONTENT);
    runtime
        .document_request(
            "PUT",
            "/api/workspace/files/AGENTS.md",
            &json!({"content":format!("{text}x")}),
        )
        .unwrap();
    assert_eq!(runtime.system_prompt().unwrap_err().status, 413);
}
