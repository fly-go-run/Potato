use crate::view::icon_button;
use crate::*;
use gpui_kit::base::slider::{
    Slider, SliderEvent, SliderIndicator, SliderState, SliderThumb, SliderTrack,
};
use gpui_kit::prelude::*;

#[derive(Default)]
pub struct Effort {
    slider: Option<Entity<SliderState>>,
    subscription: Option<Subscription>,
    pub saving: bool,
    preview: Option<String>,
}

pub fn label(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_else(|| "默认".into())
}

impl Potato {
    pub fn active_model_info(&self) -> Option<(&Value, &Value)> {
        let p = self
            .providers
            .iter()
            .find(|p| p["id"] == self.model["provider_id"])?;
        let m = ["extra_models", "models"]
            .iter()
            .flat_map(|k| p[*k].as_array().into_iter().flatten())
            .find(|m| m["id"] == self.model["model"])?;
        Some((p, m))
    }
    pub fn effort_options(&self) -> Vec<String> {
        self.active_model_info()
            .map(|(p, m)| potato_core::reasoning::effort_options(p, m))
            .unwrap_or_default()
    }
    pub fn effort_label(&self) -> String {
        let current = self
            .active_model_info()
            .and_then(|(p, m)| potato_core::reasoning::effective_effort(p, m));
        label(current.as_deref().unwrap_or(""))
    }
    pub fn prepare_effort(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options = self.effort_options();
        if options.is_empty() {
            return;
        }
        let selected = self
            .active_model_info()
            .and_then(|(p, m)| potato_core::reasoning::effective_effort(p, m));
        let index = selected
            .as_ref()
            .and_then(|v| options.iter().position(|s| s == v))
            .unwrap_or(0);
        let state = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max((options.len().saturating_sub(1).max(1)) as f32)
                .step(1.)
                .default_value(index as f32)
        });
        self.effort.preview = None;
        self.effort.subscription = Some(cx.subscribe_in(&state, window, |s, _, event, w, cx| {
            let value = match event {
                SliderEvent::Change(v) | SliderEvent::Release(v) => v.end() as usize,
            };
            let Some(effort) = s.effort_options().get(value).cloned() else {
                return;
            };
            match event {
                SliderEvent::Change(_) => {
                    s.effort.preview = Some(effort);
                    cx.notify();
                }
                SliderEvent::Release(_) => {
                    // Follow the pointer continuously during dragging; snap only on release.
                    if let Some(state) = s.effort.slider.clone() {
                        state.update(cx, |state, cx| state.set_value(value as f32, w, cx));
                    }
                    s.save_effort(Some(effort), w, cx);
                }
            }
        }));
        self.effort.slider = Some(state);
    }
    pub fn save_effort(&mut self, value: Option<String>, w: &mut Window, cx: &mut Context<Self>) {
        if self.streaming || self.effort.saving {
            return;
        }
        if value
            .as_ref()
            .is_some_and(|v| !self.effort_options().contains(v))
        {
            return;
        }
        let provider = string(&self.model, "provider_id");
        let model = string(&self.model, "model");
        self.effort.saving = true;
        self.request_result(
            "PUT",
            &format!(
                "/api/models/{}/models/{}/config",
                segment(&provider),
                segment(&model)
            ),
            json!({"reasoning_effort":value}),
            w,
            cx,
            move |s, result, w, cx| {
                s.effort.saving = false;
                match result {
                    Ok(updated) => {
                        if let Some(p) = s.providers.iter_mut().find(|p| p["id"] == provider) {
                            *p = updated;
                        }
                        s.notice.clear();
                    }
                    Err(error) => s.notice = error,
                }
                s.prepare_effort(w, cx);
            },
        );
    }
    pub fn effort_view(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let options = self.effort_options();
        let Some(state) = self.effort.slider.clone().filter(|_| !options.is_empty()) else {
            return div().into_any_element();
        };
        let current = self
            .effort
            .preview
            .as_deref()
            .map(label)
            .unwrap_or_else(|| self.effort_label());
        let fraction = if options.len() > 1 {
            state.read(cx).percentage().end
        } else {
            0.
        };
        let disabled = self.streaming || self.effort.saving;
        let slider_disabled = disabled || options.len() < 2;
        // One full-height track; inset only the value-mapping region so the
        // first/last thumb remain inside the rounded ends. The inset equals half
        // the thumb width, so both endpoints meet the track edge exactly.
        let mut track = SliderIndicator::new(&state)
            .relative()
            .w_full()
            .h(px(28.))
            .child(
                div()
                    .absolute()
                    .left(px(-11.))
                    .top_0()
                    .h_full()
                    .right(relative(1. - fraction))
                    .rounded_tl(px(9.))
                    .rounded_bl(px(9.))
                    .bg(cx.theme().muted_foreground.opacity(0.55)),
            );
        for i in 0..options.len() {
            let pos = if options.len() > 1 {
                i as f32 / (options.len() - 1) as f32
            } else {
                0.
            };
            track = track.child(
                div()
                    .absolute()
                    .left(relative(pos))
                    .top(px(12.5))
                    .ml(px(-1.5))
                    .size(px(3.))
                    .rounded_full()
                    .bg(cx.theme().muted_foreground.opacity(0.55)),
            );
        }
        let slider = Slider::new(&state)
            .disabled(slider_disabled)
            .w_full()
            .child(
                SliderTrack::new(&state)
                    .disabled(slider_disabled)
                    .w_full()
                    .px(px(11.))
                    .h(px(28.))
                    .rounded(px(9.))
                    .overflow_hidden()
                    .bg(cx.theme().accent)
                    .child(
                        track.child(
                            SliderThumb::new(&state)
                                .disabled(slider_disabled)
                                .absolute()
                                .left(relative(fraction))
                                .top_0()
                                .ml(px(-11.))
                                .w(px(22.))
                                .h(px(28.))
                                .rounded(px(9.))
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border),
                        ),
                    ),
            );
        div()
            .w(px(288.))
            .p(px(16.))
            .text_size(px(14.))
            .line_height(px(20.))
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("思考深度"),
                    )
                    .child(current)
                    .child(div().flex_1())
                    .child(
                        icon_button(
                            "effort-default",
                            IconName::RefreshCw,
                            "恢复服务商默认思考深度",
                        )
                        .text_color(cx.theme().muted_foreground)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.save_effort(None, w, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(div().text_color(cx.theme().muted_foreground).child("更快"))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("更深入"),
                    ),
            )
            .child(
                div()
                    .id("effort-slider-keyboard")
                    .debug_selector(|| "effort-slider-keyboard".into())
                    .tab_index(0)
                    .on_key_down(cx.listener(|s, event: &KeyDownEvent, w, cx| {
                        if s.effort.saving || s.streaming {
                            return;
                        }
                        let options = s.effort_options();
                        if options.is_empty() {
                            return;
                        }
                        let current = s
                            .effort
                            .slider
                            .as_ref()
                            .map(|state| state.read(cx).value().end() as usize)
                            .unwrap_or(0);
                        let index = match event.keystroke.key.as_str() {
                            "left" | "down" => current.saturating_sub(1),
                            "right" | "up" => (current + 1).min(options.len() - 1),
                            "home" => 0,
                            "end" => options.len() - 1,
                            _ => return,
                        };
                        cx.stop_propagation();
                        s.save_effort(Some(options[index].clone()), w, cx);
                    }))
                    .child(slider),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Backend, Potato};
    use gpui_kit::{
        AppContext, Context, Entity, IntoElement, Modifiers, MouseButton, Render, TestAppContext,
        Window, gpui, point, px,
    };
    use serde_json::json;
    struct Harness(Entity<Potato>);
    impl Render for Harness {
        fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.update(cx, |s, cx| s.effort_view(w, cx))
        }
    }
    #[gpui::test]
    fn slider_uses_current_provider_model_range_and_pointer_events(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let backend = Backend::for_ui_test(
            std::env::temp_dir().join(format!("effort-gpui-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        backend.executor.block_on(async {
            for (id, options, effort) in [
                ("a", json!(["low", "high"]), "high"),
                ("b", json!(["medium", "max", "ultra"]), "max"),
            ] {
                backend
                    .core
                    .request(
                        "POST",
                        "/api/models/custom-providers",
                        json!({"id":id,"name":id}),
                    )
                    .await
                    .unwrap();
                backend
                    .core
                    .request(
                        "POST",
                        &format!("/api/models/{id}/models"),
                        json!({"id":"same"}),
                    )
                    .await
                    .unwrap();
                backend
                    .core
                    .request(
                        "PUT",
                        &format!("/api/models/{id}/models/same/config"),
                        json!({"reasoning_effort_options":options,"reasoning_effort":effort}),
                    )
                    .await
                    .unwrap();
            }
            backend
                .core
                .request(
                    "PUT",
                    "/api/models/active",
                    json!({"provider_id":"b","model":"same"}),
                )
                .await
                .unwrap();
        });
        let (view, cx) = cx.add_window_view(|w,cx| {
            let app = cx.new(|cx| Potato::new(backend,w,cx));
            app.update(cx, |s,cx| {
                s.providers = vec![json!({"id":"a","models":[{"id":"same","reasoning_effort_options":["low","high"]}]}),json!({"id":"b","models":[{"id":"same","reasoning_effort_options":["medium","max","ultra"],"reasoning_effort":"max"}]})];
                s.model = json!({"provider_id":"b","model":"same"});
                s.prepare_effort(w,cx);
            });
            Harness(app)
        });
        view.read_with(cx, |view, cx| {
            let app = view.0.read(cx);
            assert_eq!(app.effort_options(), vec!["medium", "max", "ultra"]);
            assert_eq!(app.effort_label(), "Max");
            assert_eq!(app.effort.slider.as_ref().unwrap().read(cx).max_value(), 2.);
        });
        let bounds = cx.debug_bounds("effort-slider-keyboard").unwrap();
        cx.simulate_mouse_down(
            point(bounds.right() - px(14.), bounds.center().y),
            MouseButton::Left,
            Modifiers::none(),
        );
        view.read_with(cx, |view, cx| {
            assert_eq!(view.0.read(cx).effort.preview.as_deref(), Some("ultra"))
        });
        // Movement within a discrete interval must still retain a continuous
        // visual position; the label remains a valid discrete choice.
        let state = view.read_with(cx, |view, cx| {
            view.0.read(cx).effort.slider.clone().unwrap()
        });
        cx.update(|window, cx| {
            state.update(cx, |state, cx| {
                let bounds = state.bounds();
                state.update_value_by_position(
                    gpui_kit::Axis::Horizontal,
                    point(bounds.left() + bounds.size.width * 0.63, bounds.center().y),
                    false,
                    window,
                    cx,
                );
                assert!((state.percentage().end - 0.63).abs() < 0.01);
                assert_eq!(state.value().end(), 1.);
            });
        });
    }
}
