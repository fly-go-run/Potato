use gpui_kit::{Pixels, Size, px, size};
use serde_json::Value;
pub fn restored_size(saved: &Value, display: Size<Pixels>) -> Size<Pixels> {
    let dimension = |key, fallback: f64, min: f64, screen: Pixels| {
        let value = if saved["remember_window"] == false {
            fallback
        } else {
            saved[key]
                .as_f64()
                .filter(|v| v.is_finite())
                .unwrap_or(fallback)
        };
        px(value.clamp(min, (f32::from(screen) as f64 - 48.).max(min)) as f32)
    };
    size(
        dimension("width", 1180., 800., display.width),
        dimension("height", 800., 580., display.height),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn saved_dimensions_are_clamped_to_current_display() {
        assert_eq!(
            restored_size(
                &json!({"width":4000,"height":3000}),
                size(px(1440.), px(900.))
            ),
            size(px(1392.), px(852.))
        );
        assert_eq!(
            restored_size(&json!({"width":1,"height":10}), size(px(1440.), px(900.))),
            size(px(800.), px(580.))
        );
    }
    #[test]
    fn disabling_remember_ignores_old_dimensions() {
        assert_eq!(
            restored_size(
                &json!({"remember_window":false,"width":900,"height":600}),
                size(px(1800.), px(1200.))
            ),
            size(px(1180.), px(800.))
        );
    }
}
