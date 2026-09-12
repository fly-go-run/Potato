use potato_core::Runtime;
use serde_json::{Value, json};

#[tokio::test]
async fn model_effort_validates_persists_and_stays_model_scoped() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(tmp.path()).unwrap();
    for id in ["one", "two"] {
        runtime
            .request("POST", "/api/models/sub2api/models", json!({"id":id}))
            .await
            .unwrap();
    }
    let one = "/api/models/sub2api/models/one/config";
    let two = "/api/models/sub2api/models/two/config";
    runtime
        .request(
            "PUT",
            one,
            json!({"reasoning_effort_options":["low","high"],"reasoning_effort":"high"}),
        )
        .await
        .unwrap();
    runtime
        .request(
            "PUT",
            two,
            json!({"reasoning_effort_options":["medium","max"],"reasoning_effort":"max"}),
        )
        .await
        .unwrap();
    assert!(
        runtime
            .request("PUT", two, json!({"reasoning_effort":"high"}))
            .await
            .is_err()
    );
    assert!(
        runtime
            .request("PUT", two, json!({"reasoning_effort_options":[4]}))
            .await
            .is_err()
    );
    for id in ["one", "two", "one"] {
        runtime
            .request(
                "PUT",
                "/api/models/active",
                json!({"provider_id":"sub2api","model":id}),
            )
            .await
            .unwrap();
    }
    drop(runtime);
    let runtime = Runtime::open(tmp.path()).unwrap();
    let providers = runtime
        .request("GET", "/api/models", Value::Null)
        .await
        .unwrap();
    let p = providers
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "sub2api")
        .unwrap();
    let models = p["extra_models"].as_array().unwrap();
    assert_eq!(models[0]["reasoning_effort"], "high");
    assert_eq!(models[1]["reasoning_effort"], "max");
    let updated = runtime
        .request("PUT", one, json!({"reasoning_effort_options":["low"]}))
        .await
        .unwrap();
    assert!(updated["extra_models"][0]["reasoning_effort"].is_null());
    runtime
        .request("PUT", two, json!({"reasoning_effort":null}))
        .await
        .unwrap();
}
