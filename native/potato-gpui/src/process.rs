//! Native activity timeline. Protocol identity, motion and disclosure are independent.
use crate::view::message_text;
use crate::{chat::*, *};
use gpui_kit::component::{button::*, text::TextView};
use gpui_kit::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepState {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

fn activity(m: &Value) -> &Value {
    if m["tool_result"]["metadata"]["activity"].is_object() {
        &m["tool_result"]["metadata"]["activity"]
    } else {
        &m["metadata"]["activity"]
    }
}
pub(crate) fn step_state(m: &Value, active: bool) -> StepState {
    let state = activity(m)["state"].as_str().unwrap_or("");
    if state == "cancelled" || m["status"] == "cancelled" {
        return StepState::Cancelled;
    }
    if state == "failed" || tool_failed(m) || m["status"] == "failed" {
        return StepState::Failed;
    }
    if state == "completed" || is_output(m) || !m["tool_result"].is_null() {
        return StepState::Completed;
    }
    if is_call(m) || m["status"] == "in_progress" || state == "running" {
        return if active {
            StepState::Running
        } else {
            StepState::Interrupted
        };
    }
    StepState::Completed
}
fn step_duration(m: &Value) -> Option<String> {
    activity(m)["elapsed_ms"].as_u64().map(|ms| {
        if ms < 1000 {
            "不足1秒".into()
        } else {
            duration_label(ms / 1000)
        }
    })
}
pub(crate) fn round_elapsed(rows: &[(usize, Value)]) -> Option<u64> {
    let start = rows
        .iter()
        .filter_map(|(_, m)| activity(m)["started_at_ms"].as_i64())
        .min()?;
    let end = rows
        .iter()
        .filter_map(|(_, m)| activity(m)["completed_at_ms"].as_i64())
        .max()?;
    Some(end.saturating_sub(start).max(0) as u64 / 1000)
}

pub(crate) fn activity_spinner(id: impl Into<SharedString>, color: Hsla) -> AnyElement {
    Icon::new(gpui_kit::component::IconName::LoaderCircle)
        .size(px(20.))
        .text_color(color)
        .with_animation(
            ElementId::Name(id.into()),
            Animation::new(Duration::from_millis(1200)).repeat_synced(),
            |icon, phase| icon.transform(Transformation::rotate(percentage(phase))),
        )
        .into_any_element()
}

struct Reveal {
    open: bool,
    height: Rc<Cell<f32>>,
    from: f32,
    started: Instant,
}
#[derive(Default)]
pub(crate) struct ProcessMotion(RefCell<BTreeMap<String, Reveal>>);
impl ProcessMotion {
    /// Measure the natural height separately from the clipping parent. Retarget
    /// from the current height on reversal, never restart on a text delta.
    fn reveal(&self, key: String, open: bool, content: AnyElement, cx: &App) -> AnyElement {
        let mut states = self.0.borrow_mut();
        let now = Instant::now();
        let state = states.entry(key.clone()).or_insert_with(|| Reveal {
            open,
            height: Rc::new(Cell::new(0.)),
            from: 0.,
            started: now - Duration::from_secs(1),
        });
        let fraction = (state.started.elapsed().as_secs_f32() / 0.18).min(1.);
        let eased = 1. - (1. - fraction).powi(3);
        let target = if state.open { state.height.get() } else { 0. };
        let current = state.from + (target - state.from) * eased;
        if state.open != open {
            state.from = current;
            state.started = now;
            state.open = open;
        }
        let fraction = (state.started.elapsed().as_secs_f32() / 0.18).min(1.);
        let animating = fraction < 1. && !cx.reduce_motion();
        let height = state.height.clone();
        if !open && !animating {
            return div().into_any_element();
        }
        let measured = div()
            .w_full()
            .child(content)
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(bounds) = bounds.first() {
                    height.set(f32::from(bounds.size.height));
                }
            });
        let target = if open { state.height.get() } else { 0. };
        let eased = 1. - (1. - fraction).powi(3);
        let mut wrapper = div().w_full().min_w_0().overflow_hidden();
        if animating {
            wrapper = wrapper.h(px(state.from + (target - state.from) * eased));
        }
        // This frame driver is scoped to the transition; it does no work when settled.
        if animating {
            wrapper
                .child(measured)
                .with_animation(
                    ElementId::Name(format!("reveal-{key}").into()),
                    Animation::new(Duration::from_millis(180)).repeat(),
                    |el, _| el,
                )
                .into_any_element()
        } else {
            wrapper.child(measured).into_any_element()
        }
    }
}

impl Potato {
    #[allow(clippy::too_many_arguments)]
    pub fn process_view(
        &self,
        index: usize,
        rows: &[(usize, Value)],
        state: &str,
        finished: bool,
        elapsed: Option<u64>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let source = self
            .history
            .iter()
            .chain(&self.turn.messages)
            .nth(index)
            .unwrap_or(&rows[0].1);
        let key = row_key(&self.session, index, source);
        let active = state == "in_progress";
        let waiting = if active && !self.interactions.approvals.is_empty() {
            Some("等待确认")
        } else if active && !self.interactions.questions.is_empty() {
            Some("等待你的回复")
        } else {
            None
        };
        let open = process_is_open(self.chat.process_open.get(&key).copied(), finished);
        let seconds = if active {
            self.chat.run_started.map(|t| t.elapsed().as_secs())
        } else {
            elapsed
        };
        let count = rows
            .iter()
            .filter(|(_, m)| is_call(m) || is_output(m) || m["type"] == "reasoning")
            .count();
        let heading = if let Some(waiting) = waiting {
            waiting.to_owned()
        } else if state == "cancelled" {
            "已停止".into()
        } else if !active && !finished {
            "已中断".into()
        } else if finished {
            if count > 0 {
                format!("执行了 {count} 个步骤")
            } else {
                "执行过程".into()
            }
        } else {
            "进行中".into()
        };
        let label = seconds
            .map(|n| format!("{heading} · {}", duration_label(n)))
            .unwrap_or(heading);
        let toggle_key = key.clone();
        let header = Button::new(("process-toggle", index))
            .text()
            .small()
            .h(px(32.))
            .w_full()
            .px_0()
            .justify_start()
            .font_weight(FontWeight::NORMAL)
            .text_color(cx.theme().muted_foreground)
            .accessibility_label(format!(
                "{}执行过程，{label}",
                if open { "收起" } else { "展开" }
            ))
            .tooltip(if open {
                "收起执行过程"
            } else {
                "展开执行过程"
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Icon::new(if open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .size(px(14.)),
                    )
                    .child(div().text_size(px(14.)).child(label)),
            )
            .on_click(cx.listener(move |s, _, _, cx| {
                s.chat.scroll_paused = true;
                s.chat.process_open.insert(toggle_key.clone(), !open);
                if !open {
                    s.chat.full_process.insert(toggle_key.clone());
                    s.chat.frozen_preview.remove(&toggle_key);
                }
                cx.notify();
            }));
        let selected: Vec<usize> = if let Some(frozen) = self.chat.frozen_preview.get(&key) {
            rows.iter()
                .enumerate()
                .filter(|(_, (i, _))| frozen.contains(i))
                .map(|(n, _)| n)
                .collect()
        } else {
            preview_rows(rows, !active || self.chat.full_process.contains(&key))
        };
        let mut details = div().w_full().min_w_0().pl(px(24.)).pt_2().pb_1();
        let running = rows
            .iter()
            .filter(|(_, m)| is_call(m) && step_state(m, active) == StepState::Running)
            .count();
        let pending = active
            && waiting.is_none()
            && !rows
                .iter()
                .any(|(_, m)| step_state(m, true) == StepState::Running)
            && !self.chat.frozen_preview.contains_key(&key);
        if running > 1 {
            details = details.child(
                div()
                    .pl(px(8.))
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(if waiting.is_none() {
                        activity_spinner(format!("parallel-{key}"), cx.theme().muted_foreground)
                    } else {
                        Icon::new(IconName::Clock).size(px(18.)).into_any_element()
                    })
                    .child(format!("并行执行 {running} 个工具")),
            );
        }
        for (position, n) in selected.iter().enumerate() {
            let (i, message) = &rows[*n];
            if message["role"] == "user" {
                details = details.child(
                    div()
                        .pl(px(40.))
                        .py_2()
                        .text_size(px(14.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("补充指令：{}", message_text(message))),
                );
            } else if is_call(message) || is_output(message) || message["type"] == "reasoning" {
                details = details.child(self.process_step(
                    *i,
                    message,
                    active,
                    waiting,
                    position + 1 < selected.len() || pending,
                    running <= 1,
                    cx,
                ));
            } else {
                details = details.child(
                    div()
                        .pl(px(40.))
                        .py_2()
                        .pb_4()
                        .child(self.message_view(*i, message, false, false, cx)),
                );
            }
        }
        if pending {
            // Text after a group settles it, so the live group only waits on thinking.
            let placeholder = json!({"id":format!("pending-{key}"),"role":"assistant","type":"reasoning",
                "status":"in_progress","content":"","metadata":{"activity_label":"正在思考"}});
            details = details.child(self.process_step(
                usize::MAX - index,
                &placeholder,
                true,
                None,
                false,
                true,
                cx,
            ));
        }
        if selected.len() < rows.len() {
            let key = key.clone();
            details = details.child(
                Button::new(("earlier-process", index))
                    .ghost()
                    .small()
                    .ml(px(32.))
                    .font_weight(FontWeight::NORMAL)
                    .label("更早记录")
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.chat.scroll_paused = true;
                        s.chat.process_open.insert(key.clone(), true);
                        s.chat.full_process.insert(key.clone());
                        s.chat.frozen_preview.remove(&key);
                        cx.notify();
                    })),
            );
        }
        let disclosure = self
            .chat
            .motion
            .reveal(key, open, details.into_any_element(), cx);
        div()
            .id(("process", index))
            .w_full()
            .max_w(px(crate::design::CHAT_WIDTH))
            .min_w_0()
            .flex()
            .flex_col()
            .child(header)
            .child(disclosure)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn process_step(
        &self,
        i: usize,
        m: &Value,
        active: bool,
        waiting: Option<&str>,
        connected: bool,
        animate: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = row_key(&self.session, i, m);
        let status = step_state(m, active);
        let running = status == StepState::Running && waiting.is_none();
        let reasoning = m["type"] == "reasoning";
        let placeholder = m["metadata"]["activity_label"].is_string();
        let (kind_icon, title) = tool_label(m);
        let title = if let Some(label) = m["metadata"]["activity_label"].as_str() {
            label.into()
        } else if reasoning {
            if running {
                "正在思考".into()
            } else {
                "思考摘要".into()
            }
        } else {
            title
        };
        let suffix = match status {
            StepState::Failed => Some("失败"),
            StepState::Cancelled => Some("已停止"),
            StepState::Interrupted => Some("已中断"),
            StepState::Running if waiting.is_some() => waiting,
            _ => None,
        };
        let color = if status == StepState::Failed {
            cx.theme().danger
        } else if running {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };
        let open = self.chat.expanded.contains(&key);
        let icon = if running && animate {
            activity_spinner(format!("step-{key}"), color)
        } else {
            Icon::new(match status {
                StepState::Failed => IconName::Info,
                StepState::Cancelled | StepState::Interrupted => IconName::Square,
                StepState::Running if waiting.is_some() => IconName::Clock,
                _ if reasoning && !running => IconName::Check,
                _ => kind_icon,
            })
            .size(px(17.))
            .text_color(color)
            .into_any_element()
        };
        let icon = div()
            .child(icon)
            .when(active && !running, |d| d.opacity(1.));
        let icon = if active && status != StepState::Running {
            icon.with_animation(
                ElementId::Name(format!("settled-{key}-{status:?}").into()),
                Animation::new(Duration::from_millis(140)),
                |d, t| d.opacity(0.4 + 0.6 * t),
            )
            .into_any_element()
        } else {
            icon.into_any_element()
        };
        let mut rail = div()
            .w(px(40.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(7.))
            .child(
                div()
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(reasoning && status == StepState::Completed, |d| {
                        d.rounded_full().bg(cx.theme().muted)
                    })
                    .child(icon),
            );
        if connected {
            rail = rail.child(
                div()
                    .w(px(1.))
                    .flex_1()
                    .min_h(px(16.))
                    .mt_1()
                    .bg(cx.theme().border),
            );
        }
        let toggle_key = key.clone();
        let button = Button::new(("process-step", i))
            .text()
            .disabled(placeholder)
            .w_full()
            .h(px(36.))
            .px_2()
            .justify_start()
            .font_weight(if running {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            })
            .text_color(color)
            .accessibility_label(format!(
                "{title}，{}{}",
                suffix.unwrap_or(""),
                if open { "收起详情" } else { "展开详情" }
            ))
            .tooltip(title.clone())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(15.))
                    .text_ellipsis()
                    .child(title),
            )
            .when_some(suffix, |b, text| {
                b.child(div().text_size(px(12.)).child(text.to_owned()))
            })
            .when(!placeholder, |b| {
                b.child(
                    Icon::new(if open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .size(px(14.))
                    .text_color(cx.theme().muted_foreground),
                )
            })
            .on_click(cx.listener(move |s, _, _, cx| {
                s.chat.scroll_paused = true;
                s.preserve_process_reading();
                if !s.chat.expanded.remove(&toggle_key) {
                    s.chat.expanded.insert(toggle_key.clone());
                }
                cx.notify();
            }));
        let mut body = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .pb(px(16.))
            .child(button);
        if let Some(duration) = step_duration(m) {
            body = body.child(
                div()
                    .px_2()
                    .h(px(20.))
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(duration),
            );
        }
        if reasoning && !open {
            // A bounded, stable summary preview; never an unbounded reasoning dump.
            let text = message_text(m)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !text.is_empty() {
                body = body.child(
                    div()
                        .px_2()
                        .max_h(px(42.))
                        .overflow_hidden()
                        .text_size(px(14.))
                        .text_color(cx.theme().muted_foreground)
                        .child(text.chars().take(110).collect::<String>()),
                );
            }
        }
        if open {
            let content = tool_details(m);
            let full = self.chat.full_output.contains(&key);
            let (shown, truncated) = output_preview(&content, full);
            let text = if reasoning {
                shown
            } else {
                let fence = "`".repeat(content.matches('`').count().max(2) + 1);
                format!("{fence}\n{shown}\n{fence}")
            };
            body = body.child(
                div()
                    .mx_2()
                    .mt_2()
                    .p_3()
                    .rounded_lg()
                    .bg(cx.theme().muted)
                    .min_w_0()
                    .child(TextView::markdown(("process-detail", i), text).selectable(true)),
            );
            if truncated {
                body = body.child(
                    Button::new(("process-full", i))
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
        div()
            .w_full()
            .min_w_0()
            .flex()
            .gap(px(12.))
            .pl(px(4.))
            .child(rail)
            .child(body)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessMotion, StepState, round_elapsed, step_duration, step_state};
    use crate::chat::{ChatBlock, chat_blocks, process_is_open};
    use gpui_kit::{
        Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, TestAppContext,
        Window, div, gpui, px,
    };
    use serde_json::json;
    use std::time::{Duration, Instant};
    struct MotionProbe {
        motion: ProcessMotion,
        open: bool,
    }
    impl Render for MotionProbe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().flex().flex_col().items_start().child(
                div()
                    .w(px(400.))
                    .debug_selector(|| "timeline-reveal-height".into())
                    .child(self.motion.reveal(
                        "probe".into(),
                        self.open,
                        div().h(px(200.)).into_any_element(),
                        cx,
                    )),
            )
        }
    }
    #[gpui::test]
    fn measured_disclosure_retargets_from_current_height_and_settles(cx: &mut TestAppContext) {
        let (probe, cx) = cx.add_window_view(|_, _| MotionProbe {
            motion: ProcessMotion::default(),
            open: true,
        });
        assert_eq!(
            cx.debug_bounds("timeline-reveal-height")
                .unwrap()
                .size
                .height,
            px(200.)
        );
        probe.update(cx, |p, cx| {
            p.open = false;
            cx.notify();
        });
        cx.update(|w, _| w.refresh());
        cx.run_until_parked();
        probe.update(cx, |p, cx| {
            p.motion.0.borrow_mut().get_mut("probe").unwrap().started =
                Instant::now() - Duration::from_millis(90);
            cx.notify();
        });
        cx.update(|w, _| w.refresh());
        cx.run_until_parked();
        let midway = f32::from(
            cx.debug_bounds("timeline-reveal-height")
                .unwrap()
                .size
                .height,
        );
        assert!((15. ..35.).contains(&midway), "midway height {midway}");
        probe.update(cx, |p, cx| {
            p.open = true;
            cx.notify();
        });
        cx.update(|w, _| w.refresh());
        cx.run_until_parked();
        let reverse = f32::from(
            cx.debug_bounds("timeline-reveal-height")
                .unwrap()
                .size
                .height,
        );
        assert!(
            (reverse - midway).abs() < 12.,
            "reversal must not jump: {midway} -> {reverse}"
        );
        cx.update(|_, cx| cx.set_reduce_motion(true));
        probe.update(cx, |p, cx| {
            p.open = false;
            cx.notify();
        });
        cx.update(|w, _| w.refresh());
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("timeline-reveal-height")
                .unwrap()
                .size
                .height,
            px(0.)
        );
    }
    #[test]
    fn recent_preview_keeps_older_running_parallel_step_visible() {
        let mut rows = vec![(0, json!({"type":"function_call","status":"completed"}))];
        for i in 1..6 {
            rows.push((i, json!({"type":"reasoning","status":"completed"})));
        }
        assert_eq!(crate::chat::preview_rows(&rows, false), vec![0, 3, 4, 5]);
    }
    #[test]
    fn parallel_completion_metadata_stops_spinner_before_ordered_output_arrives() {
        let call = json!({"type":"function_call","status":"completed","metadata":{"activity":{"state":"completed","elapsed_ms":6200}}});
        assert_eq!(step_state(&call, true), StepState::Completed);
        assert_eq!(step_duration(&call).as_deref(), Some("6秒"));
        let pending = json!({"type":"function_call","status":"completed"});
        assert_eq!(step_state(&pending, true), StepState::Running);
        assert_eq!(step_state(&pending, false), StepState::Interrupted);
        assert_eq!(step_duration(&pending), None);
    }
    #[test]
    fn history_timing_and_failure_come_from_matching_result() {
        let call = json!({"type":"function_call","metadata":{"activity":{"state":"running","started_at_ms":1000}},
            "tool_result":{"status":"failed","metadata":{"activity":{"state":"cancelled","started_at_ms":1000,"completed_at_ms":3400,"elapsed_ms":2400}}}});
        assert_eq!(step_state(&call, false), StepState::Cancelled);
        assert_eq!(step_duration(&call).as_deref(), Some("2秒"));
        assert_eq!(round_elapsed(&[(0, call)]), Some(2));
    }
    #[test]
    fn any_text_after_reasoning_settles_the_group_and_manual_choice_wins() {
        let reasoning = json!({"id":"r","type":"reasoning","role":"assistant","status":"completed","content":"summary"});
        let mut answer =
            json!({"type":"message","role":"assistant","status":"in_progress","content":"answer"});
        for phase in ["", "commentary", "final_answer"] {
            answer["phase"] = json!(phase);
            let blocks = chat_blocks(&[reasoning.clone(), answer.clone()], true, "in_progress");
            assert!(matches!(
                &blocks[0],
                ChatBlock::Process {
                    active: false,
                    finished: true,
                    ..
                }
            ));
        }
        let open = process_is_open(Some(true), true);
        assert!(open, "manual inspection must survive automatic collapse");
        assert!(!process_is_open(None, true));
    }
}
