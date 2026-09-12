//! Real GPUI entity regressions. Every backend uses an empty, isolated directory;
//! no provider is configured and assertions run before asynchronous responses.
use crate::{Backend, Potato};
use gpui_kit::{AppContext, Context, TestAppContext, Window, gpui};
use serde_json::{Value, json};

struct ComposerControl(gpui::Entity<Potato>);

#[gpui::test]
fn timeline_completion_preserves_scrolled_reading_position(cx: &mut TestAppContext) {
    let backend = Backend::for_ui_test(
        std::env::temp_dir().join(format!("potato-reading-position-{}", uuid::Uuid::new_v4())),
    )
    .unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (app, cx) = cx.add_window_view(|window, cx| {
        let mut app = Potato::new(backend, window, cx);
        app.session = "reading-position".into();
        for n in 0..12 {
            app.history.push(json!({"id":format!("u-{n}"),"role":"user","content":format!("Earlier question {n}")}));
            app.history.push(json!({"id":format!("a-{n}"),"role":"assistant","type":"message","status":"completed","content":"Earlier answer\n\nDetails stay at the same reading position."}));
        }
        app.history.push(json!({"id":"current-user","role":"user","content":"Check the current task"}));
        app.streaming = true;
        app.turn.apply(json!({"object":"response","id":"response","status":"in_progress"}));
        app.turn.apply(json!({"object":"message","id":"live-reasoning","role":"assistant","type":"reasoning","status":"in_progress","content":[{"type":"text","text":"Checking current task"}]}));
        app
    });
    cx.run_until_parked();
    app.update(cx, |app, cx| {
        assert!(app.chat.scroll.max_offset().y > gpui::px(300.));
        app.chat.scroll_paused = true;
        app.preserve_process_reading();
        app.chat
            .scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(-300.)));
        cx.notify();
    });
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
    let before = app.read_with(cx, |app, _| app.chat.scroll.offset());
    app.update(cx, |app, cx| {
        app.turn.apply(json!({"object":"message","id":"answer","role":"assistant","type":"message","phase":"final_answer","status":"completed","content":[{"type":"text","text":"The final answer"}]}));
        app.turn.apply(json!({"object":"response","id":"response","status":"completed"}));
        app.finish_process();
        app.streaming = false;
        app.history.append(&mut app.turn.messages);
        cx.notify();
    });
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        assert_eq!(app.chat.scroll.offset(), before);
        assert_eq!(
            app.chat.process_open.get("reading-position:live-reasoning"),
            Some(&true)
        );
    });
}

#[gpui::test]
fn selected_conversation_renders_live_timeline_and_reduced_motion_disclosure(
    cx: &mut TestAppContext,
) {
    let backend = Backend::for_ui_test(
        std::env::temp_dir().join(format!("potato-timeline-{}", uuid::Uuid::new_v4())),
    )
    .unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (app, cx) = cx.add_window_view(|window, cx| {
        let mut app = Potato::new(backend, window, cx);
        let fixture: Value =
            serde_json::from_str(include_str!("../design/process-timeline/running.json")).unwrap();
        app.session = "visual-review".into();
        app.selected = Some("visual-review".into());
        app.chats = vec![json!({"id":"visual-review", "name":"回复体验优化"})];
        app.history = fixture["history"].as_array().unwrap().clone();
        app.streaming = true;
        for frame in fixture["initial"].as_array().unwrap() {
            app.turn.apply(frame.clone());
        }
        app
    });
    cx.run_until_parked();
    for open in [false, true] {
        app.update(cx, |app, cx| {
            app.chat
                .process_open
                .insert("visual-review:r1".into(), open);
            cx.notify();
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(app.chat.process_open.get("visual-review:r1"), Some(&open))
        });
    }
}
impl gpui::Render for ComposerControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        use gpui_kit::{InteractiveElement, ParentElement, Styled, div, px};
        div().flex().flex_col().items_start().child(
            div()
                .w(px(760.))
                .debug_selector(|| "composer-size".into())
                .child(self.0.update(cx, |app, cx| app.composer_view(window, cx))),
        )
    }
}

#[gpui::test]
fn composer_grows_for_wrapped_input_caps_and_shrinks_after_clear(cx: &mut TestAppContext) {
    let backend = Backend::for_ui_test(
        std::env::temp_dir().join(format!("potato-composer-{}", uuid::Uuid::new_v4())),
    )
    .unwrap();
    cx.update(gpui_kit::init);
    let (control, cx) = cx.add_window_view(|window, cx| {
        ComposerControl(cx.new(|cx| Potato::new(backend, window, cx)))
    });
    let home = cx.debug_bounds("composer-size").unwrap().size.height;
    assert!(home >= gpui::px(140.), "home composer: {home:?}");
    let app = control.read_with(cx, |control, _| control.0.clone());
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.history
                .push(serde_json::json!({"role": "user", "content": "你好"}));
            cx.notify();
        });
        window.refresh();
    });
    cx.run_until_parked();
    let empty = cx.debug_bounds("composer-size").unwrap().size.height;
    assert!(empty <= gpui::px(104.), "conversation composer: {empty:?}");
    let mut set_text = |text: String| {
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.composer
                    .update(cx, |input, cx| input.set_value(text, window, cx))
            });
            window.refresh();
        });
        cx.run_until_parked();
        cx.debug_bounds("composer-size").unwrap().size.height
    };
    let wrapped = set_text("连续文字自动换行，".repeat(35));
    assert!(wrapped > empty, "soft wrapping must increase input height");
    let capped = set_text("line\n".repeat(20));
    assert!(
        capped <= gpui::px(260.),
        "long input must preserve chat space: {capped:?}"
    );
    assert_eq!(set_text("line\n".repeat(60)), capped);
    assert_eq!(set_text(String::new()), empty);
}

struct ProcessControl {
    app: gpui::Entity<Potato>,
    message: Value,
}
impl gpui::Render for ProcessControl {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.app.update(cx, |app, cx| {
            app.process_view(
                0,
                &[(0, self.message.clone())],
                "completed",
                true,
                Some(176),
                false,
                cx,
            )
        })
    }
}

#[gpui::test]
fn process_summary_mouse_click_toggles_same_stable_round(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, point, px};
    let backend = Backend::for_ui_test(
        std::env::temp_dir().join(format!("potato-gpui-click-{}", uuid::Uuid::new_v4())),
    )
    .unwrap();
    cx.update(gpui_kit::init);
    let (control, cx) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| Potato::new(backend, window, cx));
        let message =
            json!({"id":"intro", "role":"assistant", "type":"message", "content":"checking"});
        app.update(cx, |app, _| {
            app.session = "click-review".into();
            app.history = vec![message.clone()];
        });
        ProcessControl { app, message }
    });
    for expected in [true, false] {
        cx.simulate_mouse_move(point(px(60.), px(16.)), None, Modifiers::none());
        cx.simulate_click(point(px(60.), px(16.)), Modifiers::none());
        control.read_with(cx, |control, cx| {
            let app = control.app.read(cx);
            assert_eq!(
                app.chat.process_open.get("click-review:intro"),
                Some(&expected)
            );
            assert!(app.chat.scroll_paused);
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn with_potato(
    cx: &mut TestAppContext,
    test: impl FnOnce(&mut Potato, &mut Window, &mut Context<Potato>),
) {
    let directory =
        std::env::temp_dir().join(format!("potato-gpui-state-{}", uuid::Uuid::new_v4()));
    let backend = Backend::for_ui_test(directory).unwrap();
    cx.update(gpui_kit::init);
    let window = cx.add_empty_window();
    window.update(|window, cx| {
        let app = cx.new(|cx| Potato::new(backend, window, cx));
        app.update(cx, |app, cx| test(app, window, cx));
    });
}

fn draft(
    app: &mut Potato,
    text: &str,
    attachment: &str,
    window: &mut Window,
    cx: &mut Context<Potato>,
) {
    app.composer
        .update(cx, |input, cx| input.set_value(text.to_owned(), window, cx));
    app.attachments = vec![json!({"type":"image", "url": attachment})];
}

fn assert_draft(app: &Potato, text: &str, attachment: &str, cx: &Context<Potato>) {
    assert_eq!(app.composer.read(cx).value().as_ref(), text);
    assert_eq!(
        app.attachments,
        vec![json!({"type":"image", "url": attachment})]
    );
}

fn existing_chat() -> Value {
    json!({"id":"local-fixture", "session_id":"existing-session", "user_id":"default", "channel":"console"})
}

fn previous_message() -> Value {
    json!({"id":"previous", "role":"user", "content":[
        {"type":"text", "text":"previous prompt"},
        {"type":"image", "url":"previous-image"}
    ]})
}

#[gpui::test]
fn new_chat_roundtrip_restores_text_and_attachments(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        draft(app, "new conversation draft", "new-image", window, cx);
        app.select_chat(existing_chat(), window, cx);
        assert!(app.composer.read(cx).value().is_empty());
        assert!(app.attachments.is_empty());
        draft(
            app,
            "existing conversation draft",
            "existing-image",
            window,
            cx,
        );
        app.new_chat(window, cx);
        assert_draft(app, "new conversation draft", "new-image", cx);
        app.select_chat(existing_chat(), window, cx);
        assert_draft(app, "existing conversation draft", "existing-image", cx);
    });
}

#[gpui::test]
fn regenerating_answer_with_trailing_reasoning_restores_unsent_draft(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        draft(app, "keep my draft", "keep-image", window, cx);
        app.history = vec![
            previous_message(),
            json!({"id":"answer", "role":"assistant", "type":"message", "status":"completed", "content":"answer"}),
            json!({"id":"reasoning", "role":"assistant", "type":"reasoning", "status":"completed", "content":"summary"}),
            json!({"id":"empty", "role":"assistant", "type":"message", "content":""}),
        ];
        app.model = json!({"model":"unconfigured-test-model"});
        app.reuse_chat_message(1, true, window, cx);
        assert!(app.streaming);
        assert_draft(app, "keep my draft", "keep-image", cx);
        assert!(app.chat.edit_backup.is_none());
    });
}

#[gpui::test]
fn cancelling_message_edit_restores_unsent_draft(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        draft(app, "unsent draft", "unsent-image", window, cx);
        app.history = vec![previous_message()];
        app.reuse_chat_message(0, false, window, cx);
        assert_draft(app, "previous prompt", "previous-image", cx);
        draft(app, "edited old message", "edited-image", window, cx);
        app.cancel_chat_edit(window, cx);
        assert_draft(app, "unsent draft", "unsent-image", cx);
        assert!(app.chat.edit_backup.is_none());
        assert_eq!(app.history, vec![previous_message()]);
    });
}

#[gpui::test]
fn sending_edited_message_restores_unsent_draft(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        draft(app, "unsent draft", "unsent-image", window, cx);
        app.history = vec![previous_message()];
        app.reuse_chat_message(0, false, window, cx);
        draft(app, "edited prompt", "edited-image", window, cx);
        // Satisfy only the UI guard. The isolated core has no provider or key.
        app.model = json!({"model":"unconfigured-test-model"});
        app.send(window, cx);
        assert_draft(app, "unsent draft", "unsent-image", cx);
        assert!(app.chat.edit_backup.is_none());
        assert_eq!(app.history.len(), 2);
        assert_eq!(
            app.history[1]["content"],
            json!([
                {"type":"text", "text":"edited prompt"},
                {"type":"image", "url":"edited-image"}
            ])
        );
    });
}

#[gpui::test]
fn switching_conversation_during_edit_preserves_original_draft(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        app.session = "source-session".into();
        app.selected = Some("source-chat".into());
        app.history = vec![previous_message()];
        draft(app, "source draft", "source-image", window, cx);
        app.reuse_chat_message(0, false, window, cx);
        app.select_chat(existing_chat(), window, cx);
        assert!(app.chat.edit_backup.is_none());
        assert_eq!(
            app.drafts["source-session"],
            (
                "source draft".into(),
                vec![json!({"type":"image", "url":"source-image"})]
            )
        );
    });
}

#[gpui::test]
fn conversation_search_does_not_replace_workspace_query(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        app.search.update(cx, |input, cx| {
            input.set_value("workspace filter", window, cx)
        });
        app.conversation_search.update(cx, |input, cx| {
            input.set_value("conversation filter", window, cx)
        });
        app.select_chat(existing_chat(), window, cx);
        app.new_chat(window, cx);
        assert_eq!(app.search.read(cx).value().as_ref(), "workspace filter");
        assert_eq!(
            app.conversation_search.read(cx).value().as_ref(),
            "conversation filter"
        );
    });
}

#[gpui::test]
fn reading_live_process_survives_completion_without_opening_old_history(cx: &mut TestAppContext) {
    with_potato(cx, |app, _, _| {
        app.session = "reading".into();
        app.history = vec![
            json!({"id":"old-user", "role":"user", "content":"old"}),
            json!({"id":"old-tool", "role":"assistant", "type":"function_call", "content":[]}),
            json!({"id":"old-answer", "role":"assistant", "content":"done"}),
            json!({"id":"new-user", "role":"user", "content":"new"}),
        ];
        app.streaming = true;
        app.turn.messages = vec![
            json!({"id":"live", "role":"assistant", "phase":"commentary", "content":"checking"}),
        ];
        app.preserve_process_reading();
        assert!(!app.chat.process_open.contains_key("reading:old-tool"));
        assert!(app.chat.process_open["reading:live"]);
        app.turn.status = "completed".into();
        app.chat.scroll_paused = true;
        app.finish_process();
        assert!(crate::chat::process_is_open(
            app.chat.process_open.get("reading:live").copied(),
            true
        ));
        app.chat.process_open.insert("reading:live".into(), false);
        app.finish_process();
        assert!(!crate::chat::process_is_open(
            app.chat.process_open.get("reading:live").copied(),
            true
        ));
        assert!(!crate::chat::process_is_open(None, true));
    });
}

#[gpui::test]
fn archive_dialog_preserves_chat_draft_and_independent_search(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        app.selected = Some("current".into());
        app.history = vec![previous_message()];
        app.search_open = true;
        app.conversation_search
            .update(cx, |input, cx| input.set_value("sidebar query", window, cx));
        draft(app, "unsent draft", "image", window, cx);
        app.open_archive(window, cx);
        app.archive_search
            .update(cx, |input, cx| input.set_value("archive query", window, cx));
        app.new_chat(window, cx);
        assert_eq!(app.selected.as_deref(), Some("current"));
        assert_eq!(app.history, vec![previous_message()]);
        assert_draft(app, "unsent draft", "image", cx);
        app.close_archive(window, cx);
        assert_eq!(
            app.conversation_search.read(cx).value().as_ref(),
            "sidebar query"
        );
        assert!(app.search_open);
        assert!(app.archive_trigger_focus.is_focused(window));
        app.new_chat(window, cx);
        assert!(app.selected.is_none());
        assert!(!app.conversations.archive_open);
    });
}

#[gpui::test]
fn archives_load_separately_restore_persistently_and_keep_failed_rows(cx: &mut TestAppContext) {
    let directory =
        std::env::temp_dir().join(format!("potato-archive-test-{}", uuid::Uuid::new_v4()));
    let id = "archived-fixture";
    let journal = directory
        .join("workspace/history/sessions")
        .join(uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, id.as_bytes()).to_string())
        .join("transcript.jsonl");
    std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
    let spec = json!({"id":id,"session_id":"archived-session","name":"Archived fixture","archived":true,"pinned":false});
    std::fs::write(
        &journal,
        format!(
            "{}\n",
            json!({"format":"potato-transcript-v1","text_refs":[],
        "record":{"version":1,"events":[{"type":"session","spec":spec}]}})
        ),
    )
    .unwrap();
    let backend = Backend::for_ui_test(directory).unwrap();
    let request = |method: &str, path: &str, body| {
        backend
            .executor
            .block_on(backend.request(method, path, body))
            .unwrap()
    };
    let archived = request("GET", "/api/chats?archived=true", Value::Null).unwrap();
    assert_eq!(archived.as_array().unwrap().len(), 1);
    assert!(
        request("GET", "/api/chats", Value::Null)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    let restored = request(
        "PUT",
        &format!("/api/chats/{id}"),
        json!({"archived":false}),
    );
    assert!(restored.is_ok());
    assert!(
        request("GET", "/api/chats?archived=true", Value::Null)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    let failed = request("PUT", "/api/chats/missing", json!({"archived":false}));
    assert!(failed.is_err());
    // Deliver real backend outcomes synchronously: GPUI's deterministic executor
    // cannot accept wakes from the separate Tokio runtime used by this adapter.
    with_potato(cx, |app, window, cx| {
        app.selected = Some("keep-current".into());
        app.conversations.archive_open = true;
        app.conversations.archive_rows = archived.as_array().unwrap().clone();
        app.conversations.archive_restoring.insert(id.into());
        app.finish_archive_restore(id.into(), restored, window, cx);
        assert!(app.conversations.archive_rows.is_empty());
        assert!(app.conversations.archive_restoring.is_empty());
        assert!(app.conversations.archive_open);
        assert_eq!(app.selected.as_deref(), Some("keep-current"));
        assert!(
            app.chats
                .iter()
                .any(|chat| chat["id"] == id && chat["archived"] == false)
        );
        app.conversations
            .archive_rows
            .push(json!({"id":"missing","name":"Missing archived chat"}));
        app.conversations.archive_restoring.insert("missing".into());
        app.finish_archive_restore("missing".into(), failed, window, cx);
        assert_eq!(app.conversations.archive_rows.len(), 1);
        assert!(app.conversations.archive_restoring.is_empty());
        assert!(app.conversations.archive_row_errors.contains_key("missing"));
    });
}

struct QueueControl(gpui::Entity<Potato>);
impl gpui::Render for QueueControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        use gpui_kit::{ParentElement, Styled, div, px};
        div()
            .pt(px(200.))
            .w(px(760.))
            .child(self.0.update(cx, |s, cx| s.composer_view(window, cx)))
    }
}
#[gpui::test]
fn queue_shelf_is_compact_and_hover_menu_survives_pointer_entry(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, px};
    let backend = Backend::for_ui_test(
        std::env::temp_dir().join(format!("potato-outbox-ui-{}", uuid::Uuid::new_v4())),
    )
    .unwrap();
    cx.update(gpui_kit::init);
    let (control,cx)=cx.add_window_view(|window,cx| {
        let app=cx.new(|cx|Potato::new(backend,window,cx));
        app.update(cx,|s,cx| {
            s.streaming=true;
            s.composer.update(cx,|i,cx|i.set_value("还有，顺便检查一下快捷键",window,cx));
            s.outbox.session=s.session.clone();
            s.outbox.value=json!({"items":[{"id":"one","state":"pending","request":{"input":[{"content":[{"type":"text","text":"检查工具调用"}]}]}},{"id":"two","state":"pending","request":{"input":[{"content":[{"type":"text","text":"检查语音输入"}]}]}}]});
        });
        QueueControl(app)
    });
    let shelf = cx.debug_bounds("outbox-shelf").unwrap();
    assert!(shelf.size.height <= px(84.));
    control.update(cx, |c, cx| {
        c.0.update(cx, |s, cx| {
            s.outbox.menu = Some("one".into());
            cx.notify();
        })
    });
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
    assert_eq!(
        cx.debug_bounds("outbox-shelf").unwrap(),
        shelf,
        "opening the menu must not move the composer"
    );
    assert!(cx.debug_bounds("queue-action-menu").is_some());
    control.update(cx, |c, cx| {
        c.0.update(cx, |s, cx| {
            s.outbox.menu = None;
            cx.notify();
        })
    });
    assert!(cx.debug_bounds("send-menu").is_none());
    let send = cx.debug_bounds("send-region").unwrap();
    cx.simulate_mouse_move(send.center(), None, Modifiers::none());
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
    let menu = cx
        .debug_bounds("send-menu")
        .expect("hover must reveal the menu");
    cx.simulate_mouse_move(menu.center(), None, Modifiers::none());
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
    control.read_with(cx, |c, cx| assert!(c.0.read(cx).outbox.send_hover));
}

#[gpui::test]
fn switching_during_reply_preserves_draft_and_resets_visible_run(cx: &mut TestAppContext) {
    with_potato(cx, |app, window, cx| {
        app.session = "running-session".into();
        app.selected = Some("running-chat".into());
        app.history = vec![previous_message()];
        app.streaming = true;
        app.turn.status = "in_progress".into();
        app.chat.run_started = Some(std::time::Instant::now());
        draft(app, "unsent followup", "image", window, cx);
        let epoch = app.epoch;
        app.select_chat(existing_chat(), window, cx);
        assert_eq!(app.session, "existing-session");
        assert!(!app.streaming);
        assert!(app.turn.messages.is_empty());
        assert!(app.chat.run_started.is_none());
        assert!(app.epoch > epoch);
        assert_eq!(app.drafts["running-session"].0, "unsent followup");
        assert_eq!(app.drafts["running-session"].1.len(), 1);
        app.streaming = true;
        let epoch = app.epoch;
        app.select_chat(existing_chat(), window, cx);
        assert!(
            app.streaming,
            "reselecting the current chat keeps its stream"
        );
        assert_eq!(app.epoch, epoch);
        app.new_chat(window, cx);
        assert!(app.selected.is_none());
        assert!(!app.streaming);
        assert!(app.epoch > epoch);
    });
}
