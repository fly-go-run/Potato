use crate::side_panel_data::{self as data, FileEntry, GitFiles, Preview};
use crate::view::{icon_button, muted};
use crate::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    text::TextView,
};
use gpui_kit::prelude::*;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
pub enum FileMode {
    Preview,
    Conversation,
    Git,
}
#[derive(Default)]
pub struct SidePanel {
    pub open: bool,
    context: String,
    project: PathBuf,
    generation: u64,
    git_generation: u64,
    pub selected: Option<(FileEntry, FileMode)>,
    preview: Option<Result<Preview, String>>,
    image: Option<Arc<Image>>,
    preview_dark: Option<bool>,
    git: Option<Result<GitFiles, String>>,
    git_loading: bool,
    history_count: usize,
    fingerprint: u64,
    conversation: data::ConversationFiles,
    list_scroll: ScrollHandle,
    detail_scroll: ScrollHandle,
    pub locate: Option<usize>,
    pub resizing: bool,
}
impl Potato {
    pub fn sync_side_panel(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let project = PathBuf::from(self.project["path"].as_str().unwrap_or(""));
        if self.files.context != self.session || self.files.project != project {
            let open = self.files.open;
            let generation = self.files.generation + 1;
            let git_generation = self.files.git_generation + 1;
            self.files = SidePanel {
                open,
                context: self.session.clone(),
                project,
                generation,
                git_generation,
                ..Default::default()
            };
        }
        if self.files.open
            && self.files.preview_dark != Some(self.dark)
            && let Some((entry, FileMode::Preview)) = self.files.selected.clone()
        {
            self.open_panel_file(entry, FileMode::Preview, w, cx);
        }
        if self.files.open {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.history.hash(&mut hasher);
            self.streaming.hash(&mut hasher);
            self.turn.messages.len().hash(&mut hasher);
            for message in &self.turn.messages {
                if !self.streaming
                    || !(message["type"] == "reasoning"
                        || (message["type"] == "message" && message["role"] == "assistant"))
                {
                    message.hash(&mut hasher);
                }
            }
            let fingerprint = hasher.finish();
            if self.files.fingerprint != fingerprint {
                let all: Vec<_> = self
                    .history
                    .iter()
                    .cloned()
                    .chain(self.turn.messages.iter().map(|message| {
                        if self.streaming
                            && (message["type"] == "reasoning"
                                || (message["type"] == "message" && message["role"] == "assistant"))
                        {
                            Value::Null // preserve message indices without re-parsing partial links every token
                        } else {
                            message.clone()
                        }
                    }))
                    .collect();
                self.files.conversation = data::collect(&all, &self.files.project);
                self.files.fingerprint = fingerprint;
            }
        }
        if self.files.open
            && !self.files.git_loading
            && (self.files.git.is_none()
                || (!self.streaming && self.files.history_count != self.history.len()))
        {
            self.refresh_file_git(w, cx);
        }
    }
    pub fn toggle_files(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.files.open = !self.files.open;
        if self.files.open {
            self.refresh_file_git(w, cx);
        }
        cx.notify();
    }
    fn refresh_file_git(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.files.git_generation += 1;
        let generation = self.files.git_generation;
        let session = self.session.clone();
        let project = PathBuf::from(self.project["path"].as_str().unwrap_or(""));
        self.files.history_count = self.history.len();
        self.files.git_loading = true;
        cx.spawn_in(w, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { data::git_files(&project) })
                .await;
            let _ = this.update_in(cx, |s, _, cx| {
                if s.session == session && s.files.git_generation == generation {
                    s.files.git = Some(result);
                    s.files.git_loading = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub fn open_panel_file(
        &mut self,
        entry: FileEntry,
        mode: FileMode,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.files.open = true;
        self.files.generation += 1;
        let generation = self.files.generation;
        let session = self.session.clone();
        let root = self
            .files
            .git
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|g| g.root.clone());
        self.files.selected = Some((entry.clone(), mode));
        self.files.detail_scroll = ScrollHandle::new();
        self.files.preview = None;
        self.files.image = None;
        let dark = self.dark;
        self.files.preview_dark = Some(dark);
        cx.spawn_in(w, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match mode {
                        FileMode::Preview => data::read_preview(&entry.path, dark),
                        FileMode::Conversation => Ok(data::edit_preview(&entry)),
                        FileMode::Git => root
                            .ok_or_else(|| "请刷新工作区改动".to_owned())
                            .and_then(|root| data::read_diff(&root, &entry)),
                    }
                })
                .await;
            let _ = this.update_in(cx, |s, _, cx| {
                if s.session == session && s.files.generation == generation {
                    s.files.image = match &result {
                        Ok(Preview::Image { bytes, extension }) => {
                            let format = match extension.as_str() {
                                "jpg" | "jpeg" => ImageFormat::Jpeg,
                                "webp" => ImageFormat::Webp,
                                "gif" => ImageFormat::Gif,
                                _ => ImageFormat::Png,
                            };
                            Some(Arc::new(Image::from_bytes(format, bytes.clone())))
                        }
                        _ => None,
                    };
                    s.files.preview = Some(result);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    pub fn follow_file_link(&mut self, url: &str, w: &mut Window, cx: &mut Context<Self>) {
        let lower = url.to_ascii_lowercase();
        if lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:")
        {
            cx.open_url(url);
            return;
        }
        let project = Path::new(self.project["path"].as_str().unwrap_or(""));
        if let Some(path) = data::resolve_link(url, project) {
            self.open_panel_file(
                FileEntry {
                    path,
                    ..Default::default()
                },
                FileMode::Preview,
                w,
                cx,
            );
        }
    }
    fn side_width(&self, w: &Window) -> f32 {
        let detail = self.files.selected.is_some();
        let saved = self.preferences[if detail {
            "file_detail_width"
        } else {
            "file_list_width"
        }]
        .as_f64()
        .unwrap_or(if detail { 520. } else { 304. }) as f32;
        let available = f32::from(w.viewport_size().width) - if self.sidebar { 260. } else { 0. };
        saved.clamp(240., (available - 320.).clamp(240., 720.))
    }
    pub fn resize_files(&mut self, event: &MouseMoveEvent, w: &mut Window, cx: &mut Context<Self>) {
        if !self.files.resizing {
            return;
        }
        let width = f32::from(w.viewport_size().width - event.position.x);
        self.preferences[if self.files.selected.is_some() {
            "file_detail_width"
        } else {
            "file_list_width"
        }] = json!(width.clamp(240., 720.));
        cx.notify();
    }
    pub fn finish_file_resize(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.files.resizing {
            return;
        }
        self.files.resizing = false;
        self.request(
            "PUT",
            "/api/native/preferences",
            self.preferences.clone(),
            w,
            cx,
            |_, _, _, _| {},
        );
        cx.notify();
    }
    fn open_file_system(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(w, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { open::that(path).map_err(|e| format!("无法打开文件：{e}")) })
                .await;
            if let Err(error) = result {
                let _ = this.update_in(cx, |s, _, cx| {
                    s.notice = error;
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub fn file_side_view(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.files.selected.clone();
        let mut panel = div()
            .id("file-panel")
            .debug_selector(|| "file-panel-bounds".into())
            .relative()
            .w(px(self.side_width(w)))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .overflow_hidden();
        let mut header = div()
            .h(px(44.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .border_color(cx.theme().border);
        if let Some((entry, mode)) = selected {
            let path = entry.path.clone();
            let reveal = path.clone();
            let open = path.clone();
            let retry = entry.clone();
            header = header
                .child(
                    icon_button("files-back", IconName::ChevronLeft, "返回文件列表").on_click(
                        cx.listener(|s, _, _, cx| {
                            s.files.selected = None;
                            s.files.preview = None;
                            s.files.image = None;
                            s.files.generation += 1;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    div().flex_1().min_w_0().overflow_hidden().child(
                        div()
                            .text_size(px(13.))
                            .text_ellipsis()
                            .child(file_name(&path)),
                    ),
                )
                .child(
                    icon_button("files-reveal", IconName::FolderOpen, "在文件管理器中显示")
                        .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
                )
                .child(
                    icon_button("files-open", IconName::ArrowUp, "使用系统应用打开").on_click(
                        cx.listener(move |s, _, w, cx| s.open_file_system(open.clone(), w, cx)),
                    ),
                )
                .child(
                    icon_button("files-reload", IconName::RefreshCw, "重新读取文件").on_click(
                        cx.listener(move |s, _, w, cx| {
                            s.open_panel_file(retry.clone(), mode, w, cx)
                        }),
                    ),
                );
            panel = panel.child(header.child(close_button(cx))).child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(muted(path.to_string_lossy().into_owned(), cx))
                    .child(muted(
                        match mode {
                            FileMode::Conversation => "会话编辑记录 · 工具执行时的片段",
                            FileMode::Git => "工作区当前改动 · 包含其他来源的修改",
                            FileMode::Preview => "文件当前内容",
                        },
                        cx,
                    )),
            );
            let mut body = div()
                .id("file-detail-scroll")
                .track_scroll(&self.files.detail_scroll)
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .overflow_x_scroll()
                .p_3();
            body = match &self.files.preview {
                None => body.child(muted("正在读取…", cx)),
                Some(Err(error)) => body.child(muted(error.clone(), cx)),
                Some(Ok(preview)) => body.child(self.preview_body(preview, &path, cx)),
            };
            panel = panel.child(body);
            if mode != FileMode::Preview {
                let preview = entry.clone();
                panel = panel.child(
                    Button::new("files-preview-current")
                        .ghost()
                        .label("查看文件当前内容")
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.open_panel_file(preview.clone(), FileMode::Preview, w, cx)
                        })),
                );
            }
        } else {
            header = header
                .child(
                    div()
                        .flex_1()
                        .px_1()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("文件与改动"),
                )
                .child(
                    icon_button("files-refresh", IconName::RefreshCw, "刷新工作区改动")
                        .on_click(cx.listener(|s, _, w, cx| s.refresh_file_git(w, cx))),
                )
                .child(close_button(cx));
            let files = &self.files.conversation;
            let mut list = div()
                .id("file-list-scroll")
                .track_scroll(&self.files.list_scroll)
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .gap_1();
            if let Some(approvals) = self.approval_history_side_view(cx) {
                list = list.child(approvals);
            }
            list = list.child(section_title("会话编辑记录", files.changes.len(), cx));
            if !files.changes.is_empty() {
                list = list.child(muted("± 为累计编辑片段行数，非净改动", cx).px_2().pb_1());
            }
            if files.changes.is_empty() {
                list = list.child(muted("本会话还没有成功的文件编辑", cx).px_2().py_2());
            }
            for (i, entry) in files.changes.iter().enumerate() {
                list = list.child(self.file_row(
                    format!("edit-{i}"),
                    entry,
                    FileMode::Conversation,
                    cx,
                ));
            }
            list = list.child(section_title("交付文件", files.artifacts.len(), cx));
            if files.artifacts.is_empty() {
                list = list.child(muted("回复中交付的文件会显示在这里", cx).px_2().py_2());
            }
            for (i, entry) in files.artifacts.iter().enumerate() {
                list = list.child(self.file_row(
                    format!("artifact-{i}"),
                    entry,
                    FileMode::Preview,
                    cx,
                ));
            }
            list = list.child(section_title(
                "工作区当前改动",
                self.files
                    .git
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .map_or(0, |g| g.entries.len()),
                cx,
            ));
            if self.files.git_loading {
                list = list.child(muted("正在刷新…", cx).px_2());
            }
            match &self.files.git {
                Some(Ok(git)) => {
                    list = list.child(
                        muted(format!("{} · 包含所有来源的修改", git.branch), cx)
                            .px_2()
                            .pb_2(),
                    );
                    if git.entries.is_empty() {
                        list = list.child(muted("工作区没有未提交改动", cx).px_2().py_2());
                    }
                    for (i, entry) in git.entries.iter().take(500).enumerate() {
                        list =
                            list.child(self.file_row(format!("git-{i}"), entry, FileMode::Git, cx));
                    }
                    if git.entries.len() > 500 {
                        list = list.child(muted("仅显示前 500 个文件", cx));
                    }
                }
                Some(Err(error)) => list = list.child(muted(error.clone(), cx).px_2().py_2()),
                None => {}
            }
            panel = panel.child(header).child(list);
        }
        panel
            .child(
                div()
                    .id("file-panel-resize")
                    .debug_selector(|| "file-panel-handle".into())
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(5.))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|d| d.bg(cx.theme().border))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, _, _, cx| {
                            s.files.resizing = true;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .into_any_element()
    }
    fn file_row(
        &self,
        id: String,
        entry: &FileEntry,
        mode: FileMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = entry.clone();
        let source = entry.source;
        let parent = entry.path.parent().unwrap_or(Path::new(""));
        let parent = parent
            .strip_prefix(&self.files.project)
            .unwrap_or(parent)
            .to_string_lossy()
            .into_owned();
        let meta = if mode == FileMode::Git {
            format!(
                "{}{}{}",
                if entry.untracked { "未跟踪 " } else { "" },
                if entry.staged { "已暂存 " } else { "" },
                if entry.unstaged && !entry.untracked {
                    "未暂存"
                } else {
                    ""
                }
            )
        } else {
            parent
        };
        let mut row =
            div().flex().items_center().gap_1().child(
                Button::new(ElementId::Name(id.clone().into()))
                    .ghost()
                    .h_auto()
                    .py_2()
                    .px_2()
                    .flex_1()
                    .min_w_0()
                    .justify_start()
                    .tooltip(entry.path.to_string_lossy().into_owned())
                    .accessibility_label(format!(
                        "{} {}",
                        match mode {
                            FileMode::Preview => "预览文件",
                            FileMode::Conversation => "查看会话编辑",
                            FileMode::Git => "查看工作区差异",
                        },
                        entry.path.display()
                    ))
                    .child(Icon::new(IconName::FileText).size(px(15.)).flex_shrink_0())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_ellipsis()
                                    .child(file_name(&entry.path)),
                            )
                            .child(muted(meta, cx).text_ellipsis()),
                    )
                    .when(
                        mode != FileMode::Preview && !entry.untracked && !entry.binary,
                        |b| {
                            b.child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .text_size(px(11.))
                                    .child(
                                        div()
                                            .text_color(rgb(0x32835b))
                                            .child(format!("+{}", entry.additions)),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.theme().danger)
                                            .child(format!("−{}", entry.deletions)),
                                    ),
                            )
                        },
                    )
                    .when(entry.binary, |b| b.child(muted("二进制", cx)))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.open_panel_file(selected.clone(), mode, w, cx)
                    })),
            );
        if mode != FileMode::Git {
            row = row.child(
                Button::new(ElementId::Name(format!("{id}-locate").into()))
                    .ghost()
                    .small()
                    .icon(IconName::ListEnd)
                    .tooltip("定位来源消息")
                    .accessibility_label("定位来源消息")
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.files.locate = Some(source);
                        s.chat.scroll_paused = true;
                        cx.notify();
                    })),
            );
        }
        row.into_any_element()
    }
    fn preview_body(&self, preview: &Preview, path: &Path, cx: &mut Context<Self>) -> AnyElement {
        match preview {
            Preview::External(message) => div()
                .p_3()
                .child(muted(message.clone(), cx))
                .into_any_element(),
            Preview::Image { .. } => div()
                .w_full()
                .children(
                    self.files
                        .image
                        .as_ref()
                        .map(|image| img(image.clone()).w_full().object_fit(ObjectFit::Contain)),
                )
                .into_any_element(),
            Preview::Text {
                text,
                markdown: true,
                ..
            } => {
                let entity = cx.entity().downgrade();
                let parent = path.parent().unwrap_or(Path::new("")).to_owned();
                TextView::markdown("file-markdown", text.clone())
                    .selectable(true)
                    .on_link_click(move |url, _, w, cx| {
                        let _ = entity.update(cx, |s, cx| {
                            if url.starts_with("http://") || url.starts_with("https://") {
                                cx.open_url(url);
                            } else if let Some(path) = data::resolve_link(url, &parent) {
                                s.open_panel_file(
                                    FileEntry {
                                        path,
                                        ..Default::default()
                                    },
                                    FileMode::Preview,
                                    w,
                                    cx,
                                );
                            }
                        });
                    })
                    .into_any_element()
            }
            Preview::Text {
                lines,
                truncated,
                text,
                ..
            } => {
                let copy = text.clone();
                let mut body = div().flex().flex_col().gap_1().child(
                    Button::new("copy-preview")
                        .ghost()
                        .small()
                        .label("复制内容")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        }),
                );
                let mut code = div()
                    .font_family("monospace")
                    .text_size(px(12.))
                    .line_height(px(20.))
                    .min_w_full();
                for (i, line) in lines.iter().enumerate() {
                    code = code.child(
                        div()
                            .flex()
                            .gap_3()
                            .whitespace_nowrap()
                            .child(
                                muted((i + 1).to_string(), cx)
                                    .w(px(40.))
                                    .text_right()
                                    .flex_shrink_0(),
                            )
                            .child(
                                StyledText::new(if line.text.is_empty() {
                                    " ".into()
                                } else {
                                    line.text.clone()
                                })
                                .with_highlights(
                                    line.highlights.iter().map(|(range, color)| {
                                        (
                                            range.clone(),
                                            HighlightStyle {
                                                color: Some(rgb(*color).into()),
                                                ..Default::default()
                                            },
                                        )
                                    }),
                                ),
                            ),
                    );
                }
                body = body.child(code);
                if *truncated {
                    body = body.child(muted(
                        "预览已截断（最多 1500 行，每行 4000 字符）；系统打开可查看全文",
                        cx,
                    ));
                }
                body.into_any_element()
            }
            Preview::Diff(sections) => {
                let mut body = div().flex().flex_col().gap_3();
                for section in sections {
                    let mut block = div()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_md()
                        .child(
                            muted(section.label.clone(), cx)
                                .px_2()
                                .py_2()
                                .bg(cx.theme().muted),
                        );
                    if section.lines.is_empty() {
                        block = block.child(
                            muted("当前没有差异，文件可能已提交或还原，请返回刷新列表", cx).p_2(),
                        );
                    }
                    for line in &section.lines {
                        let color = match line.kind {
                            '+' => {
                                if self.dark {
                                    rgb(0x20372b)
                                } else {
                                    rgb(0xeaf5ed)
                                }
                            }
                            '-' => {
                                if self.dark {
                                    rgb(0x422829)
                                } else {
                                    rgb(0xffeeee)
                                }
                            }
                            _ => {
                                if self.dark {
                                    rgb(0x202020)
                                } else {
                                    rgb(0xffffff)
                                }
                            }
                        };
                        block = block.child(
                            div()
                                .flex()
                                .gap_2()
                                .px_2()
                                .bg(color)
                                .font_family("monospace")
                                .text_size(px(12.))
                                .line_height(px(20.))
                                .whitespace_nowrap()
                                .child(
                                    muted(line.old.map(|n| n.to_string()).unwrap_or_default(), cx)
                                        .w(px(32.))
                                        .text_right()
                                        .flex_shrink_0(),
                                )
                                .child(
                                    muted(line.new.map(|n| n.to_string()).unwrap_or_default(), cx)
                                        .w(px(32.))
                                        .text_right()
                                        .flex_shrink_0(),
                                )
                                .child(div().w(px(10.)).flex_shrink_0().child(
                                    if matches!(line.kind, '+' | '-') {
                                        line.kind.to_string()
                                    } else {
                                        " ".into()
                                    },
                                ))
                                .child(line.text.clone()),
                        );
                    }
                    body = body.child(block);
                }
                body.into_any_element()
            }
        }
    }
}
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}
fn section_title(title: &str, count: usize, cx: &App) -> Div {
    muted(format!("{title}  {count}"), cx)
        .font_weight(FontWeight::MEDIUM)
        .px_2()
        .pt_3()
        .pb_1()
}
fn close_button(cx: &mut Context<Potato>) -> Button {
    icon_button("files-close", IconName::X, "关闭右侧栏").on_click(cx.listener(|s, _, _, cx| {
        s.files.open = false;
        cx.notify();
    }))
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, FileMode};
    use crate::{Backend, Potato};
    use gpui_kit::{TestAppContext, gpui};
    use serde_json::json;
    use std::path::PathBuf;
    #[gpui::test]
    fn switching_conversations_invalidates_pending_preview_and_keeps_panel_open(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let backend = Backend::for_ui_test(
            std::env::temp_dir().join(format!("potato-file-state-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let (app, cx) = cx.add_window_view(|w, cx| Potato::new(backend, w, cx));
        app.update_in(cx, |s, w, cx| {
            s.sync_side_panel(w, cx);
            s.files.selected = Some((
                FileEntry {
                    path: PathBuf::from("/tmp/previous.txt"),
                    ..Default::default()
                },
                FileMode::Preview,
            ));
            s.files.open = true;
            let generation = s.files.generation;
            s.session = "another-session".into();
            s.sync_side_panel(w, cx);
            assert!(s.files.open);
            assert!(s.files.selected.is_none());
            assert!(s.files.preview.is_none());
            assert!(s.files.generation > generation);
        });
    }
    #[gpui::test]
    fn changing_project_clears_old_git_and_file_selection(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let backend = Backend::for_ui_test(
            std::env::temp_dir().join(format!("potato-file-project-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let (app, cx) = cx.add_window_view(|w, cx| Potato::new(backend, w, cx));
        app.update_in(cx, |s, w, cx| {
            s.sync_side_panel(w, cx);
            s.files.git = Some(Err("previous".into()));
            s.project = json!({"path":"/tmp/another-project"});
            s.sync_side_panel(w, cx);
            assert!(s.files.git.is_none());
            assert!(s.files.selected.is_none());
        });
    }
    struct FilesControl(gpui::Entity<Potato>);
    impl gpui::Render for FilesControl {
        fn render(
            &mut self,
            w: &mut gpui::Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui_kit::{InteractiveElement, MouseButton, ParentElement, Styled, div};
            self.0.update(cx, |s, cx| {
                div()
                    .size_full()
                    .flex()
                    .justify_end()
                    .on_mouse_move(cx.listener(|s, event, w, cx| s.resize_files(event, w, cx)))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|s, _, w, cx| s.finish_file_resize(w, cx)),
                    )
                    .child(s.file_side_view(w, cx))
            })
        }
    }
    #[gpui::test]
    fn resize_handle_changes_width_and_releases_drag(cx: &mut TestAppContext) {
        use gpui_kit::{AppContext, Modifiers, MouseButton, point, px};
        cx.update(gpui_kit::init);
        let backend = Backend::for_ui_test(
            std::env::temp_dir().join(format!("potato-file-drag-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let (control, cx) =
            cx.add_window_view(|w, cx| FilesControl(cx.new(|cx| Potato::new(backend, w, cx))));
        let before = cx.debug_bounds("file-panel-bounds").unwrap();
        let handle = cx.debug_bounds("file-panel-handle").unwrap();
        let from = handle.center();
        let to = point(from.x - px(60.), from.y);
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
        cx.update(|w, _| w.refresh());
        let after = cx.debug_bounds("file-panel-bounds").unwrap();
        assert!(after.size.width > before.size.width + px(40.));
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        control.read_with(cx, |c, cx| assert!(!c.0.read(cx).files.resizing));
        cx.simulate_resize(gpui::size(px(800.), px(600.)));
        let narrow = cx.debug_bounds("file-panel-bounds").unwrap();
        assert!(narrow.size.width <= px(240.));
    }
}
