//! Compile-only integration probe. Not wired into the application.
use gpui_kit::{AppContext, Context, Entity};
use gpui_kit::component::text::{TextView, TextViewState};

pub struct StreamingMarkdown {
    view: Entity<TextViewState>,
    source: String,
}
impl StreamingMarkdown {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self { view: cx.new(|cx| TextViewState::markdown("", cx)), source: String::new() }
    }
    pub fn sync_snapshot(&mut self, source: &str, cx: &mut Context<Self>) {
        if source == self.source { return; }
        if let Some(delta) = source.strip_prefix(self.source.as_str()) {
            self.view.update(cx, |state, cx| state.push_str(delta, cx));
        } else {
            self.view.update(cx, |state, cx| state.set_text(source, cx));
        }
        self.source.clear();
        self.source.push_str(source);
    }
    pub fn element(&self) -> TextView {
        TextView::new(&self.view).selectable(true)
    }
}
