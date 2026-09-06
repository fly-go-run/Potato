//! Semantic controls for Iced's canvas UI. Platform actions use the same messages as clicks.
use crate::Message;
use accesskit::{Action, Node, NodeId, Rect, Role, Tree as AxTree, TreeId, TreeUpdate};
use iced::advanced::{
    layout, mouse, overlay, renderer,
    widget::{self, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{Element, Event, Length, Padding, Rectangle, Renderer, Size, Theme, Vector};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

pub mod button {
    pub use iced::widget::button::*;
}
pub(crate) struct Button<'a> {
    inner: iced::widget::Button<'a, Message>,
    action: Option<Message>,
}
pub fn button<'a>(content: impl Into<Element<'a, Message>>) -> Button<'a> {
    Button {
        inner: iced::widget::button(content),
        action: None,
    }
}
impl<'a> Button<'a> {
    pub(crate) fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(value);
        self
    }
    pub(crate) fn width(mut self, value: impl Into<Length>) -> Self {
        self.inner = self.inner.width(value);
        self
    }
    pub(crate) fn style(
        mut self,
        value: impl Fn(&Theme, button::Status) -> button::Style + 'a,
    ) -> Self {
        self.inner = self.inner.style(value);
        self
    }
    pub(crate) fn on_press(self, value: Message) -> Self {
        self.on_press_maybe(Some(value))
    }
    pub(crate) fn on_press_maybe(mut self, value: Option<Message>) -> Self {
        self.inner = self.inner.on_press_maybe(value.clone());
        self.action = value;
        self
    }
}
impl<'a> From<Button<'a>> for Element<'a, Message> {
    fn from(value: Button<'a>) -> Self {
        Element::new(Semantic {
            content: value.inner.into(),
            kind: Kind::Button(value.action),
        })
    }
}
type InputAction = std::sync::Arc<dyn Fn(String) -> Message + Send + Sync>;
enum Kind {
    Choice {
        label: String,
        value: String,
        options: Vec<(String, Message)>,
    },
    Root,
    Modal,
    Button(Option<Message>),
    Input {
        label: String,
        value: String,
        secure: bool,
        id: widget::Id,
        action: Option<InputAction>,
    },
}
pub fn input<'a>(
    content: Element<'a, Message>,
    label: &str,
    value: &str,
    secure: bool,
    id: widget::Id,
    action: Option<impl Fn(String) -> Message + Send + Sync + 'static>,
) -> Element<'a, Message> {
    Element::new(Semantic {
        content,
        kind: Kind::Input {
            label: label.into(),
            value: if secure { String::new() } else { value.into() },
            secure,
            id,
            action: action.map(|a| std::sync::Arc::new(a) as InputAction),
        },
    })
}
pub fn root(content: Element<'_, Message>) -> Element<'_, Message> {
    Element::new(Semantic {
        content,
        kind: Kind::Root,
    })
}
pub fn modal(content: Element<'_, Message>) -> Element<'_, Message> {
    Element::new(Semantic {
        content,
        kind: Kind::Modal,
    })
}
struct Semantic<'a> {
    content: Element<'a, Message>,
    kind: Kind,
}
static NEXT: AtomicU64 = AtomicU64::new(2);
struct State {
    id: NodeId,
    label: String,
    bounds: Option<Rectangle>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            id: NodeId(NEXT.fetch_add(1, Ordering::Relaxed)),
            label: String::new(),
            bounds: None,
        }
    }
}
#[derive(Default)]
struct Frame {
    nodes: Vec<(NodeId, Node)>,
    actions: HashMap<NodeId, Message>,
    inputs: HashMap<NodeId, (widget::Id, InputAction)>,
    focus: Option<NodeId>,
    choices: HashMap<NodeId, Vec<(String, Message)>>,
    scale: f32,
    snapshot: Option<TreeUpdate>,
}
thread_local! { static FRAME: RefCell<Frame> = RefCell::new(Frame::default()); }
#[cfg(target_os = "macos")]
thread_local! { static ADAPTER: RefCell<Option<accesskit_macos::SubclassingAdapter>> = const { RefCell::new(None) }; }
fn reset() {
    FRAME.with(|f| {
        let mut f = f.borrow_mut();
        f.nodes.clear();
        f.actions.clear();
        f.inputs.clear();
        f.choices.clear();
    });
}
fn finish(bounds: Rectangle) {
    let snapshot = FRAME.with(|f| {
        let mut f = f.borrow_mut();
        let mut root = Node::new(Role::Window);
        root.set_label("Potato Native");
        root.set_bounds(rect(bounds));
        root.set_transform(accesskit::Affine::scale(f.scale.max(1.) as f64));
        root.set_children(f.nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>());
        if !f
            .focus
            .is_some_and(|id| f.nodes.iter().any(|(n, _)| *n == id))
        {
            f.focus = None;
        }
        let mut nodes = vec![(NodeId(1), root)];
        nodes.extend(f.nodes.clone());
        let snapshot = TreeUpdate {
            nodes,
            tree: Some(AxTree::new(NodeId(1))),
            tree_id: TreeId::ROOT,
            focus: f.focus.unwrap_or(NodeId(1)),
        };
        f.snapshot = Some(snapshot.clone());
        snapshot
    });
    #[cfg(target_os = "macos")]
    ADAPTER.with(|adapter| {
        let events = adapter
            .borrow_mut()
            .as_mut()
            .and_then(|a| a.update_if_active(|| snapshot));
        if let Some(events) = events {
            events.raise();
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = snapshot;
}
fn rect(r: Rectangle) -> Rect {
    Rect::new(
        r.x as f64,
        r.y as f64,
        (r.x + r.width) as f64,
        (r.y + r.height) as f64,
    )
}
fn fallback(action: &Option<Message>) -> &'static str {
    match action {
        Some(Message::NewChat) => "新建会话",
        Some(Message::Collapse) => "收起或展开侧栏",
        Some(Message::Search) => "搜索会话",
        Some(Message::Settings) => "设置",
        Some(Message::Theme) => "切换主题",
        Some(Message::Copy(_)) => "复制",
        Some(Message::Submit) => "发送",
        Some(Message::Stop) => "停止",
        Some(Message::Media(crate::media::Event::Pick)) => "添加附件",
        Some(Message::Media(crate::media::Event::Remove(_))) => "移除附件",
        Some(Message::Voice(_)) => "语音输入",
        Some(Message::ReuseTurn(_, true)) => "编辑重发",
        Some(Message::ReuseTurn(_, false)) => "重新生成",
        Some(Message::Preferences(crate::settings::Event::Navigate(
            crate::settings::Destination::Close,
        ))) => "关闭设置",
        Some(Message::Conversation(crate::conversations::Event::Menu(_))) => "会话操作",
        _ => "操作",
    }
}
#[cfg(target_os = "macos")]
struct Handler;
#[cfg(target_os = "macos")]
impl accesskit::ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        FRAME.with(|f| f.borrow().snapshot.clone())
    }
}
#[cfg(target_os = "macos")]
impl accesskit::ActionHandler for Handler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        FRAME.with(|f| {
            let mut f = f.borrow_mut();
            if request.action == Action::SetValue {
                if let Some(options) = f.choices.get(&request.target_node) {
                    if let Some(accesskit::ActionData::Value(value)) = &request.data {
                        if let Some((_, action)) =
                            options.iter().find(|(label, _)| label == value.as_ref())
                        {
                            send(action.clone());
                        }
                    }
                    return;
                }
            }
            if let Some((id, action)) = f.inputs.get(&request.target_node).cloned() {
                match request.action {
                    Action::Focus | Action::Click => {
                        f.focus = Some(request.target_node);
                        send(Message::AccessibilityFocus(id));
                    }
                    Action::SetValue => {
                        if let Some(accesskit::ActionData::Value(value)) = request.data {
                            send(action(value.into()));
                        }
                    }
                    _ => {}
                }
                return;
            }
            if let Some(message) = f.actions.get(&request.target_node).cloned() {
                match request.action {
                    Action::Click => send(message),
                    Action::Focus => {
                        f.focus = Some(request.target_node);
                        send(Message::AccessibilityFocus(widget::Id::new(
                            "a11y-button-focus",
                        )));
                    }
                    _ => {}
                }
            }
        });
    }
}
pub fn attach(handle: raw_window_handle::RawWindowHandle) {
    #[cfg(target_os = "macos")]
    if let raw_window_handle::RawWindowHandle::AppKit(handle) = handle {
        ADAPTER.with(|slot| {
            if slot.borrow().is_none() {
                // Called by Iced's main-thread window callback before set_mode(Windowed).
                let adapter = unsafe {
                    accesskit_macos::SubclassingAdapter::new(
                        handle.ns_view.as_ptr(),
                        Handler,
                        Handler,
                    )
                };
                *slot.borrow_mut() = Some(adapter);
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}
pub fn window_focus(focused: bool) {
    #[cfg(target_os = "macos")]
    ADAPTER.with(|slot| {
        let events = slot
            .borrow_mut()
            .as_mut()
            .and_then(|a| a.update_view_focus_state(focused));
        if let Some(events) = events {
            events.raise();
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = focused;
}
pub fn set_scale(scale: f32) {
    FRAME.with(|f| f.borrow_mut().scale = scale);
}
static ACTIONS: std::sync::Mutex<Option<iced::futures::channel::mpsc::UnboundedSender<Message>>> =
    std::sync::Mutex::new(None);
#[cfg(target_os = "macos")]
fn send(message: Message) {
    if let Ok(sender) = ACTIONS.lock() {
        if let Some(sender) = sender.as_ref() {
            let _ = sender.unbounded_send(message);
        }
    }
}
pub fn subscription() -> iced::Subscription<Message> {
    iced::Subscription::run(|| {
        iced::stream::channel(
            32,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                use futures_util::{SinkExt, StreamExt};
                let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded();
                if let Ok(mut slot) = ACTIONS.lock() {
                    *slot = Some(sender);
                }
                while let Some(message) = receiver.next().await {
                    if output.send(message).await.is_err() {
                        break;
                    }
                }
            },
        )
    })
}
#[derive(Default)]
struct Labels(String);
impl widget::Operation for Labels {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        operate(self);
    }
    fn text(&mut self, _: Option<&widget::Id>, _: Rectangle, text: &str) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(text);
    }
}
// Iced scrolls via renderer translations. Capture the corresponding logical
// screen rectangles during tree traversal so AX hit testing matches what is drawn.
struct Geometry {
    offset: Vector,
    viewport: Rectangle,
    pending: Option<(Rectangle, Vector)>,
}
impl widget::Operation for Geometry {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
        let previous = (self.offset, self.viewport);
        if let Some((bounds, translation)) = self.pending.take() {
            self.viewport = self
                .viewport
                .intersection(&(bounds + self.offset))
                .unwrap_or_default();
            self.offset -= translation;
        }
        operate(self);
        (self.offset, self.viewport) = previous;
    }
    fn scrollable(
        &mut self,
        _: Option<&widget::Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn widget::operation::Scrollable,
    ) {
        self.pending = Some((bounds, translation));
    }
    fn custom(&mut self, _: Option<&widget::Id>, bounds: Rectangle, state: &mut dyn std::any::Any) {
        if let Some(state) = state.downcast_mut::<State>() {
            state.bounds = (bounds + self.offset).intersection(&self.viewport);
        }
    }
}
impl Widget<Message, Theme, Renderer> for Semantic<'_> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        if matches!(self.kind, Kind::Button(_)) {
            let mut labels = Labels::default();
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
                &mut labels,
            );
            tree.state.downcast_mut::<State>().label = labels.0;
        }
        if matches!(self.kind, Kind::Root) {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
                &mut Geometry {
                    offset: Vector::ZERO,
                    viewport: Rectangle::with_size(node.size()),
                    pending: None,
                },
            );
        }
        node
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.custom(None, layout.bounds(), tree.state.downcast_mut::<State>());
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if matches!(self.kind, Kind::Root) {
            if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            {
                use iced::keyboard::{key::Named, Key};
                if *key == Key::Named(Named::Tab) && !modifiers.command() {
                    FRAME.with(|f| {
                        let mut f = f.borrow_mut();
                        let ids: Vec<_> = f
                            .nodes
                            .iter()
                            .filter_map(|(id, _)| {
                                (f.actions.contains_key(id) || f.inputs.contains_key(id))
                                    .then_some(*id)
                            })
                            .collect();
                        if !ids.is_empty() {
                            let at = f.focus.and_then(|id| ids.iter().position(|n| *n == id));
                            let next = if modifiers.shift() {
                                at.map(|i| (i + ids.len() - 1) % ids.len())
                                    .unwrap_or(ids.len() - 1)
                            } else {
                                at.map(|i| (i + 1) % ids.len()).unwrap_or(0)
                            };
                            f.focus = Some(ids[next]);
                            let id = f
                                .inputs
                                .get(&ids[next])
                                .map(|(id, _)| id.clone())
                                .unwrap_or_else(|| widget::Id::new("a11y-button-focus"));
                            shell.publish(Message::AccessibilityFocus(id));
                        }
                    });
                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }
                if *key == Key::Named(Named::Enter) || *key == Key::Named(Named::Space) {
                    let action = FRAME.with(|f| {
                        let f = f.borrow();
                        f.focus.and_then(|id| f.actions.get(&id).cloned())
                    });
                    if let Some(action) = action {
                        shell.publish(action);
                        shell.capture_event();
                        return;
                    }
                }
            }
            if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_))) {
                FRAME.with(|f| f.borrow_mut().focus = None);
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if matches!(self.kind, Kind::Root) {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                &mut Geometry {
                    offset: Vector::ZERO,
                    viewport: *viewport,
                    pending: None,
                },
            );
        }
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if matches!(self.kind, Kind::Root | Kind::Modal) {
            reset();
        }
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if let Kind::Choice {
            label,
            value,
            options,
        } = &self.kind
        {
            if let Some(bounds) = layout.bounds().intersection(viewport) {
                let state = tree.state.downcast_ref::<State>();
                let mut node = Node::new(Role::ComboBox);
                node.set_label(label.clone());
                node.set_value(value.clone());
                node.set_bounds(rect(state.bounds.unwrap_or(bounds)));
                node.add_action(Action::Click);
                node.add_action(Action::Focus);
                node.add_action(Action::SetValue);
                FRAME.with(|f| {
                    let mut f = f.borrow_mut();
                    f.nodes.push((state.id, node));
                    f.choices.insert(state.id, options.clone());
                    f.actions.insert(
                        state.id,
                        Message::AccessibilityOptions(Some(options.clone())),
                    );
                });
            }
        }
        if let Kind::Input {
            label,
            value,
            secure,
            id,
            action,
        } = &self.kind
        {
            if let Some(bounds) = layout
                .bounds()
                .intersection(viewport)
                .filter(|b| b.width > 0. && b.height > 0.)
            {
                let state = tree.state.downcast_ref::<State>();
                let mut node = Node::new(if *secure {
                    Role::PasswordInput
                } else {
                    Role::TextInput
                });
                node.set_label(label.clone());
                if !secure {
                    node.set_value(value.clone());
                }
                node.set_bounds(rect(state.bounds.unwrap_or(bounds)));
                if action.is_some() {
                    node.add_action(Action::Focus);
                    node.add_action(Action::Click);
                    node.add_action(Action::SetValue);
                } else {
                    node.set_disabled();
                }
                FRAME.with(|f| {
                    let mut f = f.borrow_mut();
                    f.nodes.push((state.id, node));
                    if let Some(action) = action {
                        f.inputs.insert(state.id, (id.clone(), action.clone()));
                    }
                });
            }
        }
        if let Kind::Button(action) = &self.kind {
            if let Some(bounds) = layout
                .bounds()
                .intersection(viewport)
                .filter(|b| b.width > 0. && b.height > 0.)
            {
                let state = tree.state.downcast_ref::<State>();
                let mut node = Node::new(Role::Button);
                node.set_label(
                    if state.label.trim().is_empty() || matches!(state.label.as_str(), "×" | "⋯")
                    {
                        fallback(action)
                    } else {
                        &state.label
                    },
                );
                node.set_bounds(rect(state.bounds.unwrap_or(bounds)));
                if action.is_some() {
                    node.add_action(Action::Click);
                    node.add_action(Action::Focus);
                } else {
                    node.set_disabled();
                }
                let focused = FRAME.with(|f| {
                    let mut f = f.borrow_mut();
                    f.nodes.push((state.id, node));
                    if let Some(action) = action {
                        f.actions.insert(state.id, action.clone());
                    }
                    f.focus == Some(state.id)
                });
                if focused {
                    use iced::advanced::Renderer as _;
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: iced::border::rounded(6)
                                .width(2)
                                .color(theme.palette().primary),
                            ..Default::default()
                        },
                        iced::Color::TRANSPARENT,
                    );
                }
            }
        }
        if matches!(self.kind, Kind::Root) {
            finish(layout.bounds());
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

pub mod text_input {
    pub use iced::widget::text_input::*;
}
pub(crate) struct TextInput<'a> {
    inner: iced::widget::TextInput<'a, Message>,
    label: String,
    value: String,
    secure: bool,
    id: widget::Id,
    action: Option<InputAction>,
}
pub fn text_input<'a>(placeholder: &str, value: &str) -> TextInput<'a> {
    let id = widget::Id::from(format!("input-{placeholder}"));
    TextInput {
        inner: iced::widget::text_input(placeholder, value).id(id.clone()),
        label: placeholder.into(),
        value: value.into(),
        secure: false,
        id,
        action: None,
    }
}
impl<'a> TextInput<'a> {
    pub(crate) fn accessibility_label(mut self, label: &str) -> Self {
        self.label = label.into();
        self
    }
    pub(crate) fn id(mut self, id: widget::Id) -> Self {
        self.inner = self.inner.id(id.clone());
        self.id = id;
        self
    }
    pub(crate) fn secure(mut self, secure: bool) -> Self {
        self.inner = self.inner.secure(secure);
        self.secure = secure;
        self
    }
    pub(crate) fn size(mut self, size: impl Into<iced::Pixels>) -> Self {
        self.inner = self.inner.size(size);
        self
    }
    pub(crate) fn padding(mut self, p: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(p);
        self
    }
    pub(crate) fn width(mut self, w: impl Into<Length>) -> Self {
        self.inner = self.inner.width(w);
        self
    }
    pub(crate) fn style(
        mut self,
        f: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self {
        self.inner = self.inner.style(f);
        self
    }
    pub(crate) fn on_input(self, f: impl Fn(String) -> Message + Send + Sync + 'static) -> Self {
        self.on_input_maybe(Some(f))
    }
    pub(crate) fn on_input_maybe(
        mut self,
        f: Option<impl Fn(String) -> Message + Send + Sync + 'static>,
    ) -> Self {
        self.action = f.map(|f| std::sync::Arc::new(f) as InputAction);
        self.inner = self
            .inner
            .on_input_maybe(self.action.clone().map(|f| move |v| f(v)));
        self
    }
}
impl<'a> From<TextInput<'a>> for Element<'a, Message> {
    fn from(input: TextInput<'a>) -> Self {
        Element::new(Semantic {
            content: input.inner.into(),
            kind: Kind::Input {
                label: input.label,
                value: if input.secure {
                    String::new()
                } else {
                    input.value
                },
                secure: input.secure,
                id: input.id,
                action: input.action,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::widget::Operation;
    #[test]
    fn modal_tree_excludes_background_and_never_exposes_password_value() {
        let content = root(
            iced::widget::stack![
                button("后台发送").on_press(Message::Submit),
                modal(
                    iced::widget::column![
                        button("关闭设置").on_press(Message::Settings),
                        text_input("API key", "secret-that-must-not-be-exposed")
                            .secure(true)
                            .on_input(Message::Filter),
                        text_input("名称", "当前名称").on_input(Message::Filter)
                    ]
                    .into()
                )
            ]
            .width(iced::Fill)
            .height(iced::Fill)
            .into(),
        );
        let software = iced_tiny_skia::Renderer::new(iced::Font::DEFAULT, 16.into());
        #[cfg(feature = "gpu")]
        let mut renderer = iced::Renderer::Secondary(software);
        #[cfg(not(feature = "gpu"))]
        let mut renderer = software;
        let mut content = content;
        let mut tree = Tree::new(content.as_widget());
        let size = Size::new(800., 600.);
        let layout = content.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, size),
        );
        set_scale(2.);
        content.as_widget().draw(
            &tree,
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            Layout::new(&layout),
            mouse::Cursor::Unavailable,
            &Rectangle::with_size(size),
        );
        FRAME.with(|f| {
            let f = f.borrow();
            let snapshot = f.snapshot.as_ref().unwrap();
            assert_eq!(snapshot.nodes.len(), 4);
            assert!(snapshot
                .nodes
                .iter()
                .all(|(_, node)| node.label() != Some("后台发送")));
            let password = snapshot
                .nodes
                .iter()
                .find(|(_, n)| n.role() == Role::PasswordInput)
                .unwrap();
            assert!(password.1.value().is_none());
            assert!(snapshot
                .nodes
                .iter()
                .any(|(_, n)| n.value() == Some("当前名称")));
            assert_eq!(
                snapshot.nodes[0].1.transform(),
                Some(&accesskit::Affine::scale(2.))
            );
        });
        set_scale(1.);
    }
    #[test]
    fn workspace_pages_replace_the_chat_accessibility_tree() {
        for (kind, expected) in [
            (crate::pages::Kind::Tasks, "每周工作周报"),
            (crate::pages::Kind::Memory, "文档内容"),
            (crate::pages::Kind::Skills, "导入技能 ZIP"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut app = crate::App {
                backend: Some(crate::backend::Backend::open(dir.path()).unwrap()),
                ..crate::App::default()
            };
            let _ = app.update(Message::Page(crate::pages::Event::Open(kind)));
            let mut view = app.view();
            let software = iced_tiny_skia::Renderer::new(iced::Font::DEFAULT, 16.into());
            #[cfg(feature = "gpu")]
            let mut renderer = iced::Renderer::Secondary(software);
            #[cfg(not(feature = "gpu"))]
            let mut renderer = software;
            let mut tree = Tree::new(view.as_widget());
            let size = Size::new(1080., 760.);
            let layout = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, size),
            );
            view.as_widget().draw(
                &tree,
                &mut renderer,
                &Theme::Light,
                &renderer::Style::default(),
                Layout::new(&layout),
                mouse::Cursor::Unavailable,
                &Rectangle::with_size(size),
            );
            FRAME.with(|f| {
                let f = f.borrow();
                let nodes = &f.snapshot.as_ref().unwrap().nodes;
                assert!(
                    nodes.iter().any(|(_, n)| n.label() == Some(expected)),
                    "missing page control: {expected}"
                );
                assert!(!nodes.iter().any(|(_, n)| n.label() == Some("描述任务")));
            });
        }
    }
    #[test]
    fn scroll_geometry_applies_translation_and_clips_controls() {
        let mut geometry = Geometry {
            offset: Vector::ZERO,
            viewport: Rectangle::new(iced::Point::ORIGIN, Size::new(400., 300.)),
            pending: Some((
                Rectangle::new(iced::Point::new(50., 50.), Size::new(200., 100.)),
                Vector::new(0., 80.),
            )),
        };
        let mut state = State::default();
        geometry.traverse(&mut |op| {
            op.custom(
                None,
                Rectangle::new(iced::Point::new(60., 140.), Size::new(100., 30.)),
                &mut state,
            )
        });
        assert_eq!(state.bounds.unwrap().y, 60.);
        assert_eq!(geometry.offset, Vector::ZERO);
    }
}

pub mod pick_list {
    pub use iced::widget::pick_list::*;
}
pub(crate) struct PickList<'a, T: ToString + PartialEq + Clone> {
    inner: iced::widget::PickList<'a, T, Vec<T>, T, Message>,
    label: String,
    value: String,
    options: Vec<(String, Message)>,
}
pub fn pick_list<'a, T: ToString + PartialEq + Clone + 'a>(
    options: Vec<T>,
    selected: Option<T>,
    on_select: impl Fn(T) -> Message + 'a,
) -> PickList<'a, T> {
    let actions = options
        .iter()
        .map(|v| (v.to_string(), on_select(v.clone())))
        .collect();
    let value = selected
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    PickList {
        inner: iced::widget::pick_list(options, selected, on_select),
        label: "选择".into(),
        value,
        options: actions,
    }
}
impl<'a, T: ToString + PartialEq + Clone + 'a> PickList<'a, T> {
    pub(crate) fn placeholder(mut self, v: impl Into<String>) -> Self {
        let v = v.into();
        self.inner = self.inner.placeholder(v.clone());
        self.label = v;
        self
    }
    pub(crate) fn width(mut self, v: impl Into<Length>) -> Self {
        self.inner = self.inner.width(v);
        self
    }
    pub(crate) fn padding(mut self, v: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(v);
        self
    }
    pub(crate) fn text_size(mut self, v: impl Into<iced::Pixels>) -> Self {
        self.inner = self.inner.text_size(v);
        self
    }
    pub(crate) fn style(
        mut self,
        v: impl Fn(&Theme, pick_list::Status) -> pick_list::Style + 'a,
    ) -> Self {
        self.inner = self.inner.style(v);
        self
    }
    pub(crate) fn menu_style(
        mut self,
        v: impl Fn(&Theme) -> iced::widget::overlay::menu::Style + 'a,
    ) -> Self {
        self.inner = self.inner.menu_style(v);
        self
    }
}
impl<'a, T: ToString + PartialEq + Clone + 'a> From<PickList<'a, T>> for Element<'a, Message> {
    fn from(p: PickList<'a, T>) -> Self {
        Element::new(Semantic {
            content: p.inner.into(),
            kind: Kind::Choice {
                label: p.label,
                value: p.value,
                options: p.options,
            },
        })
    }
}
pub fn options_overlay<'a>(
    base: Element<'a, Message>,
    options: Option<&'a [(String, Message)]>,
) -> Element<'a, Message> {
    let Some(options) = options else {
        return base;
    };
    let mut list =
        iced::widget::column![button("取消选择").on_press(Message::AccessibilityOptions(None))]
            .spacing(8);
    for (label, action) in options {
        list = list.push(
            button(iced::widget::text(label))
                .style(crate::ui::nav)
                .padding(10)
                .width(iced::Fill)
                .on_press(Message::AccessibilityChoose(Box::new(action.clone()))),
        );
    }
    let panel = iced::widget::container(iced::widget::scrollable(list))
        .width(340)
        .max_height(400)
        .padding(16)
        .style(iced::widget::container::bordered_box);
    iced::widget::stack![
        base,
        iced::widget::mouse_area(
            iced::widget::container(iced::widget::Space::new())
                .width(iced::Fill)
                .height(iced::Fill)
        )
        .on_press(Message::AccessibilityOptions(None)),
        iced::widget::container(iced::widget::opaque(modal(panel.into())))
            .center_x(iced::Fill)
            .center_y(iced::Fill)
    ]
    .into()
}
