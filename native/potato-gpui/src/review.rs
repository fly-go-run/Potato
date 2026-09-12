//! Opt-in, debug-only visual replay. Requires an explicitly isolated data dir.
//! Fixtures use the same stream reducer and completion path as the chat UI.
use crate::*;

pub fn load(app: &mut Potato, window: &mut Window, cx: &mut Context<Potato>) {
    let Some(path) = std::env::var_os("POTATO_GPUI_REVIEW_FIXTURE") else {
        return;
    };
    if std::env::var_os("POTATO_NATIVE_DATA_DIR").is_none() {
        return;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(fixture) = serde_json::from_slice::<Value>(&bytes) else {
        return;
    };
    if let Some(kind) = fixture["composer_menu"].as_str() {
        app.providers = array(fixture["providers"].clone());
        app.model = fixture["active_model"].clone();
        if kind == "effort" {
            app.prepare_effort(window, cx);
            app.menu = Some(Menu::Effort);
        } else if kind == "model" {
            app.menu = Some(Menu::Model);
        } else if kind == "permissions" {
            app.menu = Some(Menu::Permission);
        }
    }
    app.session = "visual-review".into();
    app.selected = Some("visual-review".into());
    if let Some(title) = fixture["title"].as_str() {
        app.chats = vec![json!({"id":"visual-review","session_id":"visual-review","name":title})];
    }
    app.history = array(fixture["history"].clone());
    if fixture["approvals"].is_array() {
        app.interactions.approvals = array(fixture["approvals"].clone());
        for approval in &mut app.interactions.approvals {
            approval["root_session_id"] = json!(app.session);
            approval["session_id"] = json!(app.session);
            approval["user_id"] = json!(app.user);
            approval["created_at"] = json!(chrono::Utc::now().timestamp());
            approval["timeout_seconds"] = json!(3600);
        }
        // Visual fixtures must not be replaced by live approval polling.
        app.interactions.polling = true;
    }
    if fixture["outbox"].is_object() {
        app.outbox.session = app.session.clone();
        app.outbox.value = fixture["outbox"].clone();
        app.outbox.polling = true; // Freeze opt-in visual fixture, never dispatch it.
        app.outbox.menu = fixture["outbox_menu"].as_str().map(str::to_owned);
    }
    if fixture["shell_jobs"].is_array() {
        app.interactions.shell_jobs = array(fixture["shell_jobs"].clone());
        app.interactions.polling = true;
        for job in &mut app.interactions.shell_jobs {
            job["session_id"] = json!(app.session);
            app.interactions
                .details
                .insert(format!("shell-recovery-{}", string(job, "job_id")));
        }
    }

    app.files.open = fixture["files_open"].as_bool().unwrap_or(false);
    if let Some(path) = fixture["project_path"].as_str() {
        app.request(
            "PUT",
            "/api/workspace/coding-project",
            json!({"path":path}),
            window,
            cx,
            |s, project, _, cx| {
                s.project = project;
                cx.notify();
            },
        );
    }
    app.chat.run_started = Some(
        std::time::Instant::now()
            - std::time::Duration::from_secs(fixture["run_elapsed_seconds"].as_u64().unwrap_or(0)),
    );
    if let Some(dark) = fixture["dark"].as_bool() {
        app.dark = dark;
        design::apply(dark, Some(window), cx);
    }
    if let Some(reduce) = fixture["reduce_motion"].as_bool() {
        cx.set_reduce_motion(reduce);
    }
    app.streaming = fixture["streaming"].as_bool().unwrap_or(false);
    // Optional initial presentation states support reproducible visual checks.
    // They do not synthesize user input or execute tools.
    for id in array(fixture["open_processes"].clone()) {
        if let Some(id) = id.as_str() {
            let key = format!("visual-review:{id}");
            app.chat.process_open.insert(key.clone(), true);
            app.chat.full_process.insert(key);
        }
    }
    for id in array(fixture["open_tools"].clone()) {
        if let Some(id) = id.as_str() {
            app.chat.expanded.insert(format!("visual-review:{id}"));
        }
    }
    if let Some(width) = fixture["window_width"].as_f64() {
        window.resize(size(
            px(width.clamp(800., 1600.) as f32),
            px(fixture["window_height"]
                .as_f64()
                .unwrap_or(760.)
                .clamp(600., 1200.) as f32),
        ));
    }
    for frame in array(fixture["initial"].clone()) {
        app.turn.apply(frame);
    }
    let frames = array(fixture["frames"].clone());
    if let Some(prompt) = fixture["start_request"].as_str() {
        // Opt-in acceptance run against the isolated data directory's provider.
        let body = backend::request_body(&app.session, &app.user, "console", prompt, Vec::new());
        app.streaming = true;
        let rx = app.backend.stream(body);
        app.listen_turn(rx, window, cx);
    }
    cx.spawn_in(window, async move |this, cx| {
        for entry in frames {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    entry["delay_ms"].as_u64().unwrap_or(0),
                ))
                .await;
            if this
                .update_in(cx, |app, _, cx| {
                    app.turn.apply(entry["frame"].clone());
                    if app.turn.terminal() {
                        app.finish_process();
                        app.streaming = false;
                        app.history.append(&mut app.turn.messages);
                    }
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}
