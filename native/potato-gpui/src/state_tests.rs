//! Real GPUI entity regressions. Every backend uses an empty, isolated directory;
//! no provider is configured and assertions run before asynchronous responses.
use crate::{Backend, Potato};
use gpui_kit::{AppContext, Context, TestAppContext, Window, gpui};
use serde_json::{Value, json};

struct ComposerControl(gpui::Entity<Potato>);
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
    let backend = Backend::open_at(
        std::env::temp_dir().join(format!("potato-composer-{}", uuid::Uuid::new_v4())),
    )
    .unwrap();
    cx.update(gpui_kit::init);
    let (control, cx) = cx.add_window_view(|window, cx| {
        ComposerControl(cx.new(|cx| Potato::new(backend, window, cx)))
    });
    let empty = cx.debug_bounds("composer-size").unwrap().size.height;
    assert!(empty <= gpui::px(104.), "empty composer: {empty:?}");
    let app = control.read_with(cx, |control, _| control.0.clone());
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
                cx,
            )
        })
    }
}

#[gpui::test]
fn process_summary_mouse_click_toggles_same_stable_round(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, point, px};
    let backend = Backend::open_at(
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
    let backend = Backend::open_at(directory).unwrap();
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
        app.turn.messages = vec![json!({"id":"live", "role":"assistant", "content":"checking"})];
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
    let backend = Backend::open_at(directory).unwrap();
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
