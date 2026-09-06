use super::{Destination, Event, NoticeTone, Operation, Section, Settings};
use crate::accessibility::{button, pick_list, text_input};
use crate::{ui, Message};
use iced::widget::{
    column, container, mouse_area, opaque, row, scrollable, stack, svg, text, tooltip, Column,
    Space,
};
use iced::{alignment, border, Color, Element, Fill, Shadow, Size, Theme, Vector};

#[derive(Clone, PartialEq, Eq)]
struct ProviderChoice {
    id: String,
    name: String,
}
impl std::fmt::Display for ProviderChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

fn dark(theme: &Theme) -> bool {
    theme.palette().background.r < 0.5
}
fn line(theme: &Theme) -> Color {
    if dark(theme) {
        Color::from_rgb8(62, 62, 62)
    } else {
        Color::from_rgb8(225, 225, 222)
    }
}
fn group(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.palette().background.into()),
        border: border::rounded(10).width(1).color(line(theme)),
        ..Default::default()
    }
}
fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        border: border::rounded(8).width(1).color(line(theme)),
        ..ui::nav(theme, status)
    }
}
fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::primary(theme, status);
    style.border = border::rounded(8);
    style
}
fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.background = if dark(theme) {
        Color::from_rgb8(28, 28, 28)
    } else {
        Color::WHITE
    }
    .into();
    style.border = border::rounded(8).width(1).color(
        if matches!(status, text_input::Status::Focused { .. }) {
            theme.palette().text
        } else {
            line(theme)
        },
    );
    style
}
fn picker(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let mut style = pick_list::default(theme, status);
    style.border = border::rounded(8).width(1).color(line(theme));
    style.background = theme.palette().background.into();
    style
}
fn picker_menu(theme: &Theme) -> iced::widget::overlay::menu::Style {
    let mut style = iced::widget::overlay::menu::default(theme);
    style.border = border::rounded(8).width(1).color(line(theme));
    style
}
fn icon(path: &str, theme: &Theme) -> Element<'static, Message> {
    let c = theme.palette().text;
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="rgb({},{},{})" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{path}</svg>"#,
        (c.r * 255.) as u8,
        (c.g * 255.) as u8,
        (c.b * 255.) as u8
    );
    svg(svg::Handle::from_memory(source.into_bytes()))
        .width(16)
        .height(16)
        .into()
}
fn section_icon(section: Section, theme: &Theme) -> Element<'static, Message> {
    icon(
        match section {
            Section::Models => {
                r#"<rect x="3" y="7" width="18" height="14" rx="3"/><path d="M12 7V3H9M7 13h.01M17 13h.01M9 17h6"/>"#
            }
            Section::General => {
                r#"<path d="M3 6h5m4 0h9M3 12h11m4 0h3M3 18h3m4 0h11M8 3v6m6 0v6M6 15v6"/>"#
            }
            Section::Capabilities => {
                r#"<path d="M8 3h4a3 3 0 1 0 6 0h3v6a3 3 0 1 0 0 6v6h-6a3 3 0 1 0-6 0H3v-6a3 3 0 1 0 0-6V3h5"/>"#
            }
            Section::Security => r#"<path d="m12 3 8 3-1 9-7 6-7-6-1-9zM8 11l3 3 5-5"/>"#,
            Section::Data => {
                r#"<rect x="3" y="12" width="18" height="8" rx="2"/><path d="m3 12 3-8h12l3 8M7 16h.01M11 16h.01"/>"#
            }
            Section::Shortcuts => {
                r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M10 13h.01M14 13h.01M18 13h.01M8 16h8"/>"#
            }
            Section::About => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>"#,
        },
        theme,
    )
}
fn label(value: &str) -> iced::widget::Text<'_> {
    text(value).size(13)
}
fn heading(value: &str, size: u32) -> iced::widget::Text<'_> {
    text(value).size(size).font(iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::DEFAULT
    })
}
fn setting_row<'a>(
    title: &'a str,
    description: &'a str,
    control: impl Into<Element<'a, Message>>,
    theme: &Theme,
) -> Element<'a, Message> {
    let mut labels = column![label(title)].spacing(4).width(Fill);
    if !description.is_empty() {
        labels = labels.push(text(description).size(12).color(ui::muted(theme)));
    }
    container(
        row![labels, control.into()]
            .spacing(16)
            .align_y(alignment::Vertical::Center),
    )
    .padding([14, 16])
    .width(Fill)
    .into()
}
impl Settings {
    fn action(
        &self,
        title: &'static str,
        operation: Operation,
        enabled: bool,
    ) -> Element<'static, Message> {
        button(label(title))
            .padding([7, 12])
            .style(secondary)
            .on_press_maybe(
                (!self.busy && enabled).then_some(Message::Preferences(Event::Run(operation))),
            )
            .into()
    }
    fn field(
        &self,
        placeholder: &'static str,
        key: &'static str,
        secret: bool,
    ) -> Element<'_, Message> {
        let id = iced::widget::Id::from(format!("settings-{key}"));
        text_input(placeholder, self.value(key))
            .accessibility_label(match key {
                "max_tokens" => "最大输出 Token",
                "max_input_length" => "上下文长度",
                "reasoning_effort" => "推理强度",
                "edit_name" => "模型显示名称",
                "key" => "API key",
                "url" => "Base URL",
                "app" => "App ID",
                "resource" => "资源 ID",
                "search_provider" => "托管搜索服务商",
                "search_model" => "托管搜索模型",
                _ => placeholder,
            })
            .id(id.clone())
            .secure(secret)
            .size(13)
            .padding([8, 10])
            .width(Fill)
            .style(input)
            .on_input_maybe(
                (!self.busy).then_some(move |v| Message::Preferences(Event::Field(key, v))),
            )
            .into()
    }
    fn form_row<'a>(
        &'a self,
        title: &'a str,
        description: &'a str,
        placeholder: &'static str,
        key: &'static str,
        secret: bool,
        theme: &Theme,
    ) -> Element<'a, Message> {
        setting_row(
            title,
            description,
            container(self.field(placeholder, key, secret)).width(256),
            theme,
        )
    }
    fn protocol(&self) -> Element<'_, Message> {
        let options = vec!["Chat Completions".to_owned(), "Responses API".to_owned()];
        let selected = if self.value("protocol") == "OpenAIResponseModel" {
            options[1].clone()
        } else {
            options[0].clone()
        };
        pick_list(options, Some(selected), |v| {
            Message::Preferences(Event::Field(
                "protocol",
                if v == "Responses API" {
                    "OpenAIResponseModel"
                } else {
                    "OpenAIChatModel"
                }
                .into(),
            ))
        })
        .text_size(13)
        .padding([8, 10])
        .width(256)
        .style(picker)
        .menu_style(picker_menu)
        .into()
    }
    fn models(&self, theme: &Theme) -> Element<'_, Message> {
        if self.editing_model {
            return column![
                button(label("‹  返回服务商"))
                    .style(ui::nav)
                    .on_press(Message::Preferences(Event::Navigate(
                        Destination::Provider(self.value("id").into())
                    ))),
                heading(self.value("edit_model"), 16),
                container(column![
                    self.form_row("显示名称", "", "模型名称", "edit_name", false, theme),
                    self.form_row(
                        "最大输出 Token",
                        "留空使用服务商默认值。",
                        "自动",
                        "max_tokens",
                        false,
                        theme
                    ),
                    self.form_row(
                        "上下文长度",
                        "用于估算历史压缩阈值；留空使用默认值。",
                        "自动",
                        "max_input_length",
                        false,
                        theme
                    ),
                    self.form_row(
                        "推理强度",
                        "由模型支持，例如 low、medium、high。",
                        "默认",
                        "reasoning_effort",
                        false,
                        theme
                    ),
                    container(
                        row![
                            button(label("删除模型")).style(secondary).on_press_maybe(
                                (!self.busy).then_some(Message::Preferences(Event::Confirm(
                                    Operation::DeleteModel
                                )))
                            ),
                            Space::new().width(Fill),
                            self.action("保存参数", Operation::SaveModel, true)
                        ]
                        .spacing(8)
                    )
                    .padding(16)
                ])
                .style(group)
            ]
            .spacing(14)
            .into();
        }
        if self.creating {
            return column![
                button(label("‹  服务商列表"))
                    .style(ui::nav)
                    .padding([7, 10])
                    .on_press_maybe(
                        (!self.busy)
                            .then_some(Message::Preferences(Event::Navigate(Destination::List)))
                    ),
                container(column![
                    self.form_row("名称", "", "服务商名称", "name", false, theme),
                    iced::widget::rule::horizontal(1),
                    self.form_row(
                        "Base URL",
                        "",
                        "https://api.example.com/v1",
                        "url",
                        false,
                        theme
                    ),
                    iced::widget::rule::horizontal(1),
                    self.form_row("API key", "", "输入 API key", "key", true, theme),
                    iced::widget::rule::horizontal(1),
                    setting_row("接口协议", "", self.protocol(), theme)
                ])
                .style(group),
                self.action(
                    "添加服务商",
                    Operation::Create,
                    !self.value("name").trim().is_empty() && !self.value("url").trim().is_empty()
                )
            ]
            .spacing(12)
            .into();
        }
        if self.detail {
            let provider = self.providers.iter().find(|p| p["id"] == self.value("id"));
            let name = provider
                .and_then(|p| p["name"].as_str())
                .unwrap_or(self.value("id"));
            let configured = self.configured(self.value("id"));
            let mut models = Column::new();
            let mut seen = std::collections::HashSet::new();
            for model in provider.into_iter().flat_map(|p| {
                ["models", "extra_models"]
                    .into_iter()
                    .flat_map(move |k| p[k].as_array().into_iter().flatten())
            }) {
                let id = model["id"].as_str().unwrap_or("");
                if !seen.insert(id) {
                    continue;
                }
                models = models
                    .push(setting_row(
                        model["name"].as_str().unwrap_or(id),
                        id,
                        row![
                            button(label("参数"))
                                .style(ui::nav)
                                .padding([6, 10])
                                .on_press_maybe((!self.busy).then_some(Message::Preferences(
                                    Event::Navigate(Destination::Model(id.into()))
                                ))),
                            button(label(if self.is_active(self.value("id"), id) {
                                "✓ 当前使用"
                            } else {
                                "使用"
                            }))
                            .style(ui::nav)
                            .padding([6, 10])
                            .on_press_maybe(
                                (!self.busy && configured && !self.is_active(self.value("id"), id))
                                    .then_some(Message::Preferences(Event::Activate(
                                        self.value("id").into(),
                                        id.into(),
                                    ))),
                            )
                        ]
                        .spacing(4),
                        theme,
                    ))
                    .push(iced::widget::rule::horizontal(1));
            }
            let count = seen.len();
            let connection_dirty = ["url", "key", "protocol"]
                .iter()
                .any(|k| self.fields.get(k) != self.saved.get(k));
            column![
                button(label("‹  服务商列表"))
                    .style(ui::nav)
                    .padding([7, 10])
                    .on_press_maybe(
                        (!self.busy)
                            .then_some(Message::Preferences(Event::Navigate(Destination::List)))
                    ),
                container(setting_row(
                    name,
                    self.value("url"),
                    text(if configured { "已配置" } else { "未配置" })
                        .size(12)
                        .color(ui::muted(theme)),
                    theme
                ))
                .style(group),
                row![
                    button(label("清除已保存的 API key"))
                        .style(secondary)
                        .padding([7, 12])
                        .on_press_maybe(
                            (!self.busy && configured).then_some(Message::Preferences(
                                Event::Confirm(Operation::ClearKey)
                            ))
                        ),
                    Space::new().width(Fill),
                    button(label("删除服务商"))
                        .style(secondary)
                        .padding([7, 12])
                        .on_press_maybe(
                            (!self.busy && provider.is_some_and(|p| p["is_custom"] == true))
                                .then_some(Message::Preferences(Event::Confirm(
                                    Operation::DeleteProvider
                                )))
                        )
                ]
                .spacing(8),
                container(column![
                    self.form_row(
                        "API key",
                        if configured {
                            "已保存。输入新 key 可替换。"
                        } else {
                            ""
                        },
                        if configured {
                            "••••••••"
                        } else {
                            "输入 API key"
                        },
                        "key",
                        true,
                        theme
                    ),
                    iced::widget::rule::horizontal(1),
                    if provider.is_some_and(|p| p["freeze_url"] == true) {
                        setting_row("Base URL", "由系统管理", label(self.value("url")), theme)
                    } else {
                        self.form_row(
                            "Base URL",
                            "",
                            "https://api.example.com/v1",
                            "url",
                            false,
                            theme,
                        )
                    },
                    iced::widget::rule::horizontal(1),
                    setting_row("接口协议", "", self.protocol(), theme),
                    iced::widget::rule::horizontal(1),
                    setting_row(
                        "测试连接",
                        "",
                        row![
                            self.action("测试连接", Operation::Test, !self.value("url").is_empty()),
                            self.action("保存连接", Operation::Connection, connection_dirty)
                        ]
                        .spacing(8),
                        theme
                    )
                ])
                .style(group),
                container(column![
                    setting_row(
                        "模型",
                        "",
                        row![
                            text(count.to_string()).size(12).color(ui::muted(theme)),
                            self.action(
                                "发现模型",
                                Operation::Discover,
                                !self.value("url").is_empty()
                            )
                        ]
                        .spacing(12)
                        .align_y(alignment::Vertical::Center),
                        theme
                    ),
                    iced::widget::rule::horizontal(1),
                    models,
                    container(
                        column![
                            row![
                                self.field("模型 ID", "model", false),
                                self.field("显示名（可选）", "model_name", false)
                            ]
                            .spacing(8),
                            self.action(
                                "添加",
                                Operation::AddModel,
                                !self.value("model").trim().is_empty()
                            )
                        ]
                        .spacing(8)
                    )
                    .padding(16)
                ])
                .style(group)
            ]
            .spacing(12)
            .into()
        } else {
            let mut list = Column::new().spacing(2);
            for p in &self.providers {
                if p["api_key"].as_str().unwrap_or("").is_empty() && p["is_local"] != true {
                    continue;
                }
                list = list.push(self.provider_row(p, theme));
            }
            if !self
                .providers
                .iter()
                .any(|p| !p["api_key"].as_str().unwrap_or("").is_empty() || p["is_local"] == true)
            {
                list = list.push(
                    container(
                        text("添加服务商，连接你使用的模型。")
                            .size(13)
                            .color(ui::muted(theme)),
                    )
                    .padding(12),
                );
            }
            list = list.push(iced::widget::rule::horizontal(1)).push(
                button(
                    row![
                        text("＋").size(16),
                        label("添加服务商").width(Fill),
                        text(if self.add_open { "⌄" } else { "›" }).size(16)
                    ]
                    .spacing(8),
                )
                .padding([10, 12])
                .width(Fill)
                .style(ui::nav)
                .on_press_maybe((!self.busy).then_some(Message::Preferences(Event::ToggleAdd))),
            );
            if self.add_open {
                for p in &self.providers {
                    if p["api_key"].as_str().unwrap_or("").is_empty() && p["is_local"] != true {
                        list = list.push(self.provider_row(p, theme));
                    }
                }
                list = list.push(
                    button(label("＋  自定义服务商"))
                        .padding([10, 12])
                        .width(Fill)
                        .style(ui::nav)
                        .on_press(Message::Preferences(Event::Navigate(Destination::Create))),
                );
            }
            column![
                text("服务商").size(12).color(ui::muted(theme)),
                container(list).padding(8).style(group)
            ]
            .spacing(10)
            .into()
        }
    }
    fn provider_row<'a>(&'a self, p: &'a serde_json::Value, theme: &Theme) -> Element<'a, Message> {
        let count = ["models", "extra_models"]
            .iter()
            .flat_map(|k| p[*k].as_array().into_iter().flatten())
            .filter_map(|m| m["id"].as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        button(
            row![
                section_icon(Section::Models, theme),
                label(p["name"].as_str().unwrap_or("服务商")).width(Fill),
                text(format!("{count} 个模型"))
                    .size(12)
                    .color(ui::muted(theme)),
                text("›").size(16)
            ]
            .spacing(12)
            .align_y(alignment::Vertical::Center),
        )
        .padding([12, 12])
        .width(Fill)
        .style(ui::nav)
        .on_press_maybe((!self.busy).then_some(Message::Preferences(Event::Navigate(
            Destination::Provider(p["id"].as_str().unwrap_or("").into()),
        ))))
        .into()
    }
    fn body(&self, theme: &Theme) -> Element<'_, Message> {
        match self.section {
            Section::Models => self.models(theme),
            Section::General => column![
                heading("外观", 13),
                container(setting_row(
                    "主题",
                    "",
                    row![
                        button(label("跟随系统"))
                            .padding([7, 10])
                            .style(if self.value("follow_system") == "true" {
                                ui::selected
                            } else {
                                ui::nav
                            })
                            .on_press(Message::Preferences(Event::FollowSystem)),
                        button(label("浅色"))
                            .padding([7, 14])
                            .style(if self.value("follow_system") == "true" || dark(theme) {
                                ui::nav
                            } else {
                                ui::selected
                            })
                            .on_press(Message::Preferences(Event::Appearance(false))),
                        button(label("深色"))
                            .padding([7, 14])
                            .style(if self.value("follow_system") != "true" && dark(theme) {
                                ui::selected
                            } else {
                                ui::nav
                            })
                            .on_press(Message::Preferences(Event::Appearance(true)))
                    ]
                    .spacing(2),
                    theme
                ))
                .style(group),
                heading("联网搜索", 13),
                container(column![
                    setting_row(
                        "搜索方式",
                        "自动选择可用的搜索服务。",
                        pick_list(
                            vec!["auto", "hosted", "exa", "tavily"],
                            Some(if self.value("search_backend").is_empty() {
                                "auto"
                            } else {
                                self.value("search_backend")
                            }),
                            |v| Message::Preferences(Event::Field("search_backend", v.into()))
                        )
                        .text_size(13)
                        .width(256)
                        .style(picker)
                        .menu_style(picker_menu),
                        theme
                    ),
                    self.form_row(
                        "托管搜索服务商",
                        "留空使用当前模型的服务商。",
                        "服务商 ID",
                        "search_provider",
                        false,
                        theme
                    ),
                    self.form_row(
                        "托管搜索模型",
                        "需支持 Responses web_search。",
                        "留空使用当前模型",
                        "search_model",
                        false,
                        theme
                    ),
                    container(row![
                        Space::new().width(Fill),
                        self.action("保存搜索设置", Operation::Search, true)
                    ])
                    .padding(12)
                ])
                .style(group),
                container(column![
                    setting_row(
                        "记住窗口大小与位置",
                        "下次启动恢复上次窗口。",
                        button(label(if self.value("remember_window") != "false" {
                            "已开启"
                        } else {
                            "已关闭"
                        }))
                        .on_press(Message::Preferences(
                            Event::RememberWindow(self.value("remember_window") == "false")
                        )),
                        theme
                    ),
                    setting_row(
                        "恢复默认窗口",
                        "1080 × 760",
                        button(label("恢复")).on_press(Message::Preferences(Event::ResetWindow)),
                        theme
                    )
                ])
                .style(group)
            ]
            .spacing(12)
            .into(),
            Section::Capabilities => {
                let choices: Vec<ProviderChoice> = self
                    .providers
                    .iter()
                    .filter_map(|p| {
                        p["id"].as_str().map(|id| ProviderChoice {
                            id: id.into(),
                            name: p["name"].as_str().unwrap_or(id).into(),
                        })
                    })
                    .collect();
                let selected = choices
                    .iter()
                    .find(|p| p.id == self.value("image_provider"))
                    .cloned();
                column![
                    heading("语音输入", 13),
                    container(column![
                        setting_row(
                            "豆包语音识别",
                            "",
                            text(if self.speech_enabled {
                                "已启用"
                            } else {
                                "未启用"
                            })
                            .size(12)
                            .color(ui::muted(theme)),
                            theme
                        ),
                        iced::widget::rule::horizontal(1),
                        self.form_row(
                            "API key",
                            if self.speech_configured {
                                "已保存，留空保留。"
                            } else {
                                ""
                            },
                            "输入语音 API key",
                            "speech",
                            true,
                            theme
                        ),
                        iced::widget::rule::horizontal(1),
                        self.form_row(
                            "App ID",
                            "新版 API key 可留空。",
                            "App ID",
                            "app",
                            false,
                            theme
                        ),
                        iced::widget::rule::horizontal(1),
                        self.form_row(
                            "资源 ID",
                            "",
                            "volc.seedasr.sauc.duration",
                            "resource",
                            false,
                            theme
                        ),
                        iced::widget::rule::horizontal(1),
                        container(
                            row![
                                Space::new().width(Fill),
                                self.action(
                                    "关闭语音",
                                    Operation::DisableSpeech,
                                    self.speech_enabled
                                ),
                                self.action("保存并启用", Operation::Speech, true)
                            ]
                            .spacing(8)
                        )
                        .padding(12)
                    ])
                    .style(group),
                    heading("图片生成与编辑", 13),
                    container(column![
                        setting_row(
                            "服务商",
                            "",
                            pick_list(choices, selected, |v: ProviderChoice| Message::Preferences(
                                Event::Field("image_provider", v.id)
                            ))
                            .placeholder("选择服务商")
                            .text_size(13)
                            .padding([8, 10])
                            .width(256)
                            .style(picker)
                            .menu_style(picker_menu),
                            theme
                        ),
                        iced::widget::rule::horizontal(1),
                        self.form_row("模型", "", "gpt-image-2", "image_model", false, theme),
                        iced::widget::rule::horizontal(1),
                        container(row![
                            Space::new().width(Fill),
                            self.action(
                                "保存",
                                Operation::Image,
                                !self.value("image_provider").is_empty()
                            )
                        ])
                        .padding(12)
                    ])
                    .style(group)
                ]
                .spacing(12)
                .into()
            }
            Section::Security => column![
                heading("工具权限", 13),
                container(column![
                    setting_row(
                        "操作审批",
                        "自动：项目内日常操作直接执行，越界时确认。无人值守：需要确认的操作直接拒绝。",
                        pick_list(
                            vec!["自动（推荐）".to_owned(), "逐次确认".to_owned(), "无人值守".to_owned()],
                            Some(match self.value("approval") { "STRICT" => "逐次确认", "NEVER" => "无人值守", _ => "自动（推荐）" }.to_owned()),
                            |v: String| Message::Preferences(Event::Field("approval", match v.as_str() { "逐次确认" => "STRICT", "无人值守" => "NEVER", _ => "AUTO" }.into()))
                        ).text_size(13).padding([8,10]).width(256).style(picker).menu_style(picker_menu),
                        theme
                    ),
                    iced::widget::rule::horizontal(1),
                    setting_row(
                        "文件访问",
                        "命令执行没有系统沙箱，需单独授权；此设置控制内置文件工具。",
                        pick_list(
                            vec![
                                "只读".to_owned(),
                                "允许修改工作区".to_owned(),
                                "完整文件访问".to_owned()
                            ],
                            Some(
                                match self.value("sandbox") {
                                    "workspace-write" => "允许修改工作区",
                                    "danger-full-access" => "完整文件访问",
                                    _ => "只读",
                                }
                                .to_owned()
                            ),
                            |v: String| Message::Preferences(Event::Field(
                                "sandbox",
                                match v.as_str() {
                                    "允许修改工作区" => "workspace-write",
                                    "完整文件访问" => "danger-full-access",
                                    _ => "read-only",
                                }
                                .into()
                            ))
                        )
                        .text_size(13)
                        .padding([8, 10])
                        .width(256)
                        .style(picker)
                        .menu_style(picker_menu),
                        theme
                    ),
                    iced::widget::rule::horizontal(1),
                    container(row![
                        Space::new().width(Fill),
                        self.action(
                            "保存",
                            Operation::Security,
                            self.fields.get("sandbox") != self.saved.get("sandbox") || self.fields.get("approval") != self.saved.get("approval")
                        )
                    ])
                    .padding(12)
                ])
                .style(group)
            ]
            .spacing(12)
            .into(),
            Section::Data => column![
                heading("工作区", 13),
                container(column![
                    setting_row("附件大小上限", "", label("200 MB"), theme),
                    iced::widget::rule::horizontal(1),
                    setting_row(
                        "导出工作区",
                        "会话、文档、技能和定时任务；不含连接密钥。",
                        self.action("导出", Operation::Export, true),
                        theme
                    )
                ])
                .style(group),
                container(setting_row(
                    "导入会话历史",
                    "选择 potato-native-history-v1 格式 JSON；已有会话保持不变。",
                    self.action("选择历史文件…", Operation::ImportHistory, true),
                    theme
                ))
                .style(group),
                heading("导入旧版连接", 13),
                container(column![
                    self.form_row(
                        "数据目录",
                        "原目录和已有连接会保留。",
                        "旧版数据目录",
                        "working",
                        false,
                        theme
                    ),
                    iced::widget::rule::horizontal(1),
                    self.form_row("密钥目录", "", "旧版密钥目录", "secret", false, theme),
                    iced::widget::rule::horizontal(1),
                    container(row![
                        Space::new().width(Fill),
                        self.action(
                            "导入连接",
                            Operation::Import,
                            !self.value("working").is_empty() && !self.value("secret").is_empty()
                        )
                    ])
                    .padding(12)
                ])
                .style(group)
            ]
            .spacing(12)
            .into(),
            Section::Shortcuts => container(column![
                setting_row(
                    "新建会话",
                    "",
                    label(if cfg!(target_os = "macos") {
                        "⌘ N"
                    } else {
                        "Ctrl N"
                    }),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row(
                    "搜索会话",
                    "",
                    label(if cfg!(target_os = "macos") {
                        "⌘ K"
                    } else {
                        "Ctrl K"
                    }),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row(
                    "收起 / 展开侧栏",
                    "",
                    label(if cfg!(target_os = "macos") {
                        "⌘ B"
                    } else {
                        "Ctrl B"
                    }),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row(
                    "打开设置",
                    "",
                    label(if cfg!(target_os = "macos") {
                        "⌘ ,"
                    } else {
                        "Ctrl ,"
                    }),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row("发送消息", "", label("Enter"), theme),
                iced::widget::rule::horizontal(1),
                setting_row(
                    "查看键盘快捷键",
                    "",
                    label(if cfg!(target_os = "macos") {
                        "⌘ /"
                    } else {
                        "Ctrl /"
                    }),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row("输入换行", "", label("Shift+Enter"), theme),
                iced::widget::rule::horizontal(1),
                setting_row(
                    "全屏",
                    "",
                    label(if cfg!(target_os = "macos") {
                        "⌃ ⌘ F"
                    } else {
                        "F11"
                    }),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row("关闭设置", "未保存时先确认。", label("Esc"), theme)
            ])
            .style(group)
            .into(),
            Section::About => container(column![
                setting_row("Potato", "", heading(self.value("version"), 16), theme),
                setting_row("本地核心", "", label(self.value("health")), theme),
                setting_row(
                    "运行时间",
                    "截至打开设置时",
                    label(self.value("uptime")),
                    theme
                ),
                iced::widget::rule::horizontal(1),
                setting_row(
                    "版本",
                    "Rust 原生客户端预览版",
                    label(if cfg!(target_os = "macos") {
                        "macOS"
                    } else if cfg!(target_os = "windows") {
                        "Windows"
                    } else {
                        "Linux"
                    }),
                    theme
                )
            ])
            .style(group)
            .into(),
        }
    }
    pub fn overlay<'a>(
        &'a self,
        base: Element<'a, Message>,
        theme: &Theme,
        size: Size,
    ) -> Element<'a, Message> {
        let width = (size.width - 48.).clamp(640., 992.);
        let height = (size.height * 0.88).clamp(400., 704.);
        let mut nav = column![container(heading("设置", 16)).padding(iced::Padding {
            top: 12.,
            bottom: 20.,
            left: 12.,
            right: 12.
        })]
        .spacing(4);
        for section in [
            Section::Models,
            Section::General,
            Section::Capabilities,
            Section::Security,
            Section::Data,
            Section::Shortcuts,
            Section::About,
        ] {
            nav = nav.push(
                button(
                    row![section_icon(section, theme), label(section.title())]
                        .spacing(10)
                        .align_y(alignment::Vertical::Center),
                )
                .width(Fill)
                .padding([10, 12])
                .style(if section == self.section {
                    ui::selected
                } else {
                    ui::nav
                })
                .on_press_maybe((!self.busy).then_some(Message::Preferences(Event::Navigate(
                    Destination::Section(section),
                )))),
            );
        }
        let nav = container(nav)
            .width(208)
            .height(Fill)
            .padding(12)
            .style(|theme: &Theme| container::Style {
                background: Some(
                    if dark(theme) {
                        Color::from_rgb8(27, 27, 27)
                    } else {
                        Color::from_rgb8(245, 245, 244)
                    }
                    .into(),
                ),
                // Container clipping is rectangular: round the sidebar itself so
                // its background cannot cover the modal's left corners.
                border: border::rounded(border::Radius {
                    top_left: 15.,
                    bottom_left: 15.,
                    ..Default::default()
                }),
                ..Default::default()
            });
        let mut body = Column::new().spacing(16).width(Fill);
        let mut feedback = Column::new();
        if !self.notice.is_empty() {
            feedback = feedback.push(
                container(text(&self.notice).size(12).color(match self.notice_tone {
                    NoticeTone::Info => ui::muted(theme),
                    NoticeTone::Success => theme.palette().success,
                    NoticeTone::Error => theme.palette().danger,
                }))
                .padding([8, 12])
                .width(Fill)
                .style(group),
            );
        }
        if self.busy && !self.loaded {
            body = body.push(text("正在读取设置…").size(13));
        } else {
            body = body.push(self.body(theme));
        }
        if self.pending.is_some() {
            body = column![container(
                column![
                    heading("有未保存的修改", 16),
                    text("离开前要放弃这些修改吗？").size(13),
                    row![
                        button(label("继续编辑"))
                            .padding([8, 12])
                            .style(primary)
                            .on_press(Message::Preferences(Event::KeepEditing)),
                        button(label("放弃修改"))
                            .padding([8, 12])
                            .style(secondary)
                            .on_press(Message::Preferences(Event::Discard))
                    ]
                    .spacing(8)
                ]
                .spacing(16)
            )
            .padding(20)
            .style(group)];
        }
        if let Some(operation) = self.confirmation {
            let description = match operation {
                Operation::ClearKey => "清除后需要重新输入 API key 才能连接。聊天记录会保留。",
                Operation::DeleteProvider => {
                    "删除此服务商及其保存的连接和模型配置？聊天记录会保留。"
                }
                Operation::DeleteModel => "从此服务商中删除所选模型？当前使用此模型时会取消选择。",
                _ => "确认执行此操作？",
            };
            body = column![container(
                column![
                    heading("确认操作", 16),
                    text(description).size(13),
                    row![
                        button(label("取消"))
                            .style(primary)
                            .on_press(Message::Preferences(Event::CancelConfirmation)),
                        button(label("确认"))
                            .style(secondary)
                            .on_press(Message::Preferences(Event::Run(operation)))
                    ]
                    .spacing(10)
                ]
                .spacing(16)
            )
            .padding(20)
            .style(group)];
        }
        let header =
            container(
                row![
                    heading(self.section.title(), 22).width(Fill),
                    tooltip(
                        button(icon(r#"<path d="m6 6 12 12M6 18 18 6"/>"#, theme))
                            .padding(6)
                            .style(ui::nav)
                            .on_press_maybe((!self.busy).then_some(Message::Preferences(
                                Event::Navigate(Destination::Close)
                            ))),
                        label("关闭设置"),
                        tooltip::Position::Bottom
                    )
                ]
                .align_y(alignment::Vertical::Center),
            )
            .padding(iced::Padding {
                top: 28.,
                right: 28.,
                bottom: 20.,
                left: 28.,
            });
        let panel = container(row![
            nav,
            container(column![
                header,
                container(feedback).padding([0, 28]),
                scrollable(container(body).padding(iced::Padding {
                    top: 0.,
                    right: 28.,
                    bottom: 28.,
                    left: 28.
                }))
                .id(iced::widget::Id::new("settings-body"))
                .height(Fill)
            ])
            .width(Fill)
            .height(Fill)
        ])
        .width(width)
        .height(height)
        .padding(1)
        .clip(true)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.palette().background.into()),
            border: border::rounded(16).width(1).color(line(theme)),
            shadow: Shadow {
                color: Color::from_rgba(0., 0., 0., 0.18),
                offset: Vector::new(0., 12.),
                blur_radius: 40.,
            },
            ..Default::default()
        });
        stack![
            base,
            mouse_area(
                container(Space::new().width(Fill).height(Fill))
                    .width(Fill)
                    .height(Fill)
                    .style(|_| container::Style {
                        background: Some(Color::from_rgba(0., 0., 0., 0.28).into()),
                        ..Default::default()
                    })
            )
            .on_press(Message::Preferences(Event::Navigate(Destination::Close))),
            container(opaque(crate::accessibility::modal(panel.into())))
                .center_x(Fill)
                .center_y(Fill)
        ]
        .into()
    }
}
