//! Lazy native Cua driver adapter. Observations are single-use capabilities,
//! bound to a chat and driver snapshot; input never upgrades to foreground.
use crate::{lock, required, string, Error, Result, Runtime};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

#[derive(Clone)]
pub(crate) struct Observation {
    chat: String,
    expires: Instant,
    app: String,
    bundle: String,
    payload: Value,
    elements: Vec<Value>,
}

pub(crate) struct Computer {
    binary: PathBuf,
    socket: String,
    host: String,
    daemon: Option<Child>,
    permissions: Value,
}

fn permission_probe() -> Value {
    if cfg!(target_os = "windows") {
        json!({})
    } else {
        json!({"prompt":false,"probe_direct_capture":false})
    }
}

fn records(value: &Value) -> Vec<Value> {
    if let Some(items) = value.as_array() {
        return items.clone();
    }
    for key in ["apps", "windows", "items", "results"] {
        if let Some(items) = value[key].as_array() {
            return items.clone();
        }
    }
    Vec::new()
}
fn field<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|k| value[*k].as_str().filter(|s| !s.is_empty()))
        .unwrap_or("")
}
fn protected(app: &str, bundle: &str, pid: u64) -> bool {
    let app = app.to_lowercase();
    let bundle = bundle.to_lowercase();
    pid == std::process::id() as u64
        || ["potato", "qwenpaw"]
            .iter()
            .any(|s| app.contains(s) || bundle.contains(s))
        || [
            "terminal",
            "iterm",
            "iterm2",
            "windows terminal",
            "windowsterminal",
            "alacritty",
            "wezterm",
            "system settings",
            "system preferences",
        ]
        .contains(&app.as_str())
        || [
            "com.apple.terminal",
            "com.googlecode.iterm2",
            "com.github.wez.wezterm",
            "org.alacritty",
            "com.apple.systempreferences",
            "com.apple.preferences",
            "com.microsoft.windows.terminal",
            "microsoft.windowsterminal_8wekyb3d8bbwe",
        ]
        .contains(&bundle.as_str())
}

impl Computer {
    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.binary);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW for the background driver.
        cmd.env("CUA_DRIVER_RS_TELEMETRY_ENABLED", "0")
            .env("CUA_DRIVER_EMBEDDED", "1")
            .env("CUA_DRIVER_HOST_BUNDLE_ID", &self.host)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        cmd
    }
    async fn cli(&self, args: &[String]) -> Result<Value> {
        let mut child = self
            .command()
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdout = child.stdout.take().unwrap();
        let operation = async {
            let mut bytes = Vec::new();
            (&mut stdout)
                .take(2_000_001)
                .read_to_end(&mut bytes)
                .await?;
            if bytes.len() > 2_000_000 {
                return Err(Error::new(413, "Computer driver response is too large"));
            }
            if !child.wait().await?.success() {
                return Err(Error::new(
                    502,
                    "Computer driver command failed; check desktop permissions",
                ));
            }
            let value: Value = serde_json::from_slice(&bytes).or_else(|_| {
                let start = bytes.iter().position(|b| *b == b'{').unwrap_or(bytes.len());
                serde_json::from_slice(&bytes[start..])
            })?;
            if value["isError"] == true {
                return Err(Error::new(502, "Computer driver rejected the action"));
            }
            Ok(if value["structuredContent"].is_object() {
                value["structuredContent"].clone()
            } else {
                value
            })
        };
        tokio::time::timeout(Duration::from_secs(45), operation)
            .await
            .map_err(|_| Error::new(408, "Computer driver timed out"))?
    }
    async fn ensure(&mut self) -> Result<()> {
        if let Some(child) = &mut self.daemon {
            if child.try_wait()?.is_none() {
                return Ok(());
            }
        }
        self.daemon = None;
        self.daemon = Some(
            self.command()
                .args([
                    "serve",
                    "--embedded",
                    "--socket",
                    &self.socket,
                    "--host-bundle-id",
                    &self.host,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            if self.daemon.as_mut().unwrap().try_wait()?.is_some() {
                break;
            }
            let status = tokio::time::timeout(
                Duration::from_secs(2),
                self.cli(&[
                    "call".into(),
                    "check_permissions".into(),
                    permission_probe().to_string(),
                    "--socket".into(),
                    self.socket.clone(),
                ]),
            )
            .await;
            if let Ok(Ok(value)) = status {
                if value["accessibility"].is_boolean() || value["uia"].is_boolean() {
                    self.permissions = value;
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        self.daemon.take();
        Err(Error::new(
            502,
            "Computer driver could not start; check Accessibility and Screen Recording permissions",
        ))
    }
    async fn call(&mut self, tool: &str, payload: Value) -> Result<Value> {
        self.ensure().await?;
        self.cli(&[
            "call".into(),
            tool.into(),
            payload.to_string(),
            "--socket".into(),
            self.socket.clone(),
        ])
        .await
    }
    async fn revoke(&self, session: &str) {
        let _ = self
            .cli(&[
                "revoke".into(),
                "--session".into(),
                session.into(),
                "--socket".into(),
                self.socket.clone(),
            ])
            .await;
    }
}

impl Runtime {
    pub async fn cancel_computer(&self) {
        let mut configured = self.computer.lock().await;
        if let Some(driver) = configured.as_mut() {
            if driver.daemon.is_some() {
                // stop prints plain text; its exit still drains driver sessions.
                // Bound graceful shutdown, then reap the owned child regardless.
                let _ = tokio::time::timeout(
                    Duration::from_secs(2),
                    driver.cli(&["stop".into(), "--socket".into(), driver.socket.clone()]),
                )
                .await;
            }
            if let Some(mut child) = driver.daemon.take() {
                let _ = child.kill().await;
            }
            driver.permissions = Value::Null;
        }
        if let Ok(mut observations) = lock(&self.observations) {
            observations.clear();
        }
    }
    pub fn configure_computer_driver(&self, binary: PathBuf, host: String) -> Result<()> {
        let mut configured = self
            .computer
            .try_lock()
            .map_err(|_| Error::new(409, "Computer driver is busy"))?;
        #[cfg(unix)]
        let socket = self
            .root
            .join(format!("cua-{}.sock", std::process::id()))
            .to_string_lossy()
            .into_owned();
        #[cfg(windows)]
        let socket = format!(r"\\.\pipe\potato-native-{}", uuid::Uuid::new_v4());
        *configured = Some(Computer {
            binary,
            socket,
            host,
            daemon: None,
            permissions: Value::Null,
        });
        Ok(())
    }
    pub(crate) async fn computer_status(&self) -> Result<Value> {
        let configured = self.computer.lock().await;
        let available = configured.as_ref().is_some_and(|c| c.binary.is_file());
        Ok(
            json!({"enabled":self.db()?.get("computer_enabled",json!(false))?,"driver_available":available,
            "driver_path":configured.as_ref().map(|c|c.binary.to_string_lossy().into_owned()).unwrap_or_default(),
            "driver_version":configured.as_ref().and_then(|c|std::fs::read_to_string(c.binary.with_file_name("VERSION")).ok()).map(|v|v.trim().to_owned()).unwrap_or_default(),
            "permissions":configured.as_ref().map(|c|c.permissions.clone()).unwrap_or(Value::Null),
            "always_allowed_apps":[],"platform":std::env::consts::OS,
            "hint":if available{"Native driver starts when first used; every action requires approval"}else{"Native computer driver is not bundled"}}),
        )
    }
    pub(crate) async fn check_computer_permissions(&self) -> Result<Value> {
        {
            let mut configured = self.computer.lock().await;
            let driver = configured
                .as_mut()
                .filter(|c| c.binary.is_file())
                .ok_or_else(|| Error::new(400, "Native computer driver is not bundled"))?;
            driver.permissions = Value::Null;
            driver.permissions = driver.call("check_permissions", permission_probe()).await?;
        }
        self.computer_status().await
    }
    pub(crate) fn computer_target(&self, chat: &str, name: &str, args: &Value) -> Result<String> {
        if self.db()?.get("computer_enabled", json!(false))? != true {
            return Err(Error::new(403, "Enable computer use in Settings first"));
        }
        if name == "computer_list_apps" {
            return Ok("List desktop applications".into());
        }
        if name == "computer_observe" {
            return Ok(required(args, "app")?.to_owned());
        }
        let mut observations = lock(&self.observations)?;
        let observation = observations
            .get_mut(required(args, "observation_id")?)
            .ok_or_else(|| Error::new(409, "Observe the application before acting"))?;
        if observation.chat != chat || observation.expires < Instant::now() {
            return Err(Error::new(
                409,
                "Observation expired or belongs to another conversation",
            ));
        }
        let claimed = string(args, "app").to_lowercase();
        if !claimed.is_empty()
            && claimed != observation.bundle.to_lowercase()
            && claimed != observation.app.to_lowercase()
        {
            return Err(Error::new(403, "Application does not match observation"));
        }
        observation.expires = Instant::now() + Duration::from_secs(330);
        Ok(format!("{} ({})", observation.app, observation.bundle))
    }
    pub(crate) async fn computer_tool(
        &self,
        chat: &str,
        name: &str,
        args: &Value,
    ) -> Result<String> {
        if self.db()?.get("computer_enabled", json!(false))? != true {
            return Err(Error::new(403, "Computer use disabled"));
        }
        let mut configured = self.computer.lock().await;
        if self.db()?.get("computer_enabled", json!(false))? != true {
            return Err(Error::new(403, "Computer use disabled"));
        }
        let driver = configured
            .as_mut()
            .ok_or_else(|| Error::new(400, "Native computer driver is not bundled"))?;
        if name == "computer_list_apps" {
            return Ok(driver.call("list_apps", json!({})).await?.to_string());
        }
        if name == "computer_observe" {
            let apps = records(&driver.call("list_apps", json!({})).await?);
            let needle = required(args, "app")?.to_lowercase();
            let matches: Vec<_> = apps
                .iter()
                .filter(|app| {
                    field(app, &["bundle_id", "bundleId", "id"]).to_lowercase() == needle
                        || field(app, &["name", "display_name", "app_name"]).to_lowercase()
                            == needle
                })
                .collect();
            if matches.len() != 1 {
                return Err(Error::new(400,"Use an exact, unambiguous application name or bundle ID from computer_list_apps"));
            }
            let app = matches[0];
            let bundle = field(app, &["bundle_id", "bundleId", "id"]);
            let title = field(app, &["name", "display_name", "app_name"]);
            let pid = app["pid"]
                .as_u64()
                .filter(|pid| *pid > 0)
                .ok_or_else(|| Error::new(400, "Open the application before observing"))?;
            if protected(title, bundle, pid) {
                return Err(Error::new(
                    403,
                    "Potato, terminals and system settings are protected",
                ));
            }
            let windows = records(&driver.call("list_windows", json!({"pid":pid})).await?);
            let window = windows
                .iter()
                .max_by_key(|w| {
                    (
                        w["on_current_space"] == true,
                        w["is_on_screen"] == true,
                        !field(w, &["title", "name"]).is_empty(),
                        std::cmp::Reverse(w["z_index"].as_i64().unwrap_or(0)),
                    )
                })
                .ok_or_else(|| Error::new(404, "Application has no observable windows"))?;
            let window_id = window["window_id"]
                .as_u64()
                .or(window["id"].as_u64())
                .ok_or_else(|| Error::new(502, "Driver returned no window identity"))?;
            let id = uuid::Uuid::new_v4().to_string();
            let session = format!("potato-{id}");
            let mut payload = json!({"pid":pid,"window_id":window_id,"session":session,"include_screenshot":false});
            let state = match driver.call("get_window_state", payload.clone()).await {
                Ok(state) => state,
                Err(error) => {
                    driver.revoke(&session).await;
                    return Err(error);
                }
            };
            payload
                .as_object_mut()
                .unwrap()
                .remove("include_screenshot");
            let snapshot = match required(&state, "snapshot_id") {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    driver.revoke(&session).await;
                    return Err(Error::new(
                        409,
                        format!(
                            "Application has no actionable accessibility snapshot: {}",
                            state["degraded_reason"]
                                .as_str()
                                .unwrap_or("driver returned no snapshot identity")
                        ),
                    ));
                }
            };
            payload["snapshot_id"] = json!(snapshot);
            let elements = state["elements"].as_array().cloned().unwrap_or_default();
            let expired: Vec<_> = {
                let mut observations = lock(&self.observations)?;
                let ids: Vec<_> = observations
                    .iter()
                    .filter(|(_, o)| {
                        o.expires < Instant::now()
                            || (o.chat == chat && o.payload["window_id"] == window_id)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                let expired: Vec<_> = ids
                    .iter()
                    .filter_map(|id| observations.remove(id))
                    .collect();
                if observations.len() >= 64 {
                    return Err(Error::new(409, "Too many outstanding observations"));
                }
                observations.insert(
                    id.clone(),
                    Observation {
                        chat: chat.into(),
                        expires: Instant::now() + Duration::from_secs(120),
                        app: title.into(),
                        bundle: bundle.into(),
                        payload,
                        elements,
                    },
                );
                expired
            };
            for observation in expired {
                driver.revoke(string(&observation.payload, "session")).await;
            }
            return Ok(json!({"observation_id":id,"app":title,"bundle_id":bundle,"state":state,"note":"Observe again after each action. Use accessibility elements; screenshot capture is not enabled in this adapter."}).to_string());
        }
        let observation = {
            let mut observations = lock(&self.observations)?;
            let id = required(args, "observation_id")?;
            let observation = observations
                .get(id)
                .ok_or_else(|| Error::new(409, "Observation already consumed"))?;
            if observation.chat != chat || observation.expires < Instant::now() {
                return Err(Error::new(409, "Observation expired"));
            }
            observations.remove(id).unwrap()
        };
        let result = async {
            let tool = name.strip_prefix("computer_").unwrap_or("");
            let mut payload = observation.payload.clone();
            // 0.24.0 drag has no snapshot field; coordinates are window-bound.
            if tool == "drag" {
                payload.as_object_mut().unwrap().remove("snapshot_id");
            }
            let fields: &[&str] = match tool {
                "click" => &["x", "y", "button"],
                "set_value" => &["value"],
                "type_text" => &["text"],
                "press_key" => &["key"],
                "scroll" => &["direction", "amount"],
                "drag" => &["from_x", "from_y", "to_x", "to_y"],
                _ => return Err(Error::new(400, "Unknown computer action")),
            };
            for key in fields {
                if !args[*key].is_null() {
                    payload[*key] = args[*key].clone();
                }
            }
            if tool != "set_value" {
                payload["delivery_mode"] = json!("background");
            }
            if let Some(index) = args["element_index"].as_u64() {
                let element = observation
                    .elements
                    .iter()
                    .find(|e| e["element_index"] == index)
                    .ok_or_else(|| Error::new(400, "Element is not in the observation"))?;
                payload["element_index"] = json!(index);
                payload["element_token"] = json!(required(element, "element_token")?);
            }
            if tool == "set_value" && payload["element_token"].is_null() {
                return Err(Error::new(
                    400,
                    "Setting a value requires an observed element",
                ));
            }
            driver.call(tool, payload).await
        }
        .await;
        driver.revoke(string(&observation.payload, "session")).await;
        result.map(|value|json!({"result":value,"note":"Observe again before the next action; effect must be verified."}).to_string())
    }
}

pub(crate) fn definitions() -> Vec<Value> {
    let mut definitions = vec![
        json!({"type":"function","function":{"name":"computer_list_apps","description":"List desktop applications without launching them.","parameters":{"type":"object","properties":{},"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"computer_observe","description":"Read the accessibility tree of a running application by its exact name or bundle ID. Returns a single-use observation for background input.","parameters":{"type":"object","properties":{"app":{"type":"string"}},"required":["app"],"additionalProperties":false}}}),
    ];
    for (name, fields, required_fields) in [
        (
            "click",
            json!({"element_index":{"type":"integer"},"x":{"type":"number"},"y":{"type":"number"},"button":{"type":"string","enum":["left","right","middle"]}}),
            vec![],
        ),
        (
            "set_value",
            json!({"element_index":{"type":"integer"},"value":{"type":"string"}}),
            vec!["element_index", "value"],
        ),
        (
            "type_text",
            json!({"element_index":{"type":"integer"},"text":{"type":"string"}}),
            vec!["text"],
        ),
        (
            "press_key",
            json!({"element_index":{"type":"integer"},"key":{"type":"string"}}),
            vec!["key"],
        ),
        (
            "scroll",
            json!({"element_index":{"type":"integer"},"direction":{"type":"string","enum":["up","down","left","right"]},"amount":{"type":"integer","minimum":1,"maximum":50}}),
            vec!["direction"],
        ),
        (
            "drag",
            json!({"from_x":{"type":"number"},"from_y":{"type":"number"},"to_x":{"type":"number"},"to_y":{"type":"number"}}),
            vec!["from_x", "from_y", "to_x", "to_y"],
        ),
    ] {
        let mut properties = fields;
        properties["observation_id"] = json!({"type":"string"});
        properties["app"] = json!({"type":"string"});
        let mut required = vec!["observation_id"];
        required.extend(required_fields);
        definitions.push(json!({"type":"function","function":{"name":format!("computer_{name}"),"description":"Perform one background action on the exact observed window after user approval. The observation is consumed; observe again to verify the effect and before another action.","parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}}));
    }
    definitions
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn self_and_protected_apps_cannot_be_controlled() {
        assert!(protected("Potato Rust Preview", "", 10));
        assert!(protected("终端", "com.apple.Terminal", 10));
        assert!(protected("anything", "", std::process::id() as u64));
        assert!(!protected("Calculator", "com.apple.calculator", 10));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn driver_observations_are_chat_bound_single_use_and_background_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("driver");
        std::fs::write(&binary,r#"#!/bin/sh
printf '%s\n' "$@" >> "$(dirname "$0")/calls"
case "$1" in
serve) exec sleep 60;;
status) printf 'Cua Driver daemon is running';;
revoke) printf '{}';;
call)
case "$2" in
check_permissions) printf '{"accessibility":true,"screen_recording":true}';;
list_apps) printf '{"apps":[{"name":"Calculator","bundle_id":"com.apple.calculator","pid":12345}]}';;
list_windows) printf '{"windows":[{"window_id":42}]}';;
get_window_state) if [ -f "$(dirname "$0")/degraded" ]; then printf '{"degraded":true,"degraded_reason":"window is unavailable","elements":[]}'; exit 0; fi; printf '{"snapshot_id":"snap-1","elements":[{"element_index":1,"element_token":"token-1","label":"7"}]}';;
*) printf '{"effect":"delivered"}';;
esac;;
esac
"#).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = Runtime::open(&tmp.path().join("data")).unwrap();
        runtime
            .configure_computer_driver(binary, "test.host".into())
            .unwrap();
        runtime
            .db()
            .unwrap()
            .put("computer_enabled", &json!(true))
            .unwrap();
        assert!(!tmp.path().join("calls").exists());
        let observed: Value = serde_json::from_str(
            &runtime
                .computer_tool("chat-a", "computer_observe", &json!({"app":"Calculator"}))
                .await
                .unwrap(),
        )
        .unwrap();
        let action = json!({"observation_id":observed["observation_id"],"app":"Calculator","element_index":1});
        assert!(runtime
            .computer_target("chat-b", "computer_click", &action)
            .is_err());
        assert!(runtime
            .computer_target("chat-a", "computer_click", &action)
            .is_ok());
        runtime
            .computer_tool("chat-a", "computer_click", &action)
            .await
            .unwrap();
        assert!(runtime
            .computer_tool("chat-a", "computer_click", &action)
            .await
            .is_err());
        let calls = std::fs::read_to_string(tmp.path().join("calls")).unwrap();
        assert!(calls.contains("\"delivery_mode\":\"background\""));
        assert!(calls.contains("\"snapshot_id\":\"snap-1\""));
        assert!(calls.contains("\"element_token\":\"token-1\""));
        assert!(calls.contains("revoke"));
        let observed: Value = serde_json::from_str(
            &runtime
                .computer_tool("chat-a", "computer_observe", &json!({"app":"Calculator"}))
                .await
                .unwrap(),
        )
        .unwrap();
        runtime.computer_tool("chat-a", "computer_drag", &json!({"observation_id":observed["observation_id"],"from_x":1,"from_y":1,"to_x":2,"to_y":2})).await.unwrap();
        let calls = std::fs::read_to_string(tmp.path().join("calls")).unwrap();
        let drag = calls.lines().find(|line| line.contains("from_x")).unwrap();
        assert!(!drag.contains("snapshot_id"));
        std::fs::write(tmp.path().join("degraded"), "").unwrap();
        let error = runtime
            .computer_tool("chat-a", "computer_observe", &json!({"app":"Calculator"}))
            .await
            .unwrap_err();
        assert!(error.message.contains("window is unavailable"));
        assert!(lock(&runtime.observations).unwrap().is_empty());
        runtime
            .request("PUT", "/api/computer-use", json!({"enabled":false}))
            .await
            .unwrap();
        assert!(runtime
            .computer
            .lock()
            .await
            .as_ref()
            .unwrap()
            .daemon
            .is_none());
        assert!(lock(&runtime.observations).unwrap().is_empty());
        assert!(runtime
            .computer_tool("chat-a", "computer_list_apps", &json!({}))
            .await
            .is_err());
    }
    #[tokio::test]
    async fn missing_driver_cannot_be_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        assert!(runtime
            .request("PUT", "/api/computer-use", json!({"enabled":true}))
            .await
            .is_err());
        let status = runtime
            .request("GET", "/api/computer-use", Value::Null)
            .await
            .unwrap();
        assert_eq!(status["enabled"], false);
        assert_eq!(status["driver_available"], false);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "Requires the official driver and desktop permissions; read-only real-driver probe"]
    async fn live_driver_observation() {
        let binary = std::env::var_os("POTATO_TEST_COMPUTER_DRIVER").expect("driver path");
        let app =
            std::env::var("POTATO_TEST_COMPUTER_APP").expect("exact running test application");
        let tmp = tempfile::tempdir_in("/tmp").unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        runtime
            .configure_computer_driver(binary.into(), "dev.potato.gpui-review".into())
            .unwrap();
        let result: Result<()> = async {
            let health = runtime.check_computer_permissions().await?;
            println!("permissions: {}", health["permissions"]);
            runtime.request("PUT", "/api/computer-use", json!({"enabled":true})).await?;
            let observed: Value = serde_json::from_str(&runtime.computer_tool("live-test", "computer_observe", &json!({"app":app})).await?)?;
            assert!(observed["state"]["elements"].as_array().is_some_and(|e|!e.is_empty()));
            println!("Observed {} accessibility elements", observed["state"]["elements"].as_array().unwrap().len());
            if std::env::var("POTATO_TEST_COMPUTER_ACTIONS").as_deref() == Ok("1") {
                assert_eq!(app, "dev.cua.native-fixture", "Actions are allowed only on the disposable fixture");
                let elements = observed["state"]["elements"].as_array().unwrap();

                let input = elements.iter().find(|e|e.to_string().contains("Integration input")).expect("fixture input");
                runtime.computer_tool("live-test", "computer_set_value", &json!({"observation_id":observed["observation_id"], "element_index":input["element_index"], "value":"native-driver-verified"})).await?;
                let next: Value = serde_json::from_str(&runtime.computer_tool("live-test", "computer_observe", &json!({"app":app})).await?)?;
                assert!(next["state"]["elements"].to_string().contains("native-driver-verified"));
                let button = next["state"]["elements"].as_array().unwrap().iter().find(|e|e.to_string().contains("Increment")).expect("fixture button");
                runtime.computer_tool("live-test", "computer_click", &json!({"observation_id":next["observation_id"], "element_index":button["element_index"]})).await?;
                let verified: Value = serde_json::from_str(&runtime.computer_tool("live-test", "computer_observe", &json!({"app":app})).await?)?;
                assert!(verified["state"]["elements"].to_string().contains("Clicks: 1"));
                println!("Fixture value and background click verified by fresh observations");
            }
            Ok(())
        }.await;
        runtime.cancel_computer().await;
        result.unwrap();
    }
}
