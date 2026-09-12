//! Lucide artwork from the original React UI, embedded alongside GPUI assets.
use gpui_kit::component::{Icon, IconNamed};
use gpui_kit::*;
use std::borrow::Cow;
#[derive(Clone, Copy, IntoElement)]
pub enum IconName {
    Ellipsis,
    Trash,
    ListEnd,
    Mic,
    ArrowUp,
    ArrowDown,
    Blocks,
    Bot,
    Check,
    ChevronDown,
    Terminal,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    Clock,
    Copy,
    Database,
    Download,
    FileText,
    Files,
    Folder,
    FolderOpen,
    Info,
    Keyboard,
    LayoutGrid,
    Moon,
    Notebook,
    PanelLeft,
    PanelRight,
    Paperclip,
    Play,
    Plus,
    RefreshCw,
    Search,
    Settings,
    ShieldCheck,
    SlidersHorizontal,
    Sparkles,
    Square,
    SquarePen,
    Sun,
    Upload,
    X,
}
impl IconNamed for IconName {
    fn path(self) -> SharedString {
        match self {
            Self::Trash => "potato/trash-2.svg".into(),
            Self::ListEnd => "potato/list-end.svg".into(),
            Self::Ellipsis => "potato/ellipsis.svg".into(),
            Self::Mic => "potato/mic.svg".into(),
            Self::ArrowUp => "potato/arrow-up.svg".into(),
            Self::ArrowDown => "potato/arrow-down.svg".into(),
            Self::Blocks => "potato/blocks.svg".into(),
            Self::Bot => "potato/bot.svg".into(),
            Self::Check => "potato/check.svg".into(),
            Self::ChevronUp => "potato/chevron-up.svg".into(),
            Self::Terminal => "potato/square-terminal.svg".into(),
            Self::ChevronDown => "potato/chevron-down.svg".into(),
            Self::ChevronLeft => "potato/chevron-left.svg".into(),
            Self::ChevronRight => "potato/chevron-right.svg".into(),
            Self::Clock => "potato/clock.svg".into(),
            Self::Copy => "potato/copy.svg".into(),
            Self::Database => "potato/database.svg".into(),
            Self::Download => "potato/download.svg".into(),
            Self::FileText => "potato/file-text.svg".into(),
            Self::Files => "potato/files.svg".into(),
            Self::Folder => "potato/folder.svg".into(),
            Self::FolderOpen => "potato/folder-open.svg".into(),
            Self::Info => "potato/info.svg".into(),
            Self::Keyboard => "potato/keyboard.svg".into(),
            Self::LayoutGrid => "potato/layout-grid.svg".into(),
            Self::Moon => "potato/moon.svg".into(),
            Self::Notebook => "potato/notebook.svg".into(),
            Self::PanelRight => "potato/panel-right.svg".into(),
            Self::PanelLeft => "potato/panel-left.svg".into(),
            Self::Paperclip => "potato/paperclip.svg".into(),
            Self::Play => "potato/play.svg".into(),
            Self::Plus => "potato/plus.svg".into(),
            Self::RefreshCw => "potato/refresh-cw.svg".into(),
            Self::Search => "potato/search.svg".into(),
            Self::Settings => "potato/settings.svg".into(),
            Self::ShieldCheck => "potato/shield-check.svg".into(),
            Self::SlidersHorizontal => "potato/sliders-horizontal.svg".into(),
            Self::Sparkles => "potato/sparkles.svg".into(),
            Self::Square => "potato/square.svg".into(),
            Self::SquarePen => "potato/square-pen.svg".into(),
            Self::Sun => "potato/sun.svg".into(),
            Self::Upload => "potato/upload.svg".into(),
            Self::X => "potato/x.svg".into(),
        }
    }
}
impl RenderOnce for IconName {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::new(self)
    }
}
pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "potato/panel-right.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/panel-right.svg"
            )))),
            "potato/trash-2.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/trash-2.svg"
            )))),
            "potato/list-end.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/list-end.svg"
            )))),
            "potato/ellipsis.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/ellipsis.svg"
            )))),
            "potato/mic.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/mic.svg"
            )))),
            "potato/arrow-up.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/arrow-up.svg"
            )))),
            "potato/arrow-down.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/arrow-down.svg"
            )))),
            "potato/blocks.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/blocks.svg"
            )))),
            "potato/bot.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/bot.svg"
            )))),
            "potato/check.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/check.svg"
            )))),
            "potato/chevron-up.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/chevron-up.svg"
            )))),
            "potato/square-terminal.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/square-terminal.svg"
            )))),
            "potato/chevron-down.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/chevron-down.svg"
            )))),
            "potato/chevron-left.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/chevron-left.svg"
            )))),
            "potato/chevron-right.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/chevron-right.svg"
            )))),
            "potato/clock.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/clock.svg"
            )))),
            "potato/copy.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/copy.svg"
            )))),
            "potato/database.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/database.svg"
            )))),
            "potato/download.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/download.svg"
            )))),
            "potato/file-text.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/file-text.svg"
            )))),
            "potato/files.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/files.svg"
            )))),
            "potato/folder.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/folder.svg"
            )))),
            "potato/folder-open.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/folder-open.svg"
            )))),
            "potato/info.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/info.svg"
            )))),
            "potato/keyboard.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/keyboard.svg"
            )))),
            "potato/layout-grid.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/layout-grid.svg"
            )))),
            "potato/moon.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/moon.svg"
            )))),
            "potato/notebook.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/notebook.svg"
            )))),
            "potato/panel-left.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/panel-left.svg"
            )))),
            "potato/paperclip.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/paperclip.svg"
            )))),
            "potato/play.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/play.svg"
            )))),
            "potato/plus.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/plus.svg"
            )))),
            "potato/refresh-cw.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/refresh-cw.svg"
            )))),
            "potato/search.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/search.svg"
            )))),
            "potato/settings.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/settings.svg"
            )))),
            "potato/shield-check.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/shield-check.svg"
            )))),
            "potato/sliders-horizontal.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/sliders-horizontal.svg"
            )))),
            "potato/sparkles.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/sparkles.svg"
            )))),
            "potato/square.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/square.svg"
            )))),
            "potato/square-pen.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/square-pen.svg"
            )))),
            "potato/sun.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/sun.svg"
            )))),
            "potato/upload.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/upload.svg"
            )))),
            "potato/x.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/x.svg")))),
            _ => gpui_kit::assets::Assets.load(path),
        }
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
