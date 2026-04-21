#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSApp, NSApplication, NSImage, NSMenu},
    base::{id, nil},
    foundation::{NSData, NSUInteger},
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use tracing::{error, info};

#[cfg(target_os = "macos")]
const APP_ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");

/// Install the application icon so dev builds also show a Dock icon.
#[cfg(target_os = "macos")]
pub fn install_application_icon() {
    unsafe {
        let app = NSApp();
        if app == nil {
            error!("NSApp unavailable, skipping application icon install");
            return;
        }

        let icon_data = NSData::dataWithBytes_length_(
            nil,
            APP_ICON_PNG.as_ptr() as *const c_void,
            APP_ICON_PNG.len() as NSUInteger,
        );
        if icon_data == nil {
            error!("Failed to build NSData for embedded application icon");
            return;
        }

        let image = NSImage::initWithData_(NSImage::alloc(nil), icon_data);
        if image == nil {
            error!("Failed to decode embedded application icon PNG");
            return;
        }

        app.setApplicationIconImage_(image);
        info!("Installed macOS application icon");
    }
}

/// Rewire GPUI-created Window menu items to native AppKit selectors so
/// macOS standard shortcuts like Cmd+M and Cmd+Ctrl+F work reliably.
#[cfg(target_os = "macos")]
pub fn install_native_window_menu_shortcuts() {
    unsafe {
        let app = NSApp();
        if app == nil {
            return;
        }

        let main_menu = app.mainMenu();
        if main_menu == nil {
            return;
        }

        let window_menu_item = main_menu.itemAtIndex_(1);
        if window_menu_item == nil {
            return;
        }

        let window_menu: id = msg_send![window_menu_item, submenu];
        if window_menu == nil {
            return;
        }

        let minimize_item = window_menu.itemAtIndex_(0);
        if minimize_item != nil {
            let _: () = msg_send![minimize_item, setTarget: nil];
            let _: () = msg_send![minimize_item, setAction: sel!(performMiniaturize:)];
        }

        let fullscreen_item = window_menu.itemAtIndex_(1);
        if fullscreen_item != nil {
            let _: () = msg_send![fullscreen_item, setTarget: nil];
            let _: () = msg_send![fullscreen_item, setAction: sel!(toggleFullScreen:)];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install_application_icon() {}

#[cfg(not(target_os = "macos"))]
pub fn install_native_window_menu_shortcuts() {}
