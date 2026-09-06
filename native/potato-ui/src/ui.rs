//! Visual baseline: the installed Potato app used in the homepage comparison.
//! Its packaged CSS differs from the concurrently edited app/ sources; see VISUAL_PARITY.md.
use crate::{App, Message};
use chrono::Timelike;
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, svg, text, text_editor,
    text_input, tooltip, Column, Space,
};
use iced::{alignment, border, Color, Element, Fill, Shadow, Theme, Vector};

pub fn theme(dark: bool) -> Theme {
    Theme::custom(
        "Potato".into(),
        iced::theme::Palette {
            background: hex(if dark { 0x141414 } else { 0xfbfbfb }),
            text: hex(if dark { 0xececec } else { 0x202020 }),
            primary: hex(if dark { 0xececec } else { 0x1c1c1c }),
            success: hex(0x1c9c58),
            danger: hex(0xd64444),
            warning: hex(0xc78616),
        },
    )
}

fn hex(value: u32) -> Color {
    Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn dark(theme: &Theme) -> bool {
    theme.palette().background.r < 0.5
}
pub(super) fn muted(theme: &Theme) -> Color {
    hex(if dark(theme) { 0x787878 } else { 0x8f8f8f })
}
fn line(theme: &Theme) -> Color {
    hex(if dark(theme) { 0x464646 } else { 0xdcdcd9 })
}

pub(super) fn nav(theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            hex(if dark(theme) { 0x373737 } else { 0xededed })
        }
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(background.into()),
        text_color: theme.palette().text,
        border: border::rounded(8),
        ..Default::default()
    }
}

pub(super) fn selected(theme: &Theme, status: button::Status) -> button::Style {
    let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let shade = if dark(theme) {
        if active {
            0x3c3c3c
        } else {
            0x333333
        }
    } else if active {
        0xd3d3d1
    } else {
        0xdddddb
    };
    button::Style {
        background: Some(hex(shade).into()),
        ..nav(theme, status)
    }
}

pub(super) fn card(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(hex(if dark(theme) { 0x202020 } else { 0xffffff }).into()),
        border: iced::Border {
            color: line(theme),
            width: 1.,
            radius: 16.0.into(),
        },
        ..Default::default()
    }
}
fn send(theme: &Theme, status: button::Status) -> button::Style {
    let bg = if matches!(status, button::Status::Disabled) {
        // CSS opacity .45 composited on the white / dark composer surface.
        hex(if dark(theme) { 0x7c7c7c } else { 0x999999 })
    } else if matches!(status, button::Status::Hovered) {
        hex(if dark(theme) { 0xffffff } else { 0x333333 })
    } else {
        theme.palette().primary
    };
    button::Style {
        background: Some(bg.into()),
        text_color: theme.palette().background,
        border: border::rounded(36),
        ..Default::default()
    }
}

pub(super) fn show_top_brand(mac: bool, fullscreen: bool) -> bool {
    !mac || fullscreen
}

fn brand() -> Element<'static, Message> {
    row![
        svg(svg::Handle::from_memory(
            include_bytes!("../assets/potato.svg").as_slice()
        ))
        .width(18)
        .height(18),
        text("Potato").size(14).font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..iced::Font::DEFAULT
        })
    ]
    .spacing(8)
    .align_y(alignment::Vertical::Center)
    .into()
}

#[derive(Clone, Copy)]
enum Icon {
    New,
    Folder,
    Search,
    Clock,
    Grid,
    Book,
    Panel,
    Moon,
    Plus,
    Shield,
    Up,
    Stop,
    Chevron,
    Mic,
    Copy,
    Right,
    Refresh,
}

fn icon_image(kind: Icon, color: Color, size: f32) -> Element<'static, Message> {
    let path = match kind {
        // Lucide 0.525.0 icon paths from the original frontend (ISC).
        Icon::Copy => {
            r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#
        }
        Icon::Refresh => {
            r#"<path d="M3 12a9 9 0 0 1 15.4-6.4L21 8M21 3v5h-5M21 12a9 9 0 0 1-15.4 6.4L3 16M8 16H3v5"/>"#
        }
        Icon::Right => r#"<path d="m9 18 6-6-6-6"/>"#,

        Icon::Folder => r#"<path d="M3 7V4h7l2 3h9v13H3z"/>"#,
        Icon::New => r#"<path d="M12 3H4v17h17v-8M10 14l1-4 8-8 3 3-8 8z"/>"#,
        Icon::Search => r#"<circle cx="10" cy="10" r="7"/><path d="m15 15 6 6"/>"#,
        Icon::Clock => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l4 2"/>"#,
        Icon::Grid => {
            r#"<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>"#
        }
        Icon::Book => {
            r#"<rect x="5" y="3" width="15" height="18" rx="2"/><path d="M9 3v18M3 7h3M3 12h3M3 17h3"/>"#
        }
        Icon::Panel => r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M10 4v16"/>"#,
        Icon::Moon => r#"<path d="M20.9 13A9 9 0 0 1 11 3.1 9 9 0 1 0 20.9 13Z"/>"#,
        Icon::Plus => r#"<path d="M12 4v16M4 12h16"/>"#,
        Icon::Shield => r#"<path d="m12 3 8 3-1 9-7 6-7-6-1-9zM8 11l3 3 5-5"/>"#,
        Icon::Stop => r#"<rect x="6" y="6" width="12" height="12" rx="1" fill="currentColor"/>"#,
        Icon::Up => r#"<path d="M12 19V5m-6 6 6-6 6 6"/>"#,
        Icon::Chevron => r#"<path d="m7 10 5 5 5-5"/>"#,
        Icon::Mic => {
            r#"<rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 10v4a7 7 0 0 0 14 0v-4M12 21v-3"/>"#
        }
    };
    let bytes = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="rgb({},{},{})" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{path}</svg>"#,
        (color.r * 255.) as u8,
        (color.g * 255.) as u8,
        (color.b * 255.) as u8
    );
    svg(svg::Handle::from_memory(bytes.into_bytes()))
        .width(size)
        .height(size)
        .into()
}

pub(super) fn copy_action(value: String, theme: &Theme) -> Element<'static, Message> {
    tooltip(
        button(icon_image(Icon::Copy, muted(theme), 14.))
            .padding(6)
            .style(nav)
            .on_press(Message::Copy(value)),
        text("复制").size(12),
        tooltip::Position::Bottom,
    )
    .into()
}
pub(super) fn disclosure(open: bool, theme: &Theme) -> Element<'static, Message> {
    icon_image(
        if open { Icon::Chevron } else { Icon::Right },
        muted(theme),
        14.,
    )
}
pub(super) fn code_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(hex(if dark(theme) { 0x262626 } else { 0xf0f0ee }).into()),
        border: border::rounded(6),
        ..Default::default()
    }
}

impl App {
    fn icon(&self, kind: Icon) -> Element<'_, Message> {
        icon_image(kind, self.theme().palette().text, 16.)
    }

    fn navigation<'a>(
        &'a self,
        icon: Icon,
        label: &'a str,
        message: Message,
    ) -> Element<'a, Message> {
        button(
            row![
                icon_image(
                    icon,
                    if matches!(message, Message::NewChat)
                        && self.messages.is_empty()
                        && self.selected.is_none()
                    {
                        hex(if self.dark { 0x6b8fe6 } else { 0x3b6ef0 })
                    } else {
                        self.theme().palette().text
                    },
                    16.
                ),
                text(label)
                    .size(14)
                    .line_height(iced::widget::text::LineHeight::Absolute(20.0.into()))
            ]
            .spacing(8)
            .align_y(alignment::Vertical::Center),
        )
        .style(
            if matches!(message, Message::NewChat)
                && self.messages.is_empty()
                && self.selected.is_none()
            {
                selected
            } else {
                nav
            },
        )
        .width(Fill)
        .padding([8, 12])
        .on_press_maybe((!self.streaming && !self.busy).then_some(message))
        .into()
    }

    fn composer(&self, wide: bool) -> Element<'_, Message> {
        let editor = text_editor(&self.draft)
            .placeholder("描述任务…")
            .on_action(Message::Edit)
            .key_binding(|event| {
                if matches!(event.status, text_editor::Status::Focused { .. })
                    && event.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                    && event.modifiers.command()
                {
                    Some(text_editor::Binding::Custom(Message::Submit))
                } else {
                    text_editor::Binding::from_key_press(event)
                }
            })
            .height(if wide { 86 } else { 46 })
            .padding(iced::Padding {
                top: 16.,
                right: 20.,
                bottom: 4.,
                left: 20.,
            })
            .size(16)
            .style(|theme, status| {
                let mut style = text_editor::default(theme, status);
                style.background = Color::TRANSPARENT.into();
                style.border = border::width(0);
                style.placeholder = muted(theme);
                style
            });
        let arrow = icon_image(
            if self.streaming { Icon::Stop } else { Icon::Up },
            self.theme().palette().background,
            20.,
        );
        let controls = row![
            button(self.icon(Icon::Plus))
                .padding(8)
                .style(nav)
                .on_press(Message::Media(crate::media::Event::Pick)),
            if wide {
                button(
                    row![self.icon(Icon::Folder), text("角色").size(12)]
                        .spacing(4)
                        .align_y(alignment::Vertical::Center),
                )
                .padding([8, 4])
                .style(nav)
                .on_press(Message::Page(crate::pages::Event::Open(
                    crate::pages::Kind::Workspace,
                )))
                .into()
            } else {
                Element::from(Space::new().width(0))
            },
            button(
                row![
                    self.icon(Icon::Shield),
                    text(match self.approval.as_deref() {
                        Some("STRICT") => "逐次确认",
                        Some("AUTO") => "自动",
                        Some("SMART") => "智能确认",
                        Some("OFF") => "审批已关闭",
                        _ => "权限",
                    })
                    .size(12),
                    self.icon(Icon::Chevron)
                ]
                .spacing(4)
                .align_y(alignment::Vertical::Center)
            )
            .padding([8, 4])
            .style(nav)
            .on_press(Message::Unavailable("权限设置")),
            Space::new().width(Fill),
            button(
                row![
                    text(if self.backend.is_some() {
                        self.model.as_deref().unwrap_or("未配置模型")
                    } else {
                        "未就绪"
                    })
                    .size(12),
                    self.icon(Icon::Chevron)
                ]
                .spacing(2)
                .align_y(alignment::Vertical::Center)
            )
            .padding([8, 4])
            .style(nav)
            .on_press(Message::Settings),
            button(self.icon(Icon::Mic))
                .padding(8)
                .style(nav)
                .on_press(Message::Voice(crate::voice::Event::Toggle)),
            button(arrow)
                .padding(8)
                .style(send)
                .on_press_maybe(if self.streaming {
                    (self.accepted && !self.stop_pending).then_some(Message::Stop)
                } else {
                    (self.can_send()
                        && (!self.draft.text().trim().is_empty() || !self.attachments.is_empty()))
                    .then_some(Message::Submit)
                }),
        ]
        .spacing(4)
        .align_y(alignment::Vertical::Center);
        container(column![
            if self.editing_backup.is_some() {
                Element::from(
                    row![
                        text("编辑后重发 · 原消息保留").size(12),
                        button("取消").style(nav).on_press_maybe(
                            (!self.streaming && !self.busy).then_some(Message::CancelMessageEdit)
                        )
                    ]
                    .spacing(8)
                    .padding([4, 16]),
                )
            } else {
                Element::from(Space::new().height(0))
            },
            editor,
            if self.attachments.is_empty() {
                Element::from(Space::new().height(0))
            } else {
                let mut attachments = Column::new().spacing(4);
                for (index, attachment) in self.attachments.iter().enumerate() {
                    attachments = attachments.push(
                        row![
                            text(attachment["file_name"].as_str().unwrap_or("附件")).size(12),
                            button("×").style(nav).on_press_maybe(
                                (!self.streaming && !self.busy)
                                    .then_some(Message::Media(crate::media::Event::Remove(index)))
                            )
                        ]
                        .spacing(8),
                    );
                }
                Element::from(
                    container(
                        attachments.push(
                            button("清除附件").style(nav).on_press_maybe(
                                (!self.streaming && !self.busy)
                                    .then_some(Message::Media(crate::media::Event::Clear)),
                            ),
                        ),
                    )
                    .padding([4, 16]),
                )
            },
            container(controls).padding(iced::Padding {
                top: 0.,
                right: 12.,
                bottom: 12.,
                left: 12.
            })
        ])
        .width(Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(hex(if dark(theme) { 0x202020 } else { 0xffffff }).into()),
            border: border::rounded(20).width(1).color(line(theme)),
            shadow: Shadow {
                color: Color::from_rgba(
                    17. / 255.,
                    17. / 255.,
                    17. / 255.,
                    if dark(theme) { 0. } else { 0.10 },
                ),
                offset: Vector::new(0., 6.),
                blur_radius: 18.,
            },
            ..Default::default()
        })
        .into()
    }

    pub(super) fn view(&self) -> Element<'_, Message> {
        let mut sessions = Column::new().spacing(4);
        for chat in self.chats.iter() {
            sessions = sessions.push(row![
                button(
                    text(if chat.status == "running" {
                        format!("{} · 运行中", chat.name)
                    } else {
                        format!("{}{}", if chat.pinned { "★ " } else { "" }, chat.name)
                    })
                    .size(13),
                )
                .padding([8, 12])
                .width(Fill)
                .style(if self.selected.as_deref() == Some(&chat.id) {
                    selected
                } else {
                    nav
                })
                .on_press_maybe(
                    (!self.streaming && !self.busy).then_some(Message::Select(chat.id.clone())),
                ),
                button("⋯").style(nav).on_press(Message::Conversation(
                    crate::conversations::Event::Menu(chat.id.clone())
                ))
            ]);
        }
        let collapse = button(self.icon(Icon::Panel))
            .style(nav)
            .padding(8)
            .on_press(Message::Collapse);
        let reserve_lights = !show_top_brand(cfg!(target_os = "macos"), self.fullscreen);
        let sidebar: Element<'_, Message> = if self.collapsed {
            Space::new().width(0).into()
        } else {
            let title_content: Element<'_, Message> = if reserve_lights {
                Space::new().width(Fill).into()
            } else {
                container(brand()).width(Fill).padding([0, 4]).into()
            };
            let titlebar = row![
                mouse_area(container(title_content).height(44).center_y(44))
                    .on_press(Message::DragWindow),
                collapse
            ]
            .align_y(alignment::Vertical::Center);
            let mut navigation = column![
                container(titlebar).height(44),
                self.navigation(Icon::New, "新建", Message::NewChat),
                self.navigation(Icon::Search, "搜索", Message::Search),
                self.navigation(
                    Icon::Clock,
                    "定时",
                    Message::Page(crate::pages::Event::Open(crate::pages::Kind::Tasks))
                ),
                self.navigation(
                    Icon::Grid,
                    "技能",
                    Message::Page(crate::pages::Event::Open(crate::pages::Kind::Skills))
                ),
                self.navigation(
                    Icon::Book,
                    "记忆",
                    Message::Page(crate::pages::Event::Open(crate::pages::Kind::Memory))
                ),
            ]
            .spacing(2);
            if self.search {
                navigation = navigation.push(
                    text_input("搜索会话", &self.filter)
                        .on_input(Message::Filter)
                        .padding(10),
                );
            }
            container(column![
                container(navigation).padding([0, 12]),
                container(
                    row![
                        button(
                            text(if self.conversations.archived {
                                "已归档 · 返回会话"
                            } else {
                                "会话 · 查看归档"
                            })
                            .size(11)
                        )
                        .style(nav)
                        .on_press(Message::Conversation(
                            crate::conversations::Event::ToggleArchived
                        )),
                        Space::new().width(Fill),
                        text(self.chats.len()).size(11),
                        self.icon(Icon::Chevron)
                    ]
                    .spacing(4)
                )
                .padding(iced::Padding {
                    top: 20.,
                    right: 24.,
                    bottom: 8.,
                    left: 24.
                }),
                container(scrollable(sessions).height(Fill)).padding([0, 12]),
                container(row![
                    button(brand())
                        .style(nav)
                        .padding(8)
                        .on_press(Message::Settings),
                    Space::new().width(Fill),
                    button(self.icon(Icon::Moon))
                        .style(nav)
                        .padding(8)
                        .on_press(Message::Theme)
                ])
                .padding(12),
            ])
            .width(264)
            .height(Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(hex(if dark(theme) { 0x1c1c1c } else { 0xf0f0ee }).into()),
                ..Default::default()
            })
            .into()
        };

        if self.pages.kind.is_some() {
            let base = row![
                sidebar,
                container(self.pages.view()).width(Fill).height(Fill)
            ]
            .height(Fill)
            .into();
            return if self.settings {
                self.preferences
                    .overlay(base, &self.theme(), self.window_size)
            } else {
                base
            };
        }
        let empty = self.messages.is_empty() && self.selected.is_none();
        let mut main = Column::new().height(Fill).spacing(0);
        if self.conversations.menu.is_some() {
            main = main.push(self.conversations.view());
        }
        if empty {
            let hour = chrono::Local::now().hour();
            let greeting = if (5..12).contains(&hour) {
                "早上好，从哪件事开始？"
            } else if (12..18).contains(&hour) {
                "下午好，从哪件事开始？"
            } else {
                "晚上好，从哪件事开始？"
            };
            let heading = text(greeting)
                .size(34)
                .line_height(iced::widget::text::LineHeight::Absolute(42.0.into()))
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::with_name(if cfg!(target_os = "windows") {
                        "Microsoft YaHei"
                    } else {
                        "PingFang SC"
                    })
                });
            let welcome = column![container(heading).center_x(Fill), self.composer(true)]
                .spacing(76)
                .max_width(768);
            main = main.push(
                container(column![
                    Space::new().height(Fill),
                    container(welcome).center_x(Fill).padding([0, 24]),
                    Space::new().height(Fill)
                ])
                .height(Fill)
                .padding(iced::Padding {
                    top: 0.,
                    right: 0.,
                    bottom: self.window_size.height * 0.16 + 24.,
                    left: 0.,
                }),
            );
        } else {
            main = main.push(
                container(row![
                    Space::new().width(Fill),
                    button(self.icon(Icon::Search))
                        .style(nav)
                        .padding(8)
                        .on_press(Message::Search),
                    button(self.icon(Icon::Panel))
                        .style(nav)
                        .padding(8)
                        .on_press(Message::Unavailable("改动与产物"))
                ])
                .padding([4, 12]),
            );
            let mut messages = Column::new().spacing(28).width(Fill);
            for (index, message) in self.messages.iter().enumerate() {
                let role = &message.role;
                let body = &message.body;
                let prose = text(body)
                    .size(16)
                    .shaping(iced::widget::text::Shaping::Advanced);
                if role == "user" {
                    let actions: Element<'_, Message> = if self.hovered_message == Some(index) {
                        row![
                            copy_action(message.body.clone(), &self.theme()),
                            tooltip(
                                button(icon_image(Icon::New, muted(&self.theme()), 14.))
                                    .padding(6)
                                    .style(nav)
                                    .on_press_maybe(
                                        (self.can_send()
                                            && self.editing_backup.is_none()
                                            && message.raw.is_some())
                                        .then_some(Message::ReuseTurn(index, false))
                                    ),
                                text("编辑重发").size(12),
                                tooltip::Position::Bottom
                            )
                        ]
                        .spacing(2)
                        .into()
                    } else {
                        Space::new().height(28).into()
                    };
                    messages = messages.push(crate::hover::answer(
                        column![
                            container(
                                container(column![prose, message.images_view()])
                                    .padding([10, 16])
                                    .max_width(493)
                                    .style(|theme: &Theme| container::Style {
                                        background: Some(
                                            hex(if dark(theme) { 0x262626 } else { 0xececec })
                                                .into()
                                        ),
                                        border: border::rounded(12),
                                        ..Default::default()
                                    })
                            )
                            .align_right(Fill),
                            container(actions).height(28).align_right(Fill)
                        ]
                        .spacing(4),
                        index,
                    ));
                } else {
                    messages = messages.push(crate::hover::answer(
                        column![
                            message.view(&self.theme(), index, &self.expanded_tools),
                            if !message.reasoning
                                && message.role == "assistant"
                                && !message.body.is_empty()
                                && !(self.streaming && index + 1 == self.messages.len())
                            {
                                if self.hovered_message == Some(index) {
                                    container(
                                        row![
                                            copy_action(message.body.clone(), &self.theme()),
                                            if index + 1 == self.messages.len() {
                                                Element::from(tooltip(
                                                    button(icon_image(
                                                        Icon::Refresh,
                                                        muted(&self.theme()),
                                                        14.,
                                                    ))
                                                    .padding(6)
                                                    .style(nav)
                                                    .on_press_maybe(
                                                        (self.can_send()
                                                            && self.editing_backup.is_none())
                                                        .then_some(Message::ReuseTurn(index, true)),
                                                    ),
                                                    text("重新生成").size(12),
                                                    tooltip::Position::Bottom,
                                                ))
                                            } else {
                                                Element::from(Space::new().width(0))
                                            }
                                        ]
                                        .spacing(8),
                                    )
                                    .height(28)
                                    .into()
                                } else {
                                    Element::from(Space::new().height(28))
                                }
                            } else {
                                Element::from(Space::new().height(0))
                            }
                        ]
                        .spacing(4)
                        .width(Fill),
                        index,
                    ));
                }
            }
            if self.messages.is_empty() {
                messages = messages.push(
                    text(if self.busy {
                        "正在加载…"
                    } else {
                        "从左侧选择会话，查看已有消息。"
                    })
                    .size(14),
                );
            }
            main = main.push(
                scrollable(
                    container(messages.max_width(704))
                        .center_x(Fill)
                        .padding([20, 32]),
                )
                .id(iced::widget::Id::new("messages"))
                .on_scroll(|viewport| {
                    Message::Scrolled(
                        !viewport.relative_offset().y.is_finite()
                            || viewport.relative_offset().y >= 0.98,
                    )
                })
                .height(Fill),
            );
            {
                main = main.push(
                    container(self.interaction_view())
                        .center_x(Fill)
                        .padding([0, 24]),
                );
                main = main.push(
                    container(container(self.composer(false)).max_width(768))
                        .center_x(Fill)
                        .padding(iced::Padding {
                            top: 12.,
                            right: 24.,
                            bottom: 24.,
                            left: 24.,
                        }),
                );
            }
        }
        if self.backend.is_some() && self.uncertain && !self.streaming {
            main = main.push(
                container(
                    row![
                        button("刷新会话")
                            .style(nav)
                            .padding([4, 8])
                            .on_press_maybe(
                                (!self.streaming && !self.busy).then_some(Message::Refresh)
                            ),
                        button("重新连接输出")
                            .style(nav)
                            .padding([4, 8])
                            .on_press_maybe(
                                (!self.streaming && !self.busy && self.uncertain)
                                    .then_some(Message::Reconnect)
                            ),
                        button(if self.stop_pending {
                            "等待停止确认"
                        } else {
                            "停止任务"
                        })
                        .style(nav)
                        .padding([4, 8])
                        .on_press_maybe(
                            ((self.accepted || self.uncertain) && !self.stop_pending)
                                .then_some(Message::Stop)
                        ),
                    ]
                    .spacing(8),
                )
                .center_x(Fill),
            );
        }
        // Demo remains explicit in the model selector. Show only actionable status here;
        // a permanent footer would shift the homepage compared with the original.
        if (!self.status.is_empty() && self.status != "本地应用尚未就绪") {
            main = main.push(
                container(text(&self.status).size(11).color(muted(&self.theme())))
                    .center_x(Fill)
                    .padding([8, 24]),
            );
        }
        let base: Element<'_, Message> = if self.collapsed {
            let actions = row![
                button(self.icon(Icon::Panel))
                    .style(nav)
                    .padding(8)
                    .on_press(Message::Collapse),
                button(self.icon(Icon::New))
                    .style(nav)
                    .padding(8)
                    .on_press(Message::NewChat),
                mouse_area(Space::new().width(Fill).height(44)).on_press(Message::DragWindow)
            ]
            .spacing(2)
            .align_y(alignment::Vertical::Center);
            stack![
                container(main).width(Fill).height(Fill),
                container(actions).height(44).padding(iced::Padding {
                    left: if reserve_lights { 92. } else { 12. },
                    right: 8.,
                    top: 0.,
                    bottom: 0.
                })
            ]
            .into()
        } else {
            row![sidebar, container(main).width(Fill).height(Fill)].into()
        };
        if self.settings {
            self.preferences
                .overlay(base, &self.theme(), self.window_size)
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::show_top_brand;
    #[test]
    fn brand_yields_to_macos_traffic_lights_only_in_windowed_mode() {
        assert!(!show_top_brand(true, false));
        assert!(show_top_brand(true, true));
        assert!(show_top_brand(false, false));
        assert!(show_top_brand(false, true));
    }
}
