//! Remote MCP clients are connected only on explicit discovery/tool invocation.
//! Cached schemas keep startup and ordinary chat independent of MCP availability.
use crate::{required, string, Error, Result, Runtime};
use rmcp::{
    model::{CallToolRequestParams, ClientInfo},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    ServiceExt,
};
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};

fn public(mut config: Value) -> Value {
    if let Some(env) = config["env"].as_object_mut() {
        for value in env.values_mut() {
            *value = json!("********");
        }
    }
    if let Some(headers) = config["headers"].as_object_mut() {
        for value in headers.values_mut() {
            *value = json!("********");
        }
    }
    config.as_object_mut().unwrap().remove("catalog");
    config["access_summary"] = json!({"default_effect":"ask","overrides_count":0});
    config
}

fn tool_enabled(config: &Value, name: &str) -> bool {
    config["enabled"] == true
        && config["tools"]
            .as_array()
            .is_none_or(|tools| tools.iter().any(|t| t == name))
}

impl Runtime {
    pub(crate) fn mcp_definitions(&self) -> Result<Vec<Value>> {
        let clients = self.db()?.get("mcp_clients", json!({}))?;
        let mut definitions = Vec::new();
        for config in clients.as_object().unwrap().values() {
            for tool in config["catalog"].as_array().into_iter().flatten() {
                if tool_enabled(config, string(tool, "name")) {
                    definitions.push(json!({"type":"function","function":{"name":tool["native_name"],"description":tool["description"],"parameters":tool["inputSchema"]}}));
                }
            }
        }
        Ok(definitions)
    }

    pub(crate) fn mcp_target(&self, name: &str) -> Result<(Value, String)> {
        let clients = self.db()?.get("mcp_clients", json!({}))?;
        for config in clients.as_object().unwrap().values() {
            for tool in config["catalog"].as_array().into_iter().flatten() {
                if tool["native_name"] == name && tool_enabled(config, string(tool, "name")) {
                    return Ok((config.clone(), required(tool, "name")?.to_owned()));
                }
            }
        }
        Err(Error::new(
            404,
            "MCP tool is disabled or its definition changed",
        ))
    }

    pub(crate) async fn mcp_call(
        &self,
        config: &Value,
        call: Option<(&str, &Value)>,
    ) -> Result<Value> {
        let client = if config["transport"] == "stdio" {
            use process_wrap_mcp::tokio::*;
            let mut command = CommandWrap::with_new(required(config, "command")?, |_| {});
            command.command_mut().args(
                config["args"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            );
            if let Some(cwd) = config["cwd"].as_str().filter(|s| !s.is_empty()) {
                command.command_mut().current_dir(cwd);
            }
            if let Some(env) = config["env"].as_object() {
                let db = self.db()?;
                for (key, value) in env {
                    command
                        .command_mut()
                        .env(key, db.unseal(value.as_str().unwrap_or(""))?);
                }
            }
            command.wrap(KillOnDrop);
            #[cfg(windows)]
            command.command_mut().creation_flags(0x08000000); // CREATE_NO_WINDOW
            #[cfg(unix)]
            command.wrap(ProcessGroup::leader());
            #[cfg(windows)]
            command.wrap(JobObject);
            let (transport, _) = rmcp::transport::TokioChildProcess::builder(command)
                .stderr(std::process::Stdio::null())
                .spawn()?;
            tokio::time::timeout(
                Duration::from_secs(15),
                ClientInfo::default().serve(transport),
            )
            .await
            .map_err(|_| Error::new(504, "Local MCP initialization timed out"))?
            .map_err(|_| Error::new(502, "Local MCP initialization failed"))?
        } else {
            crate::api::validate_url(required(config, "url")?)?;
            let mut headers = HashMap::new();
            if let Some(saved) = config["headers"].as_object() {
                let db = self.db()?;
                for (name, value) in saved {
                    let header = reqwest_mcp::header::HeaderName::from_bytes(name.as_bytes())
                        .map_err(|_| Error::new(400, "Invalid MCP header"))?;
                    let value = db.unseal(
                        value
                            .as_str()
                            .ok_or_else(|| Error::new(500, "Invalid saved MCP credential"))?,
                    )?;
                    headers.insert(
                        header,
                        reqwest_mcp::header::HeaderValue::from_str(&value)
                            .map_err(|_| Error::new(400, "Invalid MCP header value"))?,
                    );
                }
            }
            let http = reqwest_mcp::Client::builder()
                .redirect(reqwest_mcp::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .build()
                .map_err(|_| Error::new(500, "MCP HTTP client unavailable"))?;
            let config =
                StreamableHttpClientTransportConfig::with_uri(required(config, "url")?.to_owned())
                    .custom_headers(headers)
                    .max_sse_event_size(2_000_000)
                    .reinit_on_expired_session(false);
            let transport = StreamableHttpClientTransport::with_client(http, config);
            let client = tokio::time::timeout(
                Duration::from_secs(15),
                ClientInfo::default().serve(transport),
            )
            .await
            .map_err(|_| Error::new(504, "MCP initialization timed out"))?
            .map_err(|_| Error::new(502, "MCP initialization failed"))?;
            client
        };
        let result = tokio::time::timeout(Duration::from_secs(90), async {
            if let Some((name, args)) = call {
                let arguments = args
                    .as_object()
                    .ok_or_else(|| Error::new(400, "MCP arguments must be an object"))?
                    .clone();
                let result = client
                    .call_tool(
                        CallToolRequestParams::new(name.to_owned()).with_arguments(arguments),
                    )
                    .await
                    .map_err(|_| Error::new(502, "MCP tool call failed"))?;
                serde_json::to_value(result).map_err(Into::into)
            } else {
                let tools = client
                    .list_all_tools()
                    .await
                    .map_err(|_| Error::new(502, "MCP tool discovery failed"))?;
                if tools.len() > 128 {
                    return Err(Error::new(413, "MCP server exposes more than 128 tools"));
                }
                serde_json::to_value(tools).map_err(Into::into)
            }
        })
        .await
        .unwrap_or_else(|_| Err(Error::new(504, "MCP operation timed out")));
        let _ = tokio::time::timeout(Duration::from_secs(3), client.cancel()).await;
        let result = result?;
        if result.to_string().len() > 2_000_000 {
            return Err(Error::new(413, "MCP result exceeds 2 MB"));
        }
        Ok(result)
    }

    pub(crate) async fn mcp_request(
        &self,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<Option<Value>> {
        if path != "/api/mcp" && !path.starts_with("/api/mcp/") {
            return Ok(None);
        }
        if method == "GET" && path.starts_with("/api/mcp/tools/") {
            let key = &path["/api/mcp/tools/".len()..];
            let clients = self.db()?.get("mcp_clients", json!({}))?;
            let config = clients
                .get(key)
                .ok_or_else(|| Error::new(404, "MCP client not found"))?
                .clone();
            if config["enabled"] != true {
                return Err(Error::new(409, "MCP client is disabled"));
            }
            let mut catalog = self.mcp_call(&config, None).await?;
            for tool in catalog.as_array_mut().unwrap() {
                required(tool, "name")?;
                if !tool["inputSchema"].is_object() {
                    return Err(Error::new(502, "Invalid MCP tool schema"));
                }
                tool["native_name"] = json!(format!("mcp_{}", uuid::Uuid::new_v4().simple()));
                if !tool["description"].is_string() {
                    tool["description"] = json!("");
                }
            }
            let db = self.db()?;
            let mut clients = db.get("mcp_clients", json!({}))?;
            let current = clients
                .get_mut(key)
                .ok_or_else(|| Error::new(409, "MCP client was deleted"))?;
            if *current != config {
                return Err(Error::new(409, "MCP settings changed during discovery"));
            }
            current["catalog"] = catalog.clone();
            db.put("mcp_clients", &clients)?;
            return Ok(Some(catalog));
        }
        let db = self.db()?;
        let mut clients = db.get("mcp_clients", json!({}))?;
        if path == "/api/mcp" && method == "GET" {
            return Ok(Some(json!(clients
                .as_object()
                .unwrap()
                .values()
                .cloned()
                .map(public)
                .collect::<Vec<_>>())));
        }
        let key = if path == "/api/mcp" {
            required(body, "client_key")?
        } else {
            path.strip_prefix("/api/mcp/toggle/")
                .or_else(|| path.strip_prefix("/api/mcp/tools/"))
                .unwrap_or(&path["/api/mcp/".len()..])
        };
        if key.is_empty()
            || key.len() > 64
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(Error::new(400, "Invalid MCP client key"));
        }
        let exists = clients.get(key).is_some();
        if method == "POST" && exists {
            return Err(Error::new(409, "MCP client already exists"));
        }
        if method != "POST" && !exists {
            return Err(Error::new(404, "MCP client not found"));
        }
        let result = match method {
            "GET" => public(clients[key].clone()),
            "DELETE" => {
                clients.as_object_mut().unwrap().remove(key);
                json!({"message":"MCP client deleted"})
            }
            "PATCH" if path.starts_with("/api/mcp/toggle/") => {
                clients[key]["enabled"] = json!(clients[key]["enabled"] != true);
                public(clients[key].clone())
            }
            "PUT" if path.starts_with("/api/mcp/tools/") => {
                if !body["tools"].is_null()
                    && !body["tools"]
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string))
                {
                    return Err(Error::new(
                        400,
                        "Tool whitelist must be a string list or null",
                    ));
                }
                clients[key]["tools"] = body["tools"].clone();
                clients[key]["catalog"].clone()
            }
            "POST" | "PUT" => {
                let update = if method == "POST" {
                    &body["client"]
                } else {
                    body
                };
                let mut config = if exists {
                    clients[key].clone()
                } else {
                    json!({"key":key,"name":key,"description":"","enabled":true,"transport":"streamable_http","headers":{},"catalog":[],"tools":null,"command":"","args":[],"env":{},"cwd":""})
                };
                for field in [
                    "name",
                    "description",
                    "url",
                    "transport",
                    "enabled",
                    "command",
                    "args",
                    "cwd",
                ] {
                    if let Some(value) = update.get(field) {
                        config[field] = value.clone();
                    }
                }
                if config["transport"] != "streamable_http" && config["transport"] != "stdio" {
                    return Err(Error::new(501, "This MCP transport is not migrated yet"));
                }
                if config["transport"] == "stdio" {
                    required(&config, "command")?;
                    if !config["args"].as_array().is_some_and(|args| {
                        args.len() <= 128
                            && args
                                .iter()
                                .all(|v| v.as_str().is_some_and(|s| s.len() <= 16000))
                    }) {
                        return Err(Error::new(
                            400,
                            "MCP arguments must be a bounded list of strings",
                        ));
                    }
                    if !string(&config, "cwd").is_empty()
                        && !std::path::Path::new(string(&config, "cwd")).is_absolute()
                    {
                        return Err(Error::new(400, "MCP working directory must be absolute"));
                    }
                } else {
                    crate::api::validate_url(required(&config, "url")?)?;
                }
                if let Some(env) = update["env"].as_object() {
                    for (key, value) in env {
                        if key.is_empty() || key.len() > 128 || key.contains(['=', '\0']) {
                            return Err(Error::new(400, "Invalid MCP environment key"));
                        }
                        let value = value.as_str().ok_or_else(|| {
                            Error::new(400, "MCP environment values must be text")
                        })?;
                        if value.contains('\0') || value.len() > 16000 {
                            return Err(Error::new(400, "Invalid MCP environment value"));
                        }
                        if value != "********" {
                            config["env"][key] = json!(db.seal(value)?);
                        }
                    }
                }
                required(&config, "name")?;
                if !config["enabled"].is_boolean() {
                    return Err(Error::new(400, "enabled must be boolean"));
                }
                if let Some(headers) = update["headers"].as_object() {
                    for (name, value) in headers {
                        let header = reqwest_mcp::header::HeaderName::from_bytes(name.as_bytes())
                            .map_err(|_| Error::new(400, "Invalid MCP header"))?;
                        if matches!(
                            header.as_str(),
                            "host" | "content-length" | "content-type" | "accept"
                        ) || header.as_str().starts_with("mcp-")
                        {
                            return Err(Error::new(400, "Reserved MCP header"));
                        }
                        let value = value
                            .as_str()
                            .ok_or_else(|| Error::new(400, "MCP header must be text"))?;
                        if value != "********" {
                            reqwest_mcp::header::HeaderValue::from_str(value)
                                .map_err(|_| Error::new(400, "Invalid MCP header value"))?;
                            config["headers"][header.as_str()] = json!(db.seal(value)?);
                        }
                    }
                }
                if exists
                    && (config["url"] != clients[key]["url"]
                        || config["headers"] != clients[key]["headers"]
                        || ["transport", "command", "args", "cwd", "env"]
                            .iter()
                            .any(|field| config[*field] != clients[key][*field]))
                {
                    config["catalog"] = json!([]);
                }
                clients[key] = config.clone();
                public(config)
            }
            _ => return Err(Error::new(405, "Method not allowed")),
        };
        if method != "GET" {
            db.put("mcp_clients", &clients)?;
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use tokio_util::sync::CancellationToken;

    #[cfg(unix)]
    #[tokio::test]
    async fn stdio_discovery_is_lazy_and_environment_is_encrypted() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("mcp-fixture");
        std::fs::write(&binary,r#"#!/bin/sh
[ "$TEST_MCP_KEY" = "synthetic-key" ] || exit 1
printf 'started\n' >> "$(dirname "$0")/started"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      protocol=$(printf '%s' "$line" | sed -n 's/.*"protocolVersion":"\([^"]*\)".*/\1/p')
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"%s","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}}}\n' "$id" "$protocol";;
    *'"method":"tools/list"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo","description":"Local echo","inputSchema":{"type":"object","properties":{}}}]}}\n' "$id";;
    *'"method":"tools/call"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"stdio result"}],"isError":false}}\n' "$id";;
  esac
done
"#).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = Runtime::open(&tmp.path().join("data")).unwrap();
        let public=runtime.request("POST","/api/mcp",json!({"client_key":"local","client":{"name":"Local fixture","transport":"stdio","command":binary,"args":[],"env":{"TEST_MCP_KEY":"synthetic-key"}}})).await.unwrap();
        assert_eq!(public["env"]["TEST_MCP_KEY"], "********");
        assert!(!tmp.path().join("started").exists());
        let tools = runtime
            .request("GET", "/api/mcp/tools/local", Value::Null)
            .await
            .unwrap();
        assert_eq!(tools[0]["name"], "echo");
        let config = runtime
            .db()
            .unwrap()
            .get("mcp_clients", Value::Null)
            .unwrap()["local"]
            .clone();
        assert!(!config.to_string().contains("synthetic-key"));
        let result = runtime
            .mcp_call(&config, Some(("echo", &json!({}))))
            .await
            .unwrap();
        assert_eq!(result["content"][0]["text"], "stdio result");
    }

    #[tokio::test]
    async fn remote_mcp_is_lazy_masked_and_callable_only_after_approval() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (seen, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut raw = Vec::new();
                    let mut chunk = [0; 4096];
                    let (header_end, length) = loop {
                        let size = socket.read(&mut chunk).await.unwrap();
                        if size == 0 {
                            return;
                        }
                        raw.extend_from_slice(&chunk[..size]);
                        if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&raw[..end]).to_lowercase();
                            let length = header
                                .lines()
                                .find_map(|l| {
                                    l.strip_prefix("content-length: ")
                                        .and_then(|v| v.parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if raw.len() >= end + 4 + length {
                                break (end + 4, length);
                            }
                        }
                    };
                    let body: Value = serde_json::from_slice(&raw[header_end..header_end + length])
                        .unwrap_or(Value::Null);
                    let headers = String::from_utf8_lossy(&raw[..header_end]).to_lowercase();
                    let method = string(&body, "method");
                    if !body.is_null() {
                        assert!(headers.contains("authorization: bearer test-mcp-key"));
                    }
                    let _ = seen.send(method.to_owned());
                    let result = match method {
                        "initialize" => Some(
                            json!({"protocolVersion":body["params"]["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}}),
                        ),
                        "tools/list" => Some(
                            json!({"tools":[{"name":"echo","description":"Echo a message","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}]}),
                        ),
                        "tools/call" => Some(
                            json!({"content":[{"type":"text","text":body["params"]["arguments"]["text"]}],"isError":false}),
                        ),
                        _ => None,
                    };
                    let (status, body) = if let Some(result) = result {
                        (
                            "200 OK",
                            json!({"jsonrpc":"2.0","id":body["id"],"result":result}).to_string(),
                        )
                    } else if headers.starts_with("get ") {
                        ("405 Method Not Allowed", String::new())
                    } else {
                        ("202 Accepted", String::new())
                    };
                    let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        let config=runtime.request("POST","/api/mcp",json!({"client_key":"test","client":{"name":"Test server","transport":"streamable_http","url":format!("http://{address}/mcp"),"headers":{"Authorization":"Bearer test-mcp-key"}}})).await.unwrap();
        assert_eq!(config["headers"]["authorization"], "********");
        assert!(requests.try_recv().is_err());
        assert!(runtime.mcp_definitions().unwrap().is_empty());
        let tools = runtime
            .request("GET", "/api/mcp/tools/test", Value::Null)
            .await
            .unwrap();
        assert_eq!(tools[0]["name"], "echo");
        let name = tools[0]["native_name"].as_str().unwrap().to_owned();
        assert_eq!(runtime.mcp_definitions().unwrap().len(), 1);
        while requests.try_recv().is_ok() {}
        let task_runtime = runtime.clone();
        let task = tokio::spawn(async move {
            let emit: crate::Emit = Arc::new(|_| Ok(()));
            task_runtime
                .execute_tool(
                    "family",
                    &name,
                    &json!({"text":"hello MCP"}),
                    &Value::Null,
                    &CancellationToken::new(),
                    &emit,
                )
                .await
        });
        let pending = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let pending = runtime
                    .request(
                        "GET",
                        "/api/console/push-messages?session_id=family",
                        Value::Null,
                    )
                    .await
                    .unwrap();
                if let Some(item) = pending["pending_approvals"].as_array().unwrap().first() {
                    break item.clone();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(requests.try_recv().is_err());
        runtime.request("POST","/api/approval/approve",json!({"request_id":pending["request_id"],"session_id":"family","user_id":"default"})).await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(result.contains("hello MCP"));
        runtime
            .request("PATCH", "/api/mcp/toggle/test", Value::Null)
            .await
            .unwrap();
        assert!(runtime.mcp_definitions().unwrap().is_empty());
        server.abort();
    }
}
