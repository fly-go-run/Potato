//! Runtime Dock icon updates for macOS appearance changes.
//!
//! The bundled `.icns` is the light appearance used by Finder and on first
//! launch. AppKit lets us replace the Dock image at runtime, so the dark
//! appearance can use a higher-contrast background without changing the
//! installer icon or any other platform.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;

const LIGHT_ICON: &[u8] = include_bytes!("../icons/icon-light.png");
const DARK_ICON: &[u8] = include_bytes!("../icons/icon-dark.png");

pub(crate) fn set(theme: tauri::Theme) {
    let Some(marker) = MainThreadMarker::new() else {
        log::warn!("could not update the Potato Dock icon off the macOS main thread");
        return;
    };

    let bytes = match theme {
        tauri::Theme::Dark => DARK_ICON,
        tauri::Theme::Light => LIGHT_ICON,
        _ => LIGHT_ICON,
    };
    let data = NSData::with_bytes(bytes);
    let Some(image) = NSImage::initWithData(marker.alloc(), &data) else {
        log::warn!("could not decode the Potato Dock icon for the current appearance");
        return;
    };

    let application = NSApplication::sharedApplication(marker);
    // AppKit retains the image as the new application icon.
    unsafe { application.setApplicationIconImage(Some(&image)) };
}
