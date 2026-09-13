use crate::view::{muted, row_button};
use crate::*;
use gpui_kit::component::button::*;
use gpui_kit::prelude::*;

impl Potato {
    pub(crate) fn cloud_settings_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let value = self
            .settings
            .data
            .get("cloud")
            .cloned()
            .unwrap_or(Value::Null);
        let mut panel = v_flex()
            .gap_3()
            .p_4()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("云端模型"));
        if self.settings.loading.contains("cloud") {
            return panel.child(muted("正在读取账号…", cx)).into_any_element();
        }
        if let Some(error) = self.settings.load_errors.get("cloud") {
            panel = panel.child(muted(error, cx));
        }
        if let Some(error) = value["local_config_error"].as_str() {
            panel = panel.child(muted(error, cx));
        }
        if value["has_session"] == true {
            panel = panel.child(muted(
                format!(
                    "{} · {}",
                    value["email"].as_str().unwrap_or(""),
                    if value["signed_in"] == true {
                        "已登录"
                    } else {
                        "登录已过期"
                    }
                ),
                cx,
            ));
            if let Some(error) = value["error"].as_str() {
                panel = panel.child(muted(error, cx));
            }
            let controls = h_flex()
                .gap_2()
                .child(
                    Button::new("cloud-refresh")
                        .outline()
                        .small()
                        .label("刷新模型")
                        .disabled(self.settings.cloud_pending || value["signed_in"] != true)
                        .on_click(cx.listener(|s, _, w, cx| s.cloud_action("refresh", w, cx))),
                )
                .child(
                    Button::new("cloud-logout")
                        .small()
                        .label("退出登录")
                        .disabled(self.settings.cloud_pending)
                        .on_click(cx.listener(|s, _, w, cx| s.cloud_action("logout", w, cx))),
                );
            panel = panel.child(controls);
            if let Some(provider) = self.providers.iter().find(|p| p["id"] == "potato-cloud") {
                for (i, model) in provider["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let id = string(model, "id");
                    let selected =
                        self.model["provider_id"] == "potato-cloud" && self.model["model"] == id;
                    panel = panel.child(
                        row_button(format!("cloud-model-{i}"), string(model, "name"))
                            .child(div().flex_1())
                            .child(muted(
                                if selected {
                                    "当前使用"
                                } else {
                                    "使用此模型"
                                },
                                cx,
                            ))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                if s.streaming {
                                    s.notice = "请等待当前回复结束后切换模型".into();
                                    return;
                                }
                                s.request_result(
                                    "PUT",
                                    "/api/models/active",
                                    json!({"provider_id":"potato-cloud","model":id}),
                                    w,
                                    cx,
                                    |s, r, w, cx| match r {
                                        Ok(v) => {
                                            s.model = v["active_llm"].clone();
                                            s.prepare_effort(w, cx);
                                            s.notice = "已切换云端模型".into();
                                        }
                                        Err(e) => s.notice = e,
                                    },
                                );
                            })),
                    );
                }
            }
            panel = panel.child(muted(
                "模型列表由邀请你的人管理。文件与命令在本机执行，沿用本机审批设置。",
                cx,
            ));
        } else if let Some(url) = value["login"]["verification_url"].as_str() {
            let url = url.to_owned();
            panel = panel
                .child(muted(
                    "在浏览器输入邮箱和收到的验证码，再核对下方确认码。",
                    cx,
                ))
                .child(
                    div()
                        .text_lg()
                        .child(value["login"]["code"].as_str().unwrap_or("").to_owned()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("cloud-login-open")
                                .outline()
                                .label("打开邮箱登录页面")
                                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                        )
                        .child(Button::new("cloud-login-cancel").label("取消").on_click(
                            cx.listener(|s, _, w, cx| s.cloud_action("login/cancel", w, cx)),
                        )),
                );
        } else {
            panel = panel.child(muted("使用受邀邮箱登录，即可使用云端模型。无需注册 Cloudflare，也无需填写接口或密钥。",cx))
                .child(Button::new("cloud-login").primary().label(if self.settings.cloud_pending {"正在连接…"} else {"使用邮箱登录"}).disabled(self.settings.cloud_pending)
                    .on_click(cx.listener(|s,_,w,cx|s.cloud_action("login/start",w,cx))));
        }
        panel.into_any_element()
    }

    pub(crate) fn cloud_action(&mut self, action: &str, w: &mut Window, cx: &mut Context<Self>) {
        self.settings.cloud_generation += 1;
        let generation = self.settings.cloud_generation;
        self.settings.cloud_pending = true;
        self.notice.clear();
        let opening = action == "login/start";
        self.request_result(
            "POST",
            &format!("/api/native/cloud/{action}"),
            json!({}),
            w,
            cx,
            move |s, r, w, cx| {
                if s.settings.cloud_generation != generation {
                    return;
                }
                s.settings.cloud_pending = false;
                match r {
                    Ok(v) => {
                        if opening && let Some(url) = v["login"]["verification_url"].as_str() {
                            cx.open_url(url);
                        }
                        s.settings.data.insert("cloud".into(), v);
                        s.settings.load_errors.remove("cloud");
                        s.refresh(w, cx);
                    }
                    Err(e) => s.notice = e,
                }
            },
        );
    }

    pub(crate) fn poll_cloud_login_ui(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings.cloud_pending
            || !self
                .settings
                .data
                .get("cloud")
                .is_some_and(|v| v["login"].is_object())
        {
            return;
        }
        self.settings.cloud_pending = true;
        let generation = self.settings.cloud_generation;
        self.request_result(
            "POST",
            "/api/native/cloud/login/poll",
            json!({}),
            w,
            cx,
            move |s, r, w, cx| {
                if generation != s.settings.cloud_generation {
                    return;
                }
                s.settings.cloud_pending = false;
                match r {
                    Ok(v) => {
                        let completed = !v["login"].is_object();
                        s.settings.data.insert("cloud".into(), v);
                        if completed {
                            s.refresh(w, cx);
                        }
                    }
                    Err(e) => {
                        s.notice = e;
                        // Stop automatic retries; reopening Settings recovers any pending handoff.
                        if let Some(v) = s.settings.data.get_mut("cloud") {
                            v["login"] = Value::Null;
                        }
                        s.request_result(
                            "GET",
                            "/api/native/cloud",
                            Value::Null,
                            w,
                            cx,
                            move |s, r, _, _| {
                                if s.settings.cloud_generation != generation {
                                    return;
                                }
                                if let Ok(mut v) = r {
                                    v["login"] = Value::Null;
                                    s.settings.data.insert("cloud".into(), v);
                                }
                            },
                        );
                    }
                }
            },
        );
    }
}
