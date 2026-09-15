use gpui_kit::WindowAppearance;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreference {
    Light,
    Dark,
    System,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    pub fn from_preferences(saved: &Value) -> Self {
        if saved["follow_system"] == true {
            Self::System
        } else if saved["dark"] == true {
            Self::Dark
        } else {
            Self::Light
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Light => "theme-light",
            Self::Dark => "theme-dark",
            Self::System => "theme-system",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "浅色",
            Self::Dark => "深色",
            Self::System => "自动（跟随系统）",
        }
    }

    pub fn shortcut_label(self) -> &'static str {
        match self {
            Self::Light => "主题：浅色 · 点击切换为深色",
            Self::Dark => "主题：深色 · 点击切换为自动",
            Self::System => "主题：自动（跟随系统） · 点击切换为浅色",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
            Self::System => Self::Light,
        }
    }

    pub fn is_dark(self, appearance: WindowAppearance) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => matches!(
                appearance,
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        }
    }

    pub fn save(self, saved: &mut Value) {
        saved["follow_system"] = json!(self == Self::System);
        saved["dark"] = json!(self == Self::Dark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_and_preserve_other_settings() {
        for mode in ThemePreference::ALL {
            let mut saved = json!({"width": 1000});
            mode.save(&mut saved);
            assert_eq!(ThemePreference::from_preferences(&saved), mode);
            assert_eq!(saved["width"], 1000);
        }
        assert_eq!(
            ThemePreference::from_preferences(&json!({})),
            ThemePreference::Light
        );
        assert_eq!(
            ThemePreference::from_preferences(&json!({"dark": true})),
            ThemePreference::Dark
        );
        assert_eq!(
            ThemePreference::from_preferences(&json!({"dark": true, "follow_system": true})),
            ThemePreference::System
        );
    }

    #[test]
    fn only_automatic_mode_tracks_system_appearance() {
        for appearance in [
            WindowAppearance::Light,
            WindowAppearance::VibrantLight,
            WindowAppearance::Dark,
            WindowAppearance::VibrantDark,
        ] {
            assert!(!ThemePreference::Light.is_dark(appearance));
            assert!(ThemePreference::Dark.is_dark(appearance));
            assert_eq!(
                ThemePreference::System.is_dark(appearance),
                matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                )
            );
        }
        assert_eq!(
            ThemePreference::Light.next().next(),
            ThemePreference::System
        );
        assert_eq!(ThemePreference::System.next(), ThemePreference::Light);
    }
}
