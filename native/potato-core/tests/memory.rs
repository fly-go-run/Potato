use potato_core::Runtime;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

async fn location(runtime: &Runtime) -> PathBuf {
    let value = runtime
        .request("GET", "/api/workspace/memory-location", Value::Null)
        .await
        .unwrap();
    PathBuf::from(value["path"].as_str().unwrap())
}

fn seed_legacy(root: &Path, documents: Value, reset_marker: bool) {
    let db = rusqlite::Connection::open(root.join("potato.sqlite3")).unwrap();
    db.execute("INSERT INTO settings(key,value) VALUES ('memory_documents',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [documents.to_string()]).unwrap();
    if reset_marker {
        db.execute(
            "DELETE FROM settings WHERE key='memory_files_migrated_v1'",
            [],
        )
        .unwrap();
    }
}

#[tokio::test]
async fn files_are_authoritative_and_deleted_notes_do_not_resurrect() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let root = location(&runtime).await;
    let endpoint = "/api/workspace/memory/topics/preferences.md";
    runtime
        .request("PUT", endpoint, json!({"content":"偏好：清茶 🍵\n"}))
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("topics/preferences.md")).unwrap(),
        "偏好：清茶 🍵\n"
    );
    fs::write(root.join("topics/preferences.md"), "外部修改\n").unwrap();
    fs::write(root.join("external.md"), "written outside the API").unwrap();
    assert_eq!(
        runtime.request("GET", endpoint, Value::Null).await.unwrap()["content"],
        "外部修改\n"
    );
    let list = runtime
        .request("GET", "/api/workspace/memory", Value::Null)
        .await
        .unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["path"] == "external.md"));
    let note = list
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "topics/preferences.md")
        .unwrap();
    assert_eq!(note["size"], "外部修改\n".len());
    runtime
        .request("DELETE", endpoint, Value::Null)
        .await
        .unwrap();
    assert!(!root.join("topics/preferences.md").exists());
    drop(runtime);
    // A stale backup remains in SQLite, but the completed migration marker wins.
    seed_legacy(
        tmp.path(),
        json!({"topics/preferences.md":{"content":"old backup"}}),
        false,
    );
    let reopened = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        reopened
            .request("GET", endpoint, Value::Null)
            .await
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        reopened
            .request("GET", "/api/workspace/memory/external.md", Value::Null)
            .await
            .unwrap()["content"],
        "written outside the API"
    );
}

#[tokio::test]
async fn legacy_migration_preserves_files_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let root = location(&runtime).await;
    fs::create_dir_all(root.join("topics")).unwrap();
    fs::write(root.join("topics/conflict.md"), "external version").unwrap();
    drop(runtime);
    let legacy = json!({"topics/new.md":{"content":"迁移内容\n"},"topics/conflict.md":{"content":"legacy version"}});
    seed_legacy(tmp.path(), legacy.clone(), true);
    let runtime = Runtime::open(tmp.path()).unwrap();
    let db = rusqlite::Connection::open(tmp.path().join("potato.sqlite3")).unwrap();
    let retired: String = db
        .query_row(
            "SELECT value FROM settings WHERE key='memory_documents'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&retired).unwrap(),
        Value::Null
    );
    drop(db);
    assert_eq!(
        fs::read_to_string(root.join("topics/new.md")).unwrap(),
        "迁移内容\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("topics/conflict.md")).unwrap(),
        "external version"
    );
    assert_eq!(
        fs::read_to_string(root.join("legacy-import/topics/conflict.md")).unwrap(),
        "legacy version"
    );
    let before = runtime
        .request("GET", "/api/workspace/memory", Value::Null)
        .await
        .unwrap();
    drop(runtime);
    seed_legacy(tmp.path(), legacy, true);
    let runtime = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/memory", Value::Null)
            .await
            .unwrap(),
        before
    );
    runtime
        .request("DELETE", "/api/workspace/memory/topics/new.md", Value::Null)
        .await
        .unwrap();
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    assert_eq!(
        runtime
            .request("GET", "/api/workspace/memory/topics/new.md", Value::Null)
            .await
            .unwrap_err()
            .status,
        404
    );
}

#[tokio::test]
async fn stale_expected_content_cannot_overwrite_or_delete_external_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    let root = location(&runtime).await;
    let endpoint = "/api/workspace/memory/MEMORY.md";
    runtime
        .request(
            "PUT",
            endpoint,
            json!({"content":"original", "expected_content":null}),
        )
        .await
        .unwrap();
    fs::write(root.join("MEMORY.md"), "external change").unwrap();
    for method in ["PUT", "DELETE"] {
        let error = runtime
            .request(
                method,
                endpoint,
                json!({"content":"stale replacement", "expected_content":"original"}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.status, 409, "{method}: {error}");
        assert_eq!(
            fs::read_to_string(root.join("MEMORY.md")).unwrap(),
            "external change"
        );
    }
    runtime
        .request(
            "PUT",
            endpoint,
            json!({"content":"accepted", "expected_content":"external change"}),
        )
        .await
        .unwrap();
    runtime
        .request("DELETE", endpoint, json!({"expected_content":"accepted"}))
        .await
        .unwrap();
    assert!(!root.join("MEMORY.md").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_note_parent_and_root_cannot_access_outside_files() {
    use std::os::unix::fs::symlink;
    for redirect in ["note", "parent", "root"] {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        let root = location(&runtime).await;
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.md");
        fs::write(&secret, "outside secret").unwrap();
        let name = match redirect {
            "note" => {
                symlink(&secret, root.join("secret.md")).unwrap();
                "secret.md"
            }
            "parent" => {
                symlink(outside.path(), root.join("nested")).unwrap();
                "nested/secret.md"
            }
            "root" => {
                fs::remove_dir(&root).unwrap();
                symlink(outside.path(), &root).unwrap();
                "secret.md"
            }
            _ => unreachable!(),
        };
        let endpoint = format!("/api/workspace/memory/{name}");
        assert!(
            runtime
                .request("GET", &endpoint, Value::Null)
                .await
                .is_err(),
            "{redirect} read escaped"
        );
        assert!(
            runtime
                .request("PUT", &endpoint, json!({"content":"overwritten"}))
                .await
                .is_err(),
            "{redirect} write escaped"
        );
        let list = runtime
            .request("GET", "/api/workspace/memory", Value::Null)
            .await;
        if let Ok(list) = list {
            assert!(
                list.as_array().unwrap().is_empty(),
                "{redirect} listing exposed outside files"
            );
        }
        // Removing a note symlink is safe; removing its target must never happen.
        let _ = runtime.request("DELETE", &endpoint, Value::Null).await;
        assert_eq!(
            fs::read_to_string(&secret).unwrap(),
            "outside secret",
            "{redirect} modified target"
        );
    }
}
