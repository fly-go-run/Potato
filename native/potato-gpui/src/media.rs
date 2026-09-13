use crate::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::component::text::{TextView, TextViewState};
use gpui_kit::prelude::*;
use std::sync::Arc;

/// Retain Markdown independently of the element tree, including process/body
/// transitions. Only the visible conversation is cached.
#[derive(Default)]
pub struct MarkdownCache {
    entries: BTreeMap<(String, String, usize), MarkdownEntry>,
}
struct MarkdownEntry {
    state: Entity<TextViewState>,
    source: String,
    settled: bool,
}
fn identity(message: &Value, row: usize) -> String {
    message["id"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("row-{row}"))
}
fn text_parts(message: &Value) -> Vec<(usize, String)> {
    if let Some(blocks) = message["content"].as_array()
        && blocks.iter().any(|b| b["type"] == "image")
    {
        return blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b["type"] != "image")
            .map(|(i, b)| (i, crate::view::message_text(&json!({"content":[b]}))))
            .collect();
    }
    vec![(0, crate::view::message_text(message))]
}
impl MarkdownCache {
    pub fn sync(&mut self, session: &str, messages: &[Value], settled: bool, cx: &mut App) {
        let mut keep = std::collections::BTreeSet::new();
        for (row, message) in messages.iter().enumerate() {
            if !matches!(message["type"].as_str(), None | Some("message"))
                || message["role"] == "tool"
            {
                continue;
            }
            for (part, source) in text_parts(message) {
                let key = (session.to_owned(), identity(message, row), part);
                keep.insert(key.clone());
                let entry = self.entries.entry(key).or_insert_with(|| MarkdownEntry {
                    state: cx.new(|cx| TextViewState::markdown(&source, cx)),
                    source: source.clone(),
                    settled,
                });
                if source != entry.source {
                    if let Some(delta) = source.strip_prefix(entry.source.as_str()) {
                        entry.state.update(cx, |s, cx| s.push_str(delta, cx));
                    } else {
                        entry.state.update(cx, |s, cx| s.set_text(&source, cx));
                    }
                    entry.source.clone_from(&source);
                    entry.settled = false;
                }
                // Late reference definitions can change earlier blocks. The
                // library's append path reparses only its last block; normalize
                // these documents once, without swapping the retained entity.
                if settled
                    && !entry.settled
                    && source.lines().any(|line| {
                        let line = line.trim_start();
                        line.starts_with('[') && line.contains("]:")
                    })
                {
                    entry.state.update(cx, |s, cx| {
                        s.set_text("", cx);
                        s.set_text(&source, cx);
                    });
                }
                entry.settled |= settled;
            }
        }
        self.entries.retain(|key, _| keep.contains(key));
    }
    fn view(
        &self,
        session: &str,
        message: &Value,
        row: usize,
        part: usize,
        fallback: String,
    ) -> TextView {
        let key = (session.to_owned(), identity(message, row), part);
        match self.entries.get(&key) {
            Some(entry) => TextView::new(&entry.state),
            None => TextView::markdown(ElementId::Name(format!("md-{key:?}").into()), fallback),
        }
        .w_full()
        .min_w_0()
        .when(message["role"] == "assistant", |view| {
            view.style(crate::design::answer_style())
        })
    }
}

fn image_url(block: &Value) -> &str {
    block["image_url"]
        .as_str()
        .or_else(|| block["image_url"]["url"].as_str())
        .or_else(|| block["url"].as_str())
        .unwrap_or("")
}

fn image_source(url: &str) -> Result<ImageSource, &'static str> {
    if url.starts_with("https://") {
        return Ok(SharedUri::from(url.to_owned()).into());
    }
    let (header, encoded) = url.split_once(',').ok_or("无效的图片地址")?;
    let format = match header {
        "data:image/png;base64" => ImageFormat::Png,
        "data:image/jpeg;base64" => ImageFormat::Jpeg,
        "data:image/webp;base64" => ImageFormat::Webp,
        _ => return Err("不支持的图片格式"),
    };
    let bytes = STANDARD.decode(encoded).map_err(|_| "图片数据损坏")?;
    Ok(Arc::new(Image::from_bytes(format, bytes)).into())
}

pub fn image_view(block: &Value, thumbnail: bool) -> AnyElement {
    match image_source(image_url(block)) {
        Ok(source) => img(source)
            .object_fit(ObjectFit::Contain)
            .w(if thumbnail { px(96.) } else { px(640.) })
            .max_w(relative(1.))
            .h(if thumbnail { px(80.) } else { px(420.) })
            .into_any_element(),
        Err(error) => div()
            .child(format!("图片无法显示：{error}"))
            .into_any_element(),
    }
}

pub fn message_view(
    session: &str,
    cache: &MarkdownCache,
    row: usize,
    message: &Value,
    cx: &mut Context<Potato>,
) -> AnyElement {
    let Some(blocks) = message["content"]
        .as_array()
        .filter(|blocks| blocks.iter().any(|b| b["type"] == "image"))
    else {
        return cache
            .view(session, message, row, 0, crate::view::message_text(message))
            .selectable(true)
            .on_link_click(link_handler(cx))
            .into_any_element();
    };
    // Keep text and images in their original order, with stable per-block IDs.
    let mut content = div()
        .id(("media-message", row))
        .flex()
        .flex_col()
        .w_full()
        .gap_2()
        .min_w_0();
    for (index, block) in blocks.iter().enumerate() {
        content = content.child(if block["type"] == "image" {
            image_view(block, false)
        } else {
            cache
                .view(
                    session,
                    message,
                    row,
                    index,
                    crate::view::message_text(&json!({"content":[block]})),
                )
                .selectable(true)
                .on_link_click(link_handler(cx))
                .into_any_element()
        });
    }
    content.into_any_element()
}

fn link_handler(
    cx: &Context<Potato>,
) -> impl Fn(&SharedString, &ClickEvent, &mut Window, &mut App) + Send + Sync + 'static {
    let entity = cx.entity().downgrade();
    move |url, _, w, cx| {
        let _ = entity.update(cx, |s, cx| s.follow_file_link(url, w, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn embedded_images_use_bytes_and_invalid_data_is_reported() {
        assert!(matches!(
            image_source("data:image/png;base64,aGVsbG8="),
            Ok(ImageSource::Image(_))
        ));
        assert!(image_source("data:image/png;base64,!!!").is_err());
        assert!(image_source("file:///etc/passwd").is_err());
        assert_eq!(
            image_url(&json!({"image_url":{"url":"https://example.com/a.png"}})),
            "https://example.com/a.png"
        );
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::MarkdownCache;
    use gpui_kit::InteractiveElement;
    use gpui_kit::{
        Context, IntoElement, ParentElement, Render, Styled, TestAppContext, Window, div, gpui, px,
    };
    use serde_json::json;

    struct Preview {
        cache: MarkdownCache,
        session: String,
        source: String,
        settled: bool,
    }
    impl Render for Preview {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let message = json!({"id":"answer", "role":"assistant", "content":self.source});
            self.cache.sync(
                &self.session,
                std::slice::from_ref(&message),
                self.settled,
                cx,
            );
            div().w(px(400.)).child(
                div()
                    .w_full()
                    .debug_selector(|| "markdown-body".into())
                    .child(
                        self.cache
                            .view(&self.session, &message, 0, 0, self.source.clone()),
                    ),
            )
        }
    }

    #[gpui::test]
    fn streaming_body_wraps_and_keeps_entity_through_completion(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(|_, _| Preview {
            cache: MarkdownCache::default(),
            session: "a".into(),
            source: "# 标题\n\n".into(),
            settled: false,
        });
        cx.run_until_parked();
        let initial_height = cx.debug_bounds("markdown-body").unwrap().size.height;
        let key = ("a".into(), "answer".into(), 0);
        let entity = view.read_with(cx, |v, _| v.cache.entries[&key].state.entity_id());
        let retained = view.read_with(cx, |v, _| v.cache.entries[&key].state.clone());
        retained.update(cx, |state, cx| state.select_all(cx));
        let paragraph = "这是测试中文长段落换行、持续输出和文字选择的内容。".repeat(20);
        for chunk in [
            paragraph.as_str(),
            "\n\n```rust\n",
            "fn main() {}\n",
            "```\n\n| 名称 | 值 |\n| --- | --- |\n| 中文 | 123 |\n",
        ] {
            view.update(cx, |v, cx| {
                v.source.push_str(chunk);
                cx.notify();
            });
            cx.run_until_parked();
        }
        let live_bounds = cx.debug_bounds("markdown-body").unwrap();
        assert!(
            live_bounds.size.height > initial_height + px(200.),
            "long text must wrap vertically"
        );
        assert_eq!(live_bounds.size.width, px(400.));
        view.update(cx, |v, cx| {
            v.settled = true;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("markdown-body").unwrap(),
            live_bounds,
            "completion must not move or resize the body"
        );
        view.read_with(cx, |v, _| {
            assert_eq!(v.cache.entries[&key].state.entity_id(), entity)
        });
        retained.read_with(cx, |state, _| {
            assert!(
                !state.selected_text().is_empty(),
                "streaming and completion must preserve selection"
            )
        });
        // A corrected snapshot replaces rather than duplicates the old text.
        view.update(cx, |v, cx| {
            v.source = "短回复".into();
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("markdown-body").unwrap().size.height < live_bounds.size.height);
        view.read_with(cx, |v, _| {
            assert_eq!(v.cache.entries[&key].source, "短回复")
        });
    }

    #[gpui::test]
    fn late_reference_definition_is_corrected_without_replacing_entity(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut cache = MarkdownCache::default();
        let key = ("a".into(), "answer".into(), 0);
        let first = json!({"id":"answer", "content":"[site][ref]\n\nAnother paragraph.\n\n"});
        cx.update(|cx| cache.sync("a", &[first], false, cx));
        cx.run_until_parked();
        let id = cache.entries[&key].state.entity_id();
        let source = "[site][ref]\n\nAnother paragraph.\n\n[ref]: https://example.com\n";
        let complete = json!({"id":"answer", "content":source});
        cx.update(|cx| cache.sync("a", std::slice::from_ref(&complete), false, cx));
        cx.run_until_parked();
        cx.update(|cx| cache.sync("a", &[complete], true, cx));
        cx.run_until_parked();
        let state = cache.entries[&key].state.clone();
        state.update(cx, |s, cx| s.select_all(cx));
        let text = state.read_with(cx, |s, _| s.selected_text());
        assert!(
            text.contains("site") && !text.contains("[ref]"),
            "late link did not resolve: {text:?}"
        );
        assert_eq!(state.entity_id(), id);
    }

    #[gpui::test]
    fn cache_follows_message_identity_not_row_and_prunes_other_sessions(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut cache = MarkdownCache::default();
        let answer = json!({"id":"answer", "content":"reply"});
        cx.update(|cx| cache.sync("a", std::slice::from_ref(&answer), false, cx));
        let key = ("a".into(), "answer".into(), 0);
        let id = cache.entries[&key].state.entity_id();
        cx.update(|cx| {
            cache.sync(
                "a",
                &[json!({"id":"user","content":"question"}), answer.clone()],
                false,
                cx,
            )
        });
        assert_eq!(cache.entries[&key].state.entity_id(), id);
        cx.update(|cx| cache.sync("b", &[answer], true, cx));
        assert_eq!(cache.entries.len(), 1);
        assert!(!cache.entries.contains_key(&key));
        assert_ne!(
            cache.entries[&("b".into(), "answer".into(), 0)]
                .state
                .entity_id(),
            id
        );
    }
}
