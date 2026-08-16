//! Align macOS traffic lights with the 44px chrome row (`h-11`).
//!
//! Tauri's `trafficLightPosition.y` only grows the titlebar container. On
//! current macOS the three buttons keep their default origin near the top, so
//! they sit above the sidebar / collapsed-rail icons. This recenters them.

use objc2_app_kit::{NSView, NSWindow, NSWindowButton};
use objc2_foundation::NSPoint;
use tauri::{AppHandle, Manager, WebviewWindow};

/// Same height as `Sidebar` / `CollapsedRail` / the overlay drag strip.
const TITLEBAR_HEIGHT: f64 = 44.0;
/// Same as `tauri.conf.json` `trafficLightPosition.x`.
const LIGHT_X: f64 = 16.0;

pub fn align_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        align(&window);
    }
}

pub fn align(window: &WebviewWindow) {
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    if ptr.is_null() {
        return;
    }
    // Safety: Tauri returns this window's NSWindow.
    let ns_window = unsafe { &*ptr.cast::<NSWindow>() };
    unsafe { position_in(ns_window) };
}

unsafe fn position_in(window: &NSWindow) {
    let Some(close) = window.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let Some(miniaturize) = window.standardWindowButton(NSWindowButton::MiniaturizeButton) else {
        return;
    };
    let Some(zoom) = window.standardWindowButton(NSWindowButton::ZoomButton) else {
        return;
    };

    let Some(button_bar) = close.superview() else {
        return;
    };
    let Some(title_bar_container) = button_bar.superview() else {
        return;
    };

    let close_rect = NSView::frame(&close);
    let button_height = close_rect.size.height;
    let space_between = NSView::frame(&miniaturize).origin.x - close_rect.origin.x;

    let mut title_bar_rect = NSView::frame(&title_bar_container);
    title_bar_rect.size.height = TITLEBAR_HEIGHT;
    title_bar_rect.origin.y = window.frame().size.height - TITLEBAR_HEIGHT;
    title_bar_container.setFrame(title_bar_rect);

    // Vertical center in the 44px bar. Same number for flipped and unflipped.
    let origin_y = ((TITLEBAR_HEIGHT - button_height) / 2.0).max(0.0);

    for (index, button) in [close, miniaturize, zoom].into_iter().enumerate() {
        button.setFrameOrigin(NSPoint {
            x: LIGHT_X + (index as f64 * space_between),
            y: origin_y,
        });
    }
}
