#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSApp, NSApplication, NSMenu},
    base::{id, nil},
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

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
pub fn install_native_window_menu_shortcuts() {}
