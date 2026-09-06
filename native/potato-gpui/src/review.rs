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
    app.session = "visual-review".into();
    app.selected = Some("visual-review".into());
    app.history = array(fixture["history"].clone());
    app.chat.run_started = Some(std::time::Instant::now());
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
        window.resize(size(px(width.clamp(800., 1600.) as f32), px(760.)));
    }
    for frame in array(fixture["initial"].clone()) {
        app.turn.apply(frame);
    }
    let frames = array(fixture["frames"].clone());
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
