//! Public model choices for the phone. Never serialize provider credentials or URLs.
use crate::{model::Connection, required, string, Error, Result, Runtime};
use serde_json::{json, Value};

impl Runtime {
    pub(crate) fn remote_model_catalog(&self) -> Result<Value> {
        let active = self.db()?.get("active", Value::Null)?;
        let mut models = vec![];
        let mut selected = Value::Null;
        for provider in self.providers()? {
            if string(&provider, "api_key").is_empty() {
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            for model in ["extra_models", "models"]
                .iter()
                .flat_map(|key| provider[*key].as_array().into_iter().flatten())
            {
                let id = string(model, "id");
                if id.trim().is_empty() || !seen.insert(id) {
                    continue;
                }
                let effort = crate::reasoning::effective_effort(&provider, model);
                models.push(json!({"provider_id":provider["id"],"provider_name":provider["name"].as_str().unwrap_or(string(&provider,"id")),"id":id,"name":model["name"].as_str().unwrap_or(id),"effort_options":crate::reasoning::effort_options(&provider,model),"default_effort":effort}));
                if active["provider_id"] == provider["id"] && active["model"] == id {
                    selected =
                        json!({"provider_id":provider["id"],"model":id,"reasoning_effort":effort});
                }
            }
        }
        Ok(json!({"version":1,"models":models,"active":selected}))
    }

    pub(crate) fn remote_model_connection(&self, choice: &Value) -> Result<Connection> {
        let provider_id = required(choice, "provider_id")?;
        let model = required(choice, "model")?;
        // Null is an explicit service default, not an instruction to reload the desktop preference.
        let effort = choice
            .get("reasoning_effort")
            .ok_or_else(|| Error::new(422, "请重新选择模型与思考设置"))?;
        if !effort.is_null() && !effort.is_string() {
            return Err(Error::new(422, "思考设置无效"));
        }
        let catalog = self.remote_model_catalog()?;
        let entry = catalog["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["provider_id"] == provider_id && m["id"] == model)
            .ok_or_else(|| Error::new(422, "所选模型已不可用，请重新选择"))?;
        if !(effort.is_null()
            || entry["effort_options"].as_array().unwrap().contains(effort)
            || entry["default_effort"] == *effort)
        {
            return Err(Error::new(422, "该模型的思考设置已改变，请重新选择"));
        }
        let mut connection = self.provider_connection(provider_id, model)?;
        connection.options["reasoning_effort"] = effort.clone();
        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock;
    use std::sync::Arc;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn configured(core: &Arc<Runtime>, url: &str, responses: bool) {
        core.request("PUT", "/api/models/sub2api/config", json!({"api_key":"synthetic-secret","base_url":url,"chat_model":if responses {"OpenAIResponseModel"} else {"OpenAIChatModel"}})).await.unwrap();
        for id in ["one", "two"] {
            core.request(
                "POST",
                "/api/models/sub2api/models",
                json!({"id":id,"name":format!("Model {id}")}),
            )
            .await
            .unwrap();
        }
        core.request(
            "PUT",
            "/api/models/sub2api/models/one/config",
            json!({"reasoning_effort_options":["low","high"],"reasoning_effort":"high"}),
        )
        .await
        .unwrap();
        core.request(
            "PUT",
            "/api/models/active",
            json!({"provider_id":"sub2api","model":"two"}),
        )
        .await
        .unwrap();
    }
    fn choice(effort: Value) -> Value {
        json!({"provider_id":"sub2api","model":"one","reasoning_effort":effort})
    }
    fn command(args: Value) -> Value {
        json!({"id":uuid::Uuid::new_v4().to_string(),"op":"send","args":args})
    }

    #[tokio::test]
    async fn remote_model_catalog_is_redacted_and_uses_declared_capabilities() {
        let tmp = tempfile::tempdir().unwrap();
        let core = Runtime::open(tmp.path()).unwrap();
        configured(&core, "https://private-fixture.invalid/v1", false).await;
        let catalog = core.remote_model_catalog().unwrap();
        let wire = catalog.to_string();
        assert!(
            !wire.contains("synthetic-secret")
                && !wire.contains("private-fixture")
                && !wire.contains("api_key")
                && !wire.contains("base_url")
        );
        assert_eq!(catalog["active"]["model"], "two");
        let one = catalog["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == "one")
            .unwrap();
        assert_eq!(one["effort_options"], json!(["low", "high"]));
        let two = catalog["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == "two")
            .unwrap();
        assert_eq!(two["effort_options"], json!([]));
        assert!(core.remote_model_connection(&choice(json!("max"))).is_err());
        assert!(core
            .remote_model_connection(
                &json!({"provider_id":"sub2api","model":"missing","reasoning_effort":null})
            )
            .is_err());
        assert!(core
            .remote_model_connection(&json!({"provider_id":"sub2api","model":"one"}))
            .is_err());
        assert_eq!(
            core.remote_model_connection(&choice(Value::Null))
                .unwrap()
                .options["reasoning_effort"],
            Value::Null
        );
        assert_eq!(
            core.provider_connection("sub2api", "one").unwrap().options["reasoning_effort"],
            "high"
        );
        let overview = core
            .remote_command(&json!({"op":"overview","args":{}}))
            .await
            .unwrap();
        assert_eq!(overview["model_catalog"], catalog);
    }

    #[tokio::test]
    async fn remote_model_request_reaches_chat_and_responses_wire_without_changing_desktop() {
        for (responses, effort, queued) in [(false, Some("low"), false), (true, Some("high"), false), (false, None, false), (false, Some("low"), true)] {
            let tmp = tempfile::tempdir().unwrap();
            let core = Runtime::open(tmp.path()).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            configured(
                &core,
                &format!("http://{}/v1", listener.local_addr().unwrap()),
                responses,
            )
            .await;
            let captured = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut data = Vec::new();
                let mut chunk = [0; 8192];
                let (header, body) = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&chunk[..n]);
                    if let Some(end) = data.windows(4).position(|v| v == b"\r\n\r\n") {
                        let h = String::from_utf8_lossy(&data[..end]).to_string();
                        let len = h
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap();
                        if data.len() >= end + 4 + len {
                            break (
                                h,
                                serde_json::from_slice::<Value>(&data[end + 4..end + 4 + len])
                                    .unwrap(),
                            );
                        }
                    }
                };
                // The fixture verifies the actual outbound request, without invoking a model.
                socket.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await.unwrap();
                (header, body)
            });
            let before = core.db().unwrap().get("active", Value::Null).unwrap();
            let mut request = command(
                json!({"text":"synthetic wire verification","model_choice":choice(json!(effort))}),
            );
            if queued {
                let chat = core.db().unwrap().ensure_chat("queued-wire-session", "Queue wire test").unwrap();
                request["args"]["chat_id"] = chat["id"].clone();
                request["args"]["delivery_mode"] = json!("queue");
            }
            let receipt = core.remote_command(&request).await.unwrap();
            if queued {
                assert_eq!(receipt["delivery"], "queued");
                core.tick_outbox().unwrap();
            }
            let (header, body) = tokio::time::timeout(std::time::Duration::from_secs(15), captured)
                .await
                .unwrap()
                .unwrap();
            assert!(header.contains(if responses {
                "/v1/responses "
            } else {
                "/v1/chat/completions "
            }));
            assert_eq!(body["model"], "one");
            assert_eq!(
                if responses {
                    &body["reasoning"]["effort"]
                } else {
                    &body["reasoning_effort"]
                },
                &json!(effort)
            );
            if effort.is_none() {
                assert!(body.get("reasoning_effort").is_none());
            }
            assert_eq!(
                core.db().unwrap().get("active", Value::Null).unwrap(),
                before
            );
            assert_eq!(
                core.provider_connection("sub2api", "one").unwrap().options["reasoning_effort"],
                "high"
            );
            assert_eq!(core.remote_command(&request).await.unwrap(), receipt);
            let key = format!("remote_receipt:{}", request["id"].as_str().unwrap());
            let mut reserved = core.db().unwrap().get(&key,Value::Null).unwrap();
            reserved.as_object_mut().unwrap().remove("result");
            core.db().unwrap().put(&key,&reserved).unwrap();
            let recovered = core.remote_command(&request).await.unwrap();
            assert_eq!(recovered["delivery"],"recovered");
            assert_eq!(recovered["chat"]["id"],receipt["chat"]["id"]);
            let history = core.db().unwrap().history(receipt["chat"]["id"].as_str().unwrap(),false).unwrap();
            let inputs: Vec<_> = history.iter().filter(|frame|frame["role"] == "user").collect();
            assert_eq!(inputs.len(),1);
            assert_eq!(inputs[0]["metadata"]["remote_operation_id"],request["id"]);
            assert!(body.get("remote_operation_id").is_none());
            let mut altered = request;
            altered["args"]["model_choice"]["reasoning_effort"] = json!("different");
            assert_eq!(core.remote_command(&altered).await.unwrap_err().status, 409);
            let session = receipt["chat"]["session_id"].as_str().unwrap();
            for _ in 0..100 {
                if !lock(&core.runs).unwrap().contains_key(session) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            assert!(!lock(&core.runs).unwrap().contains_key(session));
        }
    }

    #[tokio::test]
    async fn remote_model_invalid_choice_does_not_create_chat_or_change_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let core = Runtime::open(tmp.path()).unwrap();
        configured(&core, "https://fixture.invalid/v1", false).await;
        let before = core.db().unwrap().chats().unwrap();
        let request = command(json!({"text":"invalid","model_choice":choice(json!("invented"))}));
        assert_eq!(core.remote_command(&request).await.unwrap_err().status, 422);
        assert_eq!(core.remote_command(&request).await.unwrap_err().status, 422);
        assert_eq!(core.db().unwrap().chats().unwrap(), before);
    }
}
