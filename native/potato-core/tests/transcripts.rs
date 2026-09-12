//! Public API coverage for file-backed conversation history.
use base64::Engine;
use potato_core::Runtime;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::Path,
};

async fn import(runtime: &Runtime, id: &str, messages: Value) {
    let result = runtime.request("POST", "/api/native/import-history", json!({
        "format":"potato-native-history-v1",
        "chats":[{"spec":{"id":id,"session_id":id,"name":"Archive fixture"},"messages":messages}]
    })).await.unwrap();
    assert_eq!(result["imported"], 1);
}

async fn history(runtime: &Runtime, id: &str) -> Value {
    runtime
        .request("GET", &format!("/api/chats/{id}"), Value::Null)
        .await
        .unwrap()["messages"]
        .clone()
}

async fn archive(runtime: &Runtime, id: &str) -> Value {
    runtime
        .request("GET", &format!("/api/chats/{id}/archive"), Value::Null)
        .await
        .unwrap()
}

async fn export(runtime: &Runtime) -> Value {
    let response = runtime
        .request("GET", "/api/workspace/download", Value::Null)
        .await
        .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(response["native_binary"].as_str().unwrap())
        .unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut source = String::new();
    zip.by_name("history.json")
        .unwrap()
        .read_to_string(&mut source)
        .unwrap();
    serde_json::from_str(&source).unwrap()
}

#[tokio::test]
async fn legacy_id_archive_restarts_and_exports_exact_display_history() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(root.path()).unwrap();
    let id = "legacy-session-not-a-uuid";
    let marker = "UNIQUE_BODY_ONLY_IN_TRANSCRIPT_你好";
    let messages = json!([
        {"id":"user","type":"message","role":"user","content":[{"type":"text","text":marker}]},
        {"id":"display-only","type":"unknown_preview","content":[{"type":"image","image_url":"data:image/png;base64,aGVsbG8="}]},
        {"id":"assistant","type":"message","role":"assistant","content":[{"type":"text","text":"回答"}],"metadata":{"preserve":true}}
    ]);
    import(&runtime, id, messages.clone()).await;
    assert_eq!(history(&runtime, id).await, messages);
    let location = archive(&runtime, id).await;
    assert_eq!(location["format"], "potato-transcript-v1");
    let archive_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, id.as_bytes());
    let expected = root
        .path()
        .join("workspace/history/sessions")
        .join(archive_id.to_string());
    // Canonicalization allows the macOS /var -> /private/var temporary directory alias.
    assert_eq!(
        Path::new(location["session_dir"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        expected.canonicalize().unwrap()
    );
    assert_eq!(
        Path::new(location["transcript_path"].as_str().unwrap())
            .file_name()
            .unwrap(),
        "transcript.jsonl"
    );
    assert!(Path::new(location["transcript_path"].as_str().unwrap()).is_absolute());
    assert!(Path::new(location["project_index"].as_str().unwrap()).is_file());
    let source = std::fs::read_to_string(expected.join("transcript.jsonl")).unwrap();
    assert!(source.contains(marker));
    for line in source.lines() {
        let record: Value = serde_json::from_str(line).unwrap();
        assert_eq!(record["format"], "potato-transcript-v1");
        assert_eq!(record["record"]["version"], 1);
    }
    let db = rusqlite::Connection::open(root.path().join("potato.sqlite3")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='messages'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    for table in ["chats", "settings"] {
        let column = if table == "chats" { "spec" } else { "value" };
        let mut statement = db
            .prepare(&format!("SELECT {column} FROM {table}"))
            .unwrap();
        for value in statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
        {
            assert!(!value.unwrap().contains(marker));
        }
    }
    drop(db);
    let exported = export(&runtime).await;
    assert_eq!(exported["chats"][0]["messages"], messages);
    drop(runtime);
    let runtime = Runtime::open(root.path()).unwrap();
    assert_eq!(history(&runtime, id).await, messages);
    assert_eq!(archive(&runtime, id).await, location);
    assert_eq!(export(&runtime).await, exported);
    let copy = tempfile::tempdir().unwrap();
    let imported = Runtime::open(copy.path()).unwrap();
    imported
        .request("POST", "/api/native/import-history", exported)
        .await
        .unwrap();
    assert_eq!(history(&imported, id).await, messages);
}

#[tokio::test]
async fn huge_utf8_tool_output_is_hydrated_from_sidecars_and_deleted_durably() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(root.path()).unwrap();
    let output = "工具输出🥔第一行\n第二行\n".repeat(40_000);
    let messages = json!([
        {"id":"call","type":"function_call","content":[{"type":"data","data":{"name":"execute_shell_command","call_id":"tool-1","arguments":"{}"}}]},
        {"id":"result","type":"function_call_output","content":[{"type":"data","data":{"call_id":"tool-1","output":output}}]}
    ]);
    import(&runtime, "huge-tool", messages.clone()).await;
    assert_eq!(history(&runtime, "huge-tool").await, messages);
    let location = archive(&runtime, "huge-tool").await;
    let dir = Path::new(location["session_dir"].as_str().unwrap());
    let source = std::fs::read_to_string(dir.join("transcript.jsonl")).unwrap();
    assert!(
        source.len() < output.len() / 2,
        "Large output should live in sidecars"
    );
    let mut refs = 0;
    for line in source.lines() {
        let envelope: Value = serde_json::from_str(line).unwrap();
        let mut record = envelope["record"].clone();
        for reference in envelope["text_refs"].as_array().unwrap() {
            let path = Path::new(reference["path"].as_str().unwrap());
            assert!(path.starts_with("artifacts"));
            assert!(!path.is_absolute());
            let content = std::fs::read_to_string(dir.join(path)).unwrap();
            assert_eq!(content, output);
            *record
                .pointer_mut(reference["pointer"].as_str().unwrap())
                .unwrap() = json!(content);
            refs += 1;
        }
    }
    assert!(
        refs >= 2,
        "Both display and wire output must have sidecar references"
    );
    assert_eq!(export(&runtime).await["chats"][0]["messages"], messages);
    drop(runtime);
    let runtime = Runtime::open(root.path()).unwrap();
    assert_eq!(history(&runtime, "huge-tool").await, messages);
    runtime
        .request("DELETE", "/api/chats/huge-tool", Value::Null)
        .await
        .unwrap();
    drop(runtime);
    let runtime = Runtime::open(root.path()).unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/chats", Value::Null)
            .await
            .unwrap(),
        json!([])
    );
    assert_eq!(
        runtime
            .request("GET", "/api/chats/huge-tool", Value::Null)
            .await
            .unwrap_err()
            .status,
        404
    );
}

#[tokio::test]
async fn corrupt_archive_is_isolated_and_can_be_deleted_and_reimported() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(root.path()).unwrap();
    let messages = json!([
        {"id":"user","type":"message","role":"user","content":[{"type":"text","text":"Preserved history"}]}
    ]);
    import(&runtime, "healthy", messages.clone()).await;
    import(&runtime, "broken", messages.clone()).await;
    let location = archive(&runtime, "broken").await;
    std::fs::OpenOptions::new()
        .append(true)
        .open(location["transcript_path"].as_str().unwrap())
        .unwrap()
        .write_all(b"{invalid json}\n")
        .unwrap();
    drop(runtime);

    let runtime = Runtime::open(root.path()).expect("One corrupt session must not block startup");
    assert_eq!(history(&runtime, "healthy").await, messages);
    assert!(runtime
        .request("GET", "/api/chats/broken", Value::Null)
        .await
        .is_err());
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    let broken = chats
        .as_array()
        .unwrap()
        .iter()
        .find(|chat| chat["id"] == "broken")
        .unwrap();
    assert!(broken["history_error"]
        .as_str()
        .is_some_and(|error| !error.is_empty()));
    let health = runtime
        .request("GET", "/api/native/history-health", Value::Null)
        .await
        .unwrap();
    assert!(health["archives"].as_array().unwrap().iter().any(|issue| {
        issue["transcript_path"] == location["transcript_path"]
            && issue["error"]
                .as_str()
                .is_some_and(|error| !error.is_empty())
    }));
    runtime
        .request("DELETE", "/api/chats/broken", Value::Null)
        .await
        .unwrap();
    drop(runtime);

    let runtime = Runtime::open(root.path()).unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/chats/broken", Value::Null)
            .await
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(history(&runtime, "healthy").await, messages);
    let health = runtime
        .request("GET", "/api/native/history-health", Value::Null)
        .await
        .unwrap();
    assert_eq!(health["archives"], json!([]));
    import(&runtime, "broken", messages.clone()).await;
    assert_eq!(history(&runtime, "broken").await, messages);
    drop(runtime);
    let runtime = Runtime::open(root.path()).unwrap();
    assert_eq!(history(&runtime, "broken").await, messages);
}

#[tokio::test]
async fn missing_sidecar_does_not_prevent_startup_or_reading_other_sessions() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(root.path()).unwrap();
    let healthy = json!([
        {"id":"user","type":"message","role":"user","content":[{"type":"text","text":"Healthy session"}]}
    ]);
    import(&runtime, "healthy", healthy.clone()).await;
    import(&runtime, "missing-artifact", json!([
        {"id":"result","type":"function_call_output","content":[{"type":"data","data":{"call_id":"tool","output":"Large tool output\n".repeat(40_000)}}]}
    ])).await;
    let location = archive(&runtime, "missing-artifact").await;
    let session_dir = Path::new(location["session_dir"].as_str().unwrap());
    let source = std::fs::read_to_string(location["transcript_path"].as_str().unwrap()).unwrap();
    let artifact = source
        .lines()
        .find_map(|line| {
            let envelope: Value = serde_json::from_str(line).unwrap();
            envelope["text_refs"]
                .as_array()
                .unwrap()
                .first()
                .map(|reference| session_dir.join(reference["path"].as_str().unwrap()))
        })
        .expect("Large output must produce a sidecar");
    std::fs::remove_file(artifact).unwrap();
    drop(runtime);

    let runtime = Runtime::open(root.path()).expect("Startup metadata must not hydrate sidecars");
    let health = runtime
        .request("GET", "/api/native/history-health", Value::Null)
        .await
        .unwrap();
    assert_eq!(
        health["archives"],
        json!([]),
        "Metadata scanning must not read artifact bodies"
    );
    let chats = runtime
        .request("GET", "/api/chats", Value::Null)
        .await
        .unwrap();
    assert_eq!(chats.as_array().unwrap().len(), 2);
    assert_eq!(history(&runtime, "healthy").await, healthy);
    assert!(runtime
        .request("GET", "/api/chats/missing-artifact", Value::Null)
        .await
        .is_err());
}

#[tokio::test]
async fn derived_index_failure_is_nonfatal_and_clears_after_rebuild() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(root.path()).unwrap();
    let guide = root.path().join("workspace/history/README.md");
    std::fs::remove_file(&guide).unwrap();
    std::fs::create_dir(&guide).unwrap();
    let messages = json!([
        {"id":"user","type":"message","role":"user","content":[{"type":"text","text":"Durable despite index failure"}]}
    ]);
    import(&runtime, "index-failure", messages.clone()).await;
    assert_eq!(history(&runtime, "index-failure").await, messages);
    let health = runtime
        .request("GET", "/api/native/history-health", Value::Null)
        .await
        .unwrap();
    assert!(
        !health["indexes"].is_null(),
        "Failed index publication must be observable"
    );
    assert_eq!(health["archives"], json!([]));
    drop(runtime);

    let runtime = Runtime::open(root.path()).expect("Index obstruction must not block startup");
    assert_eq!(history(&runtime, "index-failure").await, messages);
    let health = runtime
        .request("GET", "/api/native/history-health", Value::Null)
        .await
        .unwrap();
    assert!(!health["indexes"].is_null());
    drop(runtime);
    std::fs::remove_dir(&guide).unwrap();

    let runtime = Runtime::open(root.path()).unwrap();
    assert_eq!(history(&runtime, "index-failure").await, messages);
    let health = runtime
        .request("GET", "/api/native/history-health", Value::Null)
        .await
        .unwrap();
    assert!(
        health["indexes"].is_null(),
        "A successful rebuild must clear the warning"
    );
    assert_eq!(health["archives"], json!([]));
    assert!(guide.is_file());
}
