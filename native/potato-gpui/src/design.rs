//! Values from app/src/styles/tokens.css.
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
pub fn apply(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let t = Theme::global_mut(cx);
    let c = |light, night| Hsla::from(rgb(if dark { night } else { light }));
    t.font_family = if cfg!(target_os = "macos") {
        ".SystemUIFont"
    } else {
        "Segoe UI"
    }
    .into();
    t.font_size = px(16.);
    t.radius = px(8.);
    t.radius_lg = px(16.);
    t.background = c(0xffffff, 0x181818);
    t.foreground = c(0x1d1d1f, 0xf5f5f7);
    t.border = c(0xe8e8e8, 0x373737);
    t.muted = c(0xf5f5f5, 0x202020);
    t.muted_foreground = c(0x6b6b6b, 0xa8a8a8);
    t.accent = c(0xededed, 0x333333);
    t.accent_foreground = t.foreground;
    t.primary = c(0x202020, 0xececec);
    t.primary_foreground = c(0xffffff, 0x171717);
    t.primary_hover = c(0x383838, 0xffffff);
    t.button_primary = t.primary;
    t.button_primary_foreground = t.primary_foreground;
    t.button_primary_hover = t.primary_hover;
    t.button = t.background;
    t.button_foreground = t.foreground;
    t.button_hover = t.accent;
    t.button_active = c(0xe5e5e5, 0x393939);
    t.secondary = t.muted;
    t.secondary_foreground = t.foreground;
    t.popover = c(0xffffff, 0x2d2d2d);
    t.popover_foreground = t.foreground;
    t.input = t.border;
    t.ring = c(0xd2d2d2, 0x505050);
    t.caret = t.foreground;
    t.table = c(0xffffff, 0x252525);
    t.table_head = c(0xf5f5f5, 0x2d2d2d);
    t.table_head_foreground = t.foreground;
    t.table_row_border = t.border;
    t.table_hover = t.accent;
    Theme::sync_base(cx);
}
pub fn shadow() -> BoxShadow {
    BoxShadow {
        inset: false,
        color: rgba(0x1111110a).into(),
        offset: point(px(0.), px(4.)),
        blur_radius: px(16.),
        spread_radius: px(0.),
    }
}
