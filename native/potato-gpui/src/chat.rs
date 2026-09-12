//! Chat presentation keeps the persisted protocol messages unchanged.
use crate::view::{icon_button, message_text};
use crate::*;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{button::*, text::TextView};
use gpui_kit::prelude::*;
use std::collections::BTreeSet;
#[derive(Default)]
pub struct ChatState {
    pub markdown: crate::media::MarkdownCache,
    pub loading: bool,
    pub load_error: bool,
    pub scroll: ScrollHandle,
    pub scroll_paused: bool,
    pub expanded: BTreeSet<String>,
    pub process_open: BTreeMap<String, bool>,
    pub full_process: BTreeSet<String>,
    pub frozen_preview: BTreeMap<String, Vec<usize>>,
    pub full_output: BTreeSet<String>,
    pub completed_runs: BTreeMap<String, Value>,
    pub run_started: Option<std::time::Instant>,
    pub motion: crate::process::ProcessMotion,
    pub copied: Option<String>,
    pub edit_backup: Option<(String, Vec<Value>)>,
}
pub const NEW_DRAFT: &str = "__new_chat_draft__";
pub fn draft_key(session: &str, unsent: bool) -> &str {
    if unsent { NEW_DRAFT } else { session }
}
/// Tool outputs only join their matching call; an orphan remains visible.
pub(crate) fn is_call(v: &Value) -> bool {
    matches!(
        v["type"].as_str(),
        Some("plugin_call" | "function_call" | "mcp_tool_call")
    )
}
pub(crate) fn is_output(v: &Value) -> bool {
    matches!(
        v["type"].as_str(),
        Some("plugin_call_output" | "function_call_output" | "mcp_tool_call_output")
    )
}
pub fn presentation(messages: &[Value]) -> Vec<(usize, Value)> {
    let mut rows: Vec<(usize, Value)> = vec![];
    for (i, message) in messages.iter().enumerate() {
        if is_output(message) {
            let call = message["content"][0]["data"]["call_id"]
                .as_str()
                .unwrap_or("");
            if !call.is_empty()
                && let Some((_, parent)) = rows
                    .iter_mut()
                    .rev()
                    .take_while(|(_, m)| !turn_boundary(m))
                    .find(|(_, m)| is_call(m) && m["content"][0]["data"]["call_id"] == call)
            {
                parent["tool_result"] = message.clone();
                continue;
            }
        }
        rows.push((i, message.clone()));
    }
    rows
}
pub enum ChatBlock {
    Message {
        index: usize,
        message: Value,
        final_answer: bool,
    },
    Process {
        index: usize,
        rows: Vec<(usize, Value)>,
        finished: bool,
        active: bool,
        state: String,
        elapsed: Option<u64>,
        answering: bool,
    },
}

fn turn_boundary(message: &Value) -> bool {
    message["role"] == "user"
        && message["metadata"]["steering_state"].is_null()
        && message["metadata"]["question_request_id"].is_null()
}
pub(crate) fn phase(message: &Value) -> &str {
    message["phase"]
        .as_str()
        .or_else(|| message["metadata"]["phase"].as_str())
        .unwrap_or("")
}
pub(crate) fn answer_text(message: &Value) -> bool {
    message["role"] == "assistant"
        && matches!(message["type"].as_str(), None | Some("message"))
        && (!reusable(message).0.trim().is_empty() || !reusable(message).1.is_empty())
}
fn visible(message: &Value) -> bool {
    is_call(message) || is_output(message) || !message_text(message).trim().is_empty()
}
/// Unphased assistant text stays in the same body throughout the run. Only
/// explicit commentary, reasoning and tools belong to the collapsible process.
/// A message-completed event alone never enables final-answer actions.
pub fn chat_blocks(messages: &[Value], streaming: bool, latest_status: &str) -> Vec<ChatBlock> {
    let rows = presentation(messages);
    let mut blocks = vec![];
    let mut start = 0;
    while start < rows.len() {
        if turn_boundary(&rows[start].1) {
            let (index, message) = rows[start].clone();
            blocks.push(ChatBlock::Message {
                index,
                message,
                final_answer: false,
            });
            start += 1;
            continue;
        }
        let end = rows[start..]
            .iter()
            .position(|(_, m)| turn_boundary(m))
            .map_or(rows.len(), |n| start + n);
        let active = streaming && end == rows.len();
        let saved = &rows[start].1["_presentation"];
        let state = if end == rows.len() && !latest_status.is_empty() {
            latest_status
        } else {
            saved["status"].as_str().unwrap_or("")
        };
        // Providers may create the answer placeholder before the reasoning
        // item, then update both in place. A trailing reasoning item must not
        // hide the completed answer; a later tool call still prevents it.
        let last_content = (start..end)
            .rev()
            .find(|&n| visible(&rows[n].1) && rows[n].1["type"] != "reasoning");
        let candidate = last_content.filter(|&n| {
            let m = &rows[n].1;
            answer_text(m)
                && phase(m) != "commentary"
                && !matches!(
                    m["status"].as_str(),
                    Some("failed" | "cancelled" | "in_progress")
                )
        });
        let finished = !active && candidate.is_some() && matches!(state, "" | "completed");
        let bodies: Vec<_> = (start..end)
            .filter(|&n| answer_text(&rows[n].1) && phase(&rows[n].1) != "commentary")
            .collect();
        let process: Vec<_> = rows[start..end]
            .iter()
            .enumerate()
            .filter(|(n, (_, m))| !bodies.contains(&(start + n)) && visible(m))
            .map(|(_, row)| row.clone())
            .collect();
        if !process.is_empty() {
            let inferred = if active {
                "in_progress"
            } else if !state.is_empty() {
                state
            } else if rows[start..end]
                .iter()
                .any(|(_, m)| !is_call(m) && !is_output(m) && m["status"] == "cancelled")
            {
                "cancelled"
            } else if finished {
                "completed"
            } else {
                "incomplete"
            };
            blocks.push(ChatBlock::Process {
                index: rows[start].0,
                rows: process,
                finished,
                active,
                state: inferred.into(),
                elapsed: saved["elapsed"]
                    .as_u64()
                    .or_else(|| crate::process::round_elapsed(&rows[start..end])),
                answering: active
                    && bodies.iter().any(|&n| {
                        matches!(phase(&rows[n].1), "final" | "final_answer") && visible(&rows[n].1)
                    })
                    && !rows[start..end].iter().any(|(_, m)| {
                        is_call(m)
                            && crate::process::step_state(m, true)
                                == crate::process::StepState::Running
                    }),
            });
        }
        for n in bodies {
            let (index, message) = rows[n].clone();
            blocks.push(ChatBlock::Message {
                index,
                message,
                final_answer: finished && Some(n) == candidate,
            });
        }
        start = end;
    }
    blocks
}

/// Explicit user choices take precedence over all automatic transitions.
pub fn process_is_open(choice: Option<bool>, finished: bool) -> bool {
    choice.unwrap_or(!finished)
}
pub(crate) fn row_key(session: &str, index: usize, message: &Value) -> String {
    format!(
        "{session}:{}",
        message["id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| index.to_string())
    )
}
pub(crate) fn duration_label(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}秒")
    } else if seconds < 3600 {
        format!("{}分{}秒", seconds / 60, seconds % 60)
    } else {
        format!("{}小时{}分", seconds / 3600, seconds % 3600 / 60)
    }
}
pub(crate) fn tool_failed(message: &Value) -> bool {
    let result = if is_output(message) {
        message
    } else {
        &message["tool_result"]
    };
    if result["status"] == "failed" || result["content"][0]["data"]["state"] == "error" {
        return true;
    }
    // A shell process may finish normally with a nonzero exit code.
    let output = result["content"][0]["data"]["output"]
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or(Value::Null);
    output["exit_code"].as_i64().is_some_and(|n| n != 0)
}
pub(crate) fn tool_label(message: &Value) -> (IconName, String) {
    if message["type"] == "reasoning" {
        return (IconName::Sparkles, "思考".into());
    }
    let data = &message["content"][0]["data"];
    let args = data["arguments"]
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| data["arguments"].clone());
    let path = args["file_path"]
        .as_str()
        .or_else(|| args["path"].as_str())
        .unwrap_or("");
    let file = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    let name = data["name"].as_str().unwrap_or("工具调用");
    let (icon, fallback) = match name {
        "create_office_file" => (IconName::FileText, format!("生成 Office 文件 {file}")),
        "fill_office_template" => (IconName::FileText, format!("填充 Office 模板 {file}")),
        "read_file" => (IconName::FileText, format!("读取 {file}")),
        "write_file" | "edit_file" | "apply_patch" => (
            IconName::SquarePen,
            if file.is_empty() {
                "修改文件".into()
            } else {
                format!("修改 {file}")
            },
        ),
        "execute_shell_command" | "exec_command" => (
            IconName::Terminal,
            args["command"]
                .as_str()
                .map(|s| format!("运行 {s}"))
                .unwrap_or_else(|| "运行命令".into()),
        ),
        "job_output" => (IconName::Terminal, "读取命令输出".into()),
        "search_files" | "file_search" => (IconName::Search, "搜索文件".into()),
        _ => (IconName::Blocks, name.to_owned()),
    };
    (
        icon,
        args["description"]
            .as_str()
            .or_else(|| data["description"].as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&fallback)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}
pub(crate) fn tool_details(message: &Value) -> String {
    if message["type"] == "reasoning" {
        return message_text(message);
    }
    let data = &message["content"][0]["data"];
    let raw = data["arguments"].as_str().unwrap_or("");
    let args = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| data["arguments"].clone());
    let input = args["command"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if args.is_null() {
                raw.into()
            } else {
                serde_json::to_string_pretty(&args).unwrap_or_default()
            }
        });
    let output = if is_output(message) {
        data["output"].as_str()
    } else {
        message["tool_result"]["content"][0]["data"]["output"].as_str()
    };
    match output {
        Some(out) if input.is_empty() => out.into(),
        Some(out) => format!("{input}\n{out}"),
        None => input,
    }
}
fn answer_button(id: &'static str, icon: IconName, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .rounded_full()
        .size(px(32.))
        .px_0()
        .child(Icon::new(icon).size(px(16.)))
        .tooltip(label)
        .accessibility_label(label)
}
pub(crate) fn preview_rows(rows: &[(usize, Value)], full: bool) -> Vec<usize> {
    if full {
        return (0..rows.len()).collect();
    }
    let last_text = rows.iter().rposition(|(_, m)| answer_text(m));
    let recent: BTreeSet<_> = rows
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, (_, m))| is_call(m) || is_output(m) || m["type"] == "reasoning")
        .take(3)
        .map(|(n, _)| n)
        .collect();
    (0..rows.len())
        .filter(|n| {
            Some(*n) == last_text
                || recent.contains(n)
                || rows[*n].1["role"] == "user"
                || crate::process::step_state(&rows[*n].1, true)
                    == crate::process::StepState::Running
        })
        .collect()
}
pub(crate) fn output_preview(text: &str, full: bool) -> (String, bool) {
    if full {
        return (text.into(), false);
    }
    let mut lines = 0;
    let end = text
        .char_indices()
        .enumerate()
        .find_map(|(count, (byte, c))| {
            if c == '\n' {
                lines += 1;
            }
            (count >= 3000 || lines >= 10).then_some(byte)
        });
    end.map(|end| (text[..end].to_owned(), true))
        .unwrap_or_else(|| (text.into(), false))
}
pub fn reusable(message: &Value) -> (String, Vec<Value>) {
    if let Some(text) = message["content"].as_str() {
        return (text.into(), vec![]);
    }
    let content = message["content"].as_array().cloned().unwrap_or_default();
    let text = content
        .iter()
        .filter(|v| v["type"] == "text")
        .filter_map(|v| v["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (
        text,
        content
            .into_iter()
            .filter(|v| v["type"] != "text")
            .collect(),
    )
}
impl Potato {
    pub fn preserve_process_reading(&mut self) {
        // Freeze the live preview before any rows can disappear on completion
        // or as newer activity arrives while the user reads previous content.
        let all: Vec<_> = self
            .history
            .iter()
            .chain(&self.turn.messages)
            .cloned()
            .collect();
        for block in chat_blocks(&all, self.streaming, &self.turn.status) {
            if let ChatBlock::Process {
                index,
                rows,
                active: true,
                ..
            } = block
            {
                let key = row_key(&self.session, index, &all[index]);
                self.chat.process_open.entry(key.clone()).or_insert(true);
                let preview = preview_rows(&rows, self.chat.full_process.contains(&key))
                    .into_iter()
                    .map(|n| rows[n].0)
                    .collect();
                self.chat.frozen_preview.entry(key).or_insert(preview);
            }
        }
    }
    pub fn finish_process(&mut self) {
        if self.chat.scroll_paused {
            self.preserve_process_reading();
        }
        if let Some(first) = self.turn.messages.first() {
            let key = row_key(&self.session, self.history.len(), first);
            self.chat.completed_runs.insert(
                key,
                json!({
                    "status": self.turn.status,
                    "elapsed": self.chat.run_started.take().map(|t| t.elapsed().as_secs())
                }),
            );
        } else {
            self.chat.run_started = None;
        }
    }

    pub fn cancel_chat_edit(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.streaming {
            return;
        }
        if let Some((text, attachments)) = self.chat.edit_backup.take() {
            self.composer.update(cx, |v, cx| v.set_value(text, w, cx));
            self.attachments = attachments;
        }
        cx.notify();
    }
    pub fn reuse_chat_message(
        &mut self,
        index: usize,
        send: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.streaming
            || self.busy
            || self.voice.active
            || self.chat.loading
            || self.chat.load_error
            || self.chat.edit_backup.is_some()
        {
            return;
        }
        if send
            && !matches!(
                chat_blocks(&self.history, false, &self.turn.status).last(),
                Some(ChatBlock::Message { index: last, final_answer: true, .. }) if *last == index
            )
        {
            return;
        }
        let Some(message) = self
            .history
            .iter()
            .take(index + 1)
            .rev()
            .find(|m| turn_boundary(m))
        else {
            return;
        };
        let (text, attachments) = reusable(message);
        if text.trim().is_empty() && attachments.is_empty() {
            return;
        }
        self.chat.edit_backup = Some((
            self.composer.read(cx).value().to_string(),
            self.attachments.clone(),
        ));
        self.attachments = attachments;
        self.composer.update(cx, |v, cx| {
            v.set_value(text, w, cx);
            v.focus(w, cx);
        });
        if send {
            self.send(w, cx);
        }
        cx.notify();
    }
    pub fn message_view(
        &self,
        i: usize,
        m: &Value,
        final_answer: bool,
        last: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = row_key(&self.session, i, m);
        let user = m["role"] == "user";
        let tool = is_call(m) || is_output(m);
        let reasoning = m["type"] == "reasoning";
        let text = message_text(m);
        let hover_group = format!("chat-message-{i}");
        let mut card = div()
            .id(ElementId::Name(format!("message-row-{key}").into()))
            .group(hover_group.clone())
            .w_full()
            .max_w(px(crate::design::CHAT_WIDTH))
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .when(user, |d| d.items_end());
        if tool || reasoning {
            let expanded = self.chat.expanded.contains(&key);
            let failed = tool_failed(m);
            let running = self.streaming && tool && !is_output(m) && m["tool_result"].is_null();
            let (icon, name) = tool_label(m);
            let toggle_key = key.clone();
            let mut toggle = Button::new(("expand", i))
                .ghost()
                .w_full()
                .h(px(32.))
                .px_0()
                .justify_start()
                .font_weight(FontWeight::NORMAL)
                // Only the trailing chevron inherits this transparent idle
                // color. Button hover and keyboard focus reveal it; the title
                // and status icon retain their explicit, readable colors.
                .text_color(rgba(0x00000000))
                .focus(|style| style.text_color(cx.theme().muted_foreground))
                .accessibility_label(format!(
                    "{}，{}{}",
                    name,
                    if failed { "失败，" } else { "" },
                    if expanded {
                        "收起详情"
                    } else {
                        "展开详情"
                    }
                ))
                .tooltip(if failed {
                    format!("{name}（失败，点击查看详情）")
                } else {
                    name.clone()
                });
            if running {
                toggle = toggle.child(Spinner::new().with_size(px(16.)));
            } else {
                toggle = toggle.child(Icon::new(icon).size(px(16.)).text_color(if failed {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                }));
            }
            toggle = toggle
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(14.))
                        .text_color(cx.theme().muted_foreground)
                        .text_ellipsis()
                        .child(name),
                )
                .child(
                    Icon::new(if expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .size(px(16.)),
                );
            let mut detail =
                div()
                    .w_full()
                    .min_w_0()
                    .child(toggle.on_click(cx.listener(move |s, _, _, cx| {
                        s.chat.scroll_paused = true;
                        s.preserve_process_reading();
                        if !s.chat.expanded.remove(&toggle_key) {
                            s.chat.expanded.insert(toggle_key.clone());
                        }
                        cx.notify();
                    })));
            if expanded {
                let content = tool_details(m);
                let full = self.chat.full_output.contains(&key);
                let (shown, truncated) = output_preview(&content, full);
                let body = if reasoning {
                    shown
                } else {
                    let fence = "`".repeat(content.matches('`').count().max(2) + 1);
                    format!("{fence}\n{shown}\n{fence}")
                };
                detail = detail.child(
                    div()
                        .mt_1()
                        .mb_2()
                        .p_3()
                        .min_w_0()
                        .rounded_lg()
                        .bg(cx.theme().muted)
                        .child(TextView::markdown(("detail", i), body).selectable(true)),
                );
                if truncated {
                    let key = key.clone();
                    detail = detail.child(
                        Button::new(("full-output", i))
                            .ghost()
                            .small()
                            .label("展开完整输出")
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.chat.full_output.insert(key.clone());
                                cx.notify();
                            })),
                    );
                }
            }
            card = card.child(detail);
        } else {
            card = card.child(
                div()
                    .when(!user, |d| d.text_size(px(16.)).line_height(px(26.)))
                    .when(!user, |d| d.w_full())
                    .max_w_full()
                    .min_w_0()
                    .when(user, |d| {
                        d.bg(cx.theme().muted).rounded(px(20.)).px_5().py_3()
                    })
                    .child(crate::media::message_view(
                        &self.session,
                        &self.chat.markdown,
                        i,
                        m,
                        cx,
                    )),
            );
        }
        let copy_key = key.clone();
        let copy_text = if m["tool_result"].is_null() {
            text
        } else {
            format!("{text}\n\n{}", message_text(&m["tool_result"]))
        };
        let copied = self.chat.copied.as_ref() == Some(&key);
        let mut actions = div().flex().items_center().gap_1();
        if final_answer {
            actions = actions.child(
                answer_button(
                    "copy",
                    if copied {
                        IconName::Check
                    } else {
                        IconName::Copy
                    },
                    if copied { "已复制" } else { "复制回复" },
                )
                .rounded_full()
                .size(px(32.))
                .on_click(cx.listener(move |s, _, w, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
                    s.chat.copied = Some(copy_key.clone());
                    let key = copy_key.clone();
                    cx.spawn_in(w, async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_secs(2))
                            .await;
                        let _ = this.update_in(cx, |s, _, cx| {
                            if s.chat.copied.as_ref() == Some(&key) {
                                s.chat.copied = None;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                    cx.notify();
                })),
            );
        }
        if user {
            actions = actions.child(
                icon_button("edit-message", IconName::SquarePen, "编辑并重发")
                    .opacity(0.)
                    .group_hover(hover_group, |style| style.opacity(1.))
                    .focus(|style| style.opacity(1.))
                    .disabled(self.streaming || self.chat.edit_backup.is_some())
                    .on_click(
                        cx.listener(move |s, _, w, cx| s.reuse_chat_message(i, false, w, cx)),
                    ),
            );
        } else if final_answer && last {
            actions = actions.child(
                answer_button("regenerate", IconName::RefreshCw, "重新生成")
                    .rounded_full()
                    .size(px(32.))
                    .disabled(
                        self.streaming
                            || self.busy
                            || self.voice.active
                            || self.chat.edit_backup.is_some(),
                    )
                    .on_click(cx.listener(move |s, _, w, cx| s.reuse_chat_message(i, true, w, cx))),
            );
        }
        card.when(user || final_answer, |d| d.child(actions))
            .into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::{ChatBlock, chat_blocks, presentation, reusable};
    use serde_json::json;
    #[test]
    fn tools_pair_by_call_id_and_keep_orphans() {
        let call = |id| json!({"type":"plugin_call","content":[{"data":{"call_id":id}}]});
        let output = |id| json!({"type":"plugin_call_output","content":[{"data":{"call_id":id,"output":"done"}}]});
        let messages = vec![
            call("a"),
            call("b"),
            output("b"),
            output("a"),
            output("unknown"),
        ];
        let rows = presentation(&messages);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0].1["tool_result"]["content"][0]["data"]["call_id"],
            "a"
        );
        assert_eq!(
            rows[1].1["tool_result"]["content"][0]["data"]["call_id"],
            "b"
        );
        assert_eq!(rows[2].0, 4);
        assert!(messages[0]["tool_result"].is_null());
    }
    #[test]
    fn editing_preserves_attachments_without_converting_them_to_text() {
        let file = json!({"type":"file","file_name":"资料.pdf","file_url":"data:application/pdf;base64,AA=="});
        let (text, files) =
            reusable(&json!({"content":[{"type":"text","text":"读一下"},file.clone()]}));
        assert_eq!(text, "读一下");
        assert_eq!(files, vec![file]);
    }
    fn message(role: &str, kind: &str, text: &str) -> serde_json::Value {
        json!({"role":role,"type":kind,"status":"completed","content":[{"type":"text","text":text}]})
    }
    #[test]
    fn empty_reasoning_does_not_create_process_rows() {
        let messages = vec![
            message("user", "message", "question"),
            message("assistant", "message", "answer"),
            message("assistant", "reasoning", "  "),
        ];
        let blocks = chat_blocks(&messages, false, "completed");
        assert_eq!(blocks.len(), 2);
        assert!(matches!(
            &blocks[1],
            ChatBlock::Message {
                final_answer: true,
                ..
            }
        ));
        let blocks = chat_blocks(&messages, true, "in_progress");
        assert!(matches!(
            &blocks[1],
            ChatBlock::Message {
                final_answer: false,
                ..
            }
        ));
    }
    #[test]
    fn completed_turn_groups_narration_and_tools_before_one_final_answer() {
        let messages = vec![
            message("user", "message", "question"),
            {
                let mut m = message("assistant", "message", "checking files");
                m["phase"] = json!("commentary");
                m
            },
            message("assistant", "reasoning", "thinking"),
            json!({"type":"function_call","role":"assistant","content":[{"data":{"call_id":"x"}}]}),
            json!({"type":"function_call_output","role":"tool","content":[{"data":{"call_id":"x","output":"done"}}]}),
            message("assistant", "message", "final answer"),
        ];
        let blocks = chat_blocks(&messages, false, "");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(
            &blocks[0],
            ChatBlock::Message {
                final_answer: false,
                ..
            }
        ));
        assert!(
            matches!(&blocks[1], ChatBlock::Process { rows, finished: true, active: false, .. } if rows.len()==3 && !rows[2].1["tool_result"].is_null())
        );
        assert!(matches!(
            &blocks[2],
            ChatBlock::Message {
                index: 5,
                final_answer: true,
                ..
            }
        ));
        let live = chat_blocks(&messages, true, "");
        assert!(matches!(
            &live[1],
            ChatBlock::Process {
                finished: false,
                active: true,
                ..
            }
        ));
        assert_eq!(live.len(), 3, "unphased live text stays in the body");
        assert!(matches!(
            &live[2],
            ChatBlock::Message {
                index: 5,
                final_answer: false,
                ..
            }
        ));
    }
    #[test]
    fn body_is_never_reclassified_when_a_tool_arrives_or_run_stops() {
        let body = message("assistant", "message", "# Title\n\n".repeat(100).as_str());
        for status in ["in_progress", "completed", "cancelled", "failed"] {
            let rows = vec![
                body.clone(),
                json!({"type":"function_call", "role":"assistant", "content":[]}),
            ];
            let blocks = chat_blocks(&rows, status == "in_progress", status);
            assert!(
                blocks
                    .iter()
                    .any(|b| matches!(b, ChatBlock::Message { index: 0, .. }))
            );
            assert!(blocks.iter().all(
                |b| !matches!(b, ChatBlock::Process { rows, .. } if rows.iter().any(|(i,_)| *i==0))
            ));
        }
    }
    #[test]
    fn earlier_turn_stays_settled_while_latest_turn_streams() {
        let messages = vec![
            message("user", "message", "one"),
            message("assistant", "reasoning", "thinking"),
            message("assistant", "message", "answer one"),
            message("user", "message", "two"),
            message("assistant", "reasoning", "still thinking"),
        ];
        let blocks = chat_blocks(&messages, true, "");
        assert!(matches!(
            &blocks[1],
            ChatBlock::Process {
                finished: true,
                active: false,
                ..
            }
        ));
        assert!(matches!(
            &blocks[2],
            ChatBlock::Message {
                final_answer: true,
                ..
            }
        ));
        assert!(matches!(
            &blocks[4],
            ChatBlock::Process {
                finished: false,
                active: true,
                ..
            }
        ));
    }
    #[test]
    fn failed_or_incomplete_turn_is_not_hidden_as_a_success() {
        for status in ["failed", "cancelled", "in_progress"] {
            let mut partial = message("assistant", "message", "partial text");
            partial["status"] = json!(status);
            let blocks = chat_blocks(
                &[message("assistant", "reasoning", "details"), partial],
                false,
                "",
            );
            assert!(!blocks.iter().any(|b| matches!(
                b,
                ChatBlock::Message {
                    final_answer: true,
                    ..
                }
            )));
            assert!(matches!(
                &blocks[0],
                ChatBlock::Process {
                    finished: false,
                    ..
                }
            ));
        }
    }
    #[test]
    fn tool_ids_never_pair_across_user_turns() {
        let messages = vec![
            json!({"type":"function_call","content":[{"data":{"call_id":"x"}}]}),
            message("user", "message", "new turn"),
            json!({"type":"function_call_output","content":[{"data":{"call_id":"x"}}]}),
        ];
        assert_eq!(presentation(&messages).len(), 3);
    }
    #[test]
    fn question_answer_keeps_call_and_result_in_one_completed_process() {
        let rows = vec![
            message("user", "message", "choose fruit"),
            json!({"role":"assistant", "type":"function_call", "content":[{"data":{"call_id":"q"}}]}),
            json!({"role":"user", "metadata":{"question_request_id":"question-1"}, "content":"banana"}),
            json!({"role":"tool", "type":"function_call_output", "content":[{"data":{"call_id":"q", "state":"success"}}]}),
            message("assistant", "message", "banana"),
        ];
        let blocks = chat_blocks(&rows, false, "completed");
        assert_eq!(blocks.len(), 3);
        assert!(
            matches!(&blocks[1], ChatBlock::Process { rows, finished: true, .. }
            if rows.len() == 2 && !rows[0].1["tool_result"].is_null())
        );
    }
    #[test]
    fn steering_between_call_and_result_keeps_one_process_and_pairs_output() {
        let rows = vec![
            message("user", "message", "request"),
            json!({"id":"call", "role":"assistant", "type":"function_call", "content":[{"data":{"call_id":"x"}}]}),
            json!({"id":"steer", "role":"user", "metadata":{"steering_state":"delivered"}, "content":"use another command"}),
            json!({"role":"tool", "type":"function_call_output", "content":[{"data":{"call_id":"x", "state":"error"}}]}),
            message("assistant", "message", "recovered"),
        ];
        let blocks = chat_blocks(&rows, false, "completed");
        assert_eq!(blocks.len(), 3);
        assert!(
            matches!(&blocks[1], ChatBlock::Process {rows, finished:true, ..}
            if rows.len()==2 && super::tool_failed(&rows[0].1) && rows[1].1["role"]=="user")
        );
        assert!(matches!(
            &blocks[2],
            ChatBlock::Message {
                final_answer: true,
                ..
            }
        ));
    }
    #[test]
    fn response_failure_overrides_completed_text_and_explicit_final_streams_separately() {
        let mut text = message("assistant", "message", "partial answer");
        text["phase"] = json!("final");
        let messages = vec![message("assistant", "reasoning", "summary"), text];
        assert!(matches!(
            chat_blocks(&messages, true, "in_progress").last(),
            Some(ChatBlock::Message {
                final_answer: false,
                ..
            })
        ));
        assert!(
            !chat_blocks(&messages, false, "failed")
                .iter()
                .any(|b| matches!(
                    b,
                    ChatBlock::Message {
                        final_answer: true,
                        ..
                    }
                ))
        );
        assert!(matches!(
            chat_blocks(&messages, false, "completed").last(),
            Some(ChatBlock::Message {
                final_answer: true,
                ..
            })
        ));
    }
    #[test]
    fn explicit_commentary_and_empty_tail_are_not_final_answers() {
        let mut commentary = message("assistant", "message", "checking");
        commentary["phase"] = json!("commentary");
        assert!(matches!(
            &chat_blocks(&[commentary], false, "completed")[0],
            ChatBlock::Process {
                finished: false,
                ..
            }
        ));
        let messages = vec![
            message("assistant", "message", "answer"),
            message("assistant", "message", ""),
        ];
        assert!(matches!(
            &chat_blocks(&messages, false, "completed")[0],
            ChatBlock::Message {
                index: 0,
                final_answer: true,
                ..
            }
        ));
    }
    #[test]
    fn long_unicode_output_preview_keeps_exact_recoverable_prefix() {
        let output = "中文🦀".repeat(1500);
        let (preview, truncated) = super::output_preview(&output, false);
        assert!(truncated && output.starts_with(&preview));
        assert_eq!(super::output_preview(&output, true), (output, false));
    }
    #[test]
    fn trailing_reasoning_does_not_hide_answer_but_a_later_tool_does() {
        let mut rows = vec![
            message("assistant", "message", "answer"),
            message("assistant", "reasoning", "summary"),
        ];
        assert!(matches!(
            chat_blocks(&rows, false, "completed").last(),
            Some(ChatBlock::Message {
                final_answer: true,
                index: 0,
                ..
            })
        ));
        rows.push(json!({"role":"assistant", "type":"function_call", "content":[]}));
        assert!(
            !chat_blocks(&rows, false, "completed")
                .iter()
                .any(|b| matches!(
                    b,
                    ChatBlock::Message {
                        final_answer: true,
                        ..
                    }
                ))
        );
    }
    #[test]
    fn nonzero_shell_exit_is_local_failure_even_when_transport_succeeds() {
        let tool = json!({"type":"function_call", "tool_result":{"status":"completed", "content":[{"data":{"state":"success", "output":"{\"status\":\"completed\",\"exit_code\":1}"}}]}});
        assert!(super::tool_failed(&tool));
        let blocks = chat_blocks(
            &[tool, message("assistant", "message", "done")],
            false,
            "completed",
        );
        assert!(matches!(
            &blocks[0],
            ChatBlock::Process { finished: true, .. }
        ));
    }
}
