use super::*;

pub(super) fn mcp_editor_config(item: Option<&Value>) -> Value {
    match item {
        Some(item) => {
            let mut config = json!({});
            for key in [
                "name",
                "description",
                "transport",
                "enabled",
                "url",
                "headers",
                "command",
                "args",
                "env",
                "cwd",
            ] {
                if let Some(value) = item.get(key) {
                    config[key] = value.clone();
                }
            }
            config
        }
        None => {
            json!({"name":"MCP 服务", "transport":"stdio", "command":"", "args":[], "env":{}, "cwd":"", "enabled":true})
        }
    }
}

impl Potato {
    pub(super) fn set_mcp_transport(
        &mut self,
        transport: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(mut config) = serde_json::from_str::<Value>(&self.editor.read(cx).value()) else {
            self.notice = "请先修正配置 JSON，再切换连接方式".into();
            return;
        };
        if !config.is_object() {
            self.notice = "配置必须是 JSON 对象".into();
            return;
        }
        config["transport"] = json!(transport);
        if transport == "streamable_http" {
            if config.get("url").is_none() {
                config["url"] = json!("");
            }
            if config.get("headers").is_none() {
                config["headers"] = json!({});
            }
        } else {
            if config.get("command").is_none() {
                config["command"] = json!("");
            }
            if config.get("args").is_none() {
                config["args"] = json!([]);
            }
            if config.get("env").is_none() {
                config["env"] = json!({});
            }
        }
        self.editor.update(cx, |v, cx| {
            v.set_value(serde_json::to_string_pretty(&config).unwrap(), w, cx)
        });
    }

    pub(super) fn is_mcp_page(&self) -> bool {
        self.page == Page::Skills && self.workspace.tab == "mcp"
    }

    pub(super) fn workspace_item_path(&self, id: &str) -> String {
        if self.is_mcp_page() {
            format!("/api/mcp/{}", segment(id))
        } else {
            item_path(self.page, id)
        }
    }

    pub(super) fn import_skill_zip(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        cx.spawn_in(w, async move |this, cx| {
            let Some(file) = rfd::AsyncFileDialog::new().add_filter("技能包", &["zip"]).pick_file().await else { return; };
            let path = file.path().to_path_buf();
            let filename = file.file_name();
            let result = backend.executor.spawn(async move {
                use base64::Engine;
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(path).map_err(|e| e.to_string())?.take(20_000_001).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                if bytes.len() > 20_000_000 { return Err("技能包超过 20 MB".to_owned()); }
                Ok(json!({"filename":filename, "base64":base64::engine::general_purpose::STANDARD.encode(bytes)}))
            }).await.map_err(|e| e.to_string()).and_then(|v| v);
            let _ = this.update_in(cx, |s, w, cx| {
                match result {
                    Ok(body) => s.request_result("POST", "/api/skills/upload", body, w, cx, |s, result, w, cx| {
                        match result {
                            Ok(value) => { s.notice = format!("已导入技能 {}", string(&value, "name"));
                                if s.page == Page::Skills && s.workspace.tab.is_empty() { s.load_page(w, cx); }
                            }
                            Err(error) => s.notice = error,
                        }
                    }),
                    Err(error) => s.notice = error,
                }
                cx.notify();
            });
        }).detach();
    }

    fn mcp_action(
        &mut self,
        method: &'static str,
        path: &str,
        body: Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.mcp_busy {
            return;
        }
        self.workspace.mcp_busy = true;
        self.notice = if method == "GET" {
            "正在连接并发现工具…".into()
        } else {
            "正在更新…".into()
        };
        self.request_result(method, path, body, w, cx, |s, result, w, cx| {
            s.workspace.mcp_busy = false;
            match result {
                Ok(_) => {
                    s.notice = "MCP 服务已更新".into();
                    if s.is_mcp_page() {
                        s.load_page(w, cx);
                    }
                }
                Err(error) => s.notice = error,
            }
        });
    }

    pub(super) fn mcp_row(
        &mut self,
        index: usize,
        item: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = string(item, "key");
        let selected = item.clone();
        let discover_key = key.clone();
        let toggle_key = key.clone();
        let enabled = item["enabled"] == true;
        let busy = self.workspace.mcp_busy;
        let mut row = div()
            .flex()
            .flex_col()
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new(("mcp-edit", index))
                            .ghost()
                            .flex_1()
                            .justify_start()
                            .accessibility_label(string(item, "name"))
                            .child(div().w_full().text_left().child(string(item, "name")))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.edit_item(Some(selected.clone()), w, cx)
                            })),
                    )
                    .child(muted(
                        format!(
                            "{} · {} 个工具",
                            string(item, "transport"),
                            item["discovered_tools"].as_u64().unwrap_or(0)
                        ),
                        cx,
                    ))
                    .child(
                        Button::new(("mcp-discover", index))
                            .outline()
                            .small()
                            .label("发现工具")
                            .disabled(!enabled || busy)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.mcp_action(
                                    "GET",
                                    &format!("/api/mcp/tools/{}", segment(&discover_key)),
                                    Value::Null,
                                    w,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Switch::new(("mcp-enabled", index))
                            .small()
                            .checked(enabled)
                            .disabled(busy)
                            .accessibility_label(format!("启用 MCP {}", string(item, "name")))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.mcp_action(
                                    "PATCH",
                                    &format!("/api/mcp/toggle/{}", segment(&toggle_key)),
                                    Value::Null,
                                    w,
                                    cx,
                                )
                            })),
                    ),
            );
        let catalog = item["tool_catalog"].as_array().cloned().unwrap_or_default();
        if catalog.is_empty() {
            row = row.child(muted(
                "尚未发现工具。启用服务后点击“发现工具”，成功后助手即可调用。",
                cx,
            ));
        }
        for (tool_index, tool) in catalog.iter().enumerate() {
            let name = string(tool, "name");
            let tools = item["tools"]
                .as_array()
                .cloned()
                .unwrap_or_else(|| catalog.iter().map(|t| t["name"].clone()).collect());
            let checked = tools.iter().any(|t| t == &name);
            let key = key.clone();
            let toggle_name = name.clone();
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .pl_4()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(name.clone())
                            .child(muted(string(tool, "description"), cx)),
                    )
                    .child(
                        Switch::new(("mcp-tool", index * 128 + tool_index))
                            .small()
                            .checked(checked)
                            .disabled(!enabled || busy)
                            .accessibility_label(format!("启用工具 {name}"))
                            .on_click(cx.listener(move |s, on, w, cx| {
                                let mut tools = tools.clone();
                                tools.retain(|t| t != &toggle_name);
                                if *on {
                                    tools.push(json!(toggle_name));
                                }
                                s.mcp_action(
                                    "PUT",
                                    &format!("/api/mcp/tools/{}", segment(&key)),
                                    json!({"tools":tools}),
                                    w,
                                    cx,
                                );
                            })),
                    ),
            );
        }
        row.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Backend, Page, Potato};
    use gpui_kit::{TestAppContext, gpui};
    use serde_json::{Value, json};

    #[gpui::test]
    fn mcp_editor_saves_real_server_and_preserves_masked_credentials(cx: &mut TestAppContext) {
        let backend = Backend::for_ui_test(
            std::env::temp_dir().join(format!("potato-mcp-ui-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        cx.update(gpui_kit::init);
        let (app, cx) = cx.add_window_view(|w, cx| Potato::new(backend, w, cx));
        cx.run_until_parked();
        cx.update(|w, cx| app.update(cx, |s, cx| {
            s.page = Page::Skills;
            s.workspace.tab = "mcp".into();
            s.edit_item(None, w, cx);
            s.fields["document-name"].update(cx, |v, cx| v.set_value("fixture", w, cx));
            s.editor.update(cx, |v, cx| v.set_value(r#"{"name":"Fixture","transport":"streamable_http","url":"http://127.0.0.1:9/mcp","headers":{"Authorization":"synthetic-secret"}}"#, w, cx));
            s.save_document(w, cx);
        }));
        cx.run_until_parked();
        cx.update(|w, cx| {
            app.update(cx, |s, cx| {
                assert!(!s.workspace.editing, "{}", s.notice);
                assert_eq!(s.workspace.list.len(), 1);
                let item = s.workspace.list[0].clone();
                assert_eq!(item["key"], "fixture");
                assert_eq!(item["headers"]["authorization"], "********");
                s.edit_item(Some(item), w, cx);
                assert!(!s.document_dirty(cx));
                assert!(!s.editor.read(cx).value().contains("synthetic-secret"));
                s.editor.update(cx, |v, cx| {
                    let mut config: Value = serde_json::from_str(&v.value()).unwrap();
                    config["name"] = json!("Renamed");
                    v.set_value(config.to_string(), w, cx);
                });
                s.save_document(w, cx);
            })
        });
        cx.run_until_parked();
        app.read_with(cx, |s, _| {
            assert!(!s.workspace.editing, "{}", s.notice);
            assert_eq!(s.workspace.list[0]["name"], "Renamed");
            assert_eq!(s.workspace.list[0]["headers"]["authorization"], "********");
            assert_eq!(s.workspace_item_path("fixture"), "/api/mcp/fixture");
        });
    }
}
