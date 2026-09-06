//! Observe hover before rich text consumes pointer events; do not intercept child actions.
use crate::Message;
use iced::advanced::{
    layout, mouse, overlay, renderer,
    widget::{self, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

pub fn answer<'a>(content: impl Into<Element<'a, Message>>, index: usize) -> Element<'a, Message> {
    Element::new(Hover {
        content: content.into(),
        index: Some(index),
    })
}
/// Draw a quote rule against the measured content height, including nested quotes.
pub fn quote<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    Element::new(Hover {
        content: content.into(),
        index: None,
    })
}
struct Hover<'a> {
    content: Element<'a, Message>,
    index: Option<usize>,
}
impl Widget<Message, Theme, Renderer> for Hover<'_> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<bool>()
    }
    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(false)
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
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
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
        let active = cursor.is_over(layout.bounds()) && cursor.is_over(*viewport);
        let previous = tree.state.downcast_mut::<bool>();
        if let Some(index) = self.index {
            if active != *previous {
                *previous = active;
                shell.publish(if active {
                    Message::HoverAnswer(index)
                } else {
                    Message::LeaveAnswer(index)
                });
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
        )
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
        if self.index.is_none() {
            use iced::advanced::Renderer as _;
            let bounds = layout.bounds();
            if let Some(bounds) = (Rectangle {
                width: 2.0,
                ..bounds
            })
            .intersection(viewport)
            {
                let mut color = theme.palette().text;
                color.a = 0.25;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..Default::default()
                    },
                    color,
                );
            }
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
