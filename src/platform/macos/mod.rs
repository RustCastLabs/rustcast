//! Macos specific logic, such as window settings, etc.
pub mod accessibility;
pub mod discovery;
pub mod events;
pub mod haptics;
pub mod launching;
pub mod urlscheme;
pub mod window;

use iced::wgpu::rwh::WindowHandle;

pub(super) use self::discovery::get_installed_apps;
pub(super) use self::haptics::perform_haptic;

use objc2_service_management::SMAppService;

pub fn start_at_login() {
    unsafe {
        SMAppService::mainAppService().registerAndReturnError().ok();
    }
}

pub fn stop_at_login() {
    unsafe {
        SMAppService::mainAppService()
            .unregisterAndReturnError()
            .ok();
    }
}

pub fn get_autostart_status() -> bool {
    unsafe {
        SMAppService::mainAppService()
            .registerAndReturnError()
            .ok()
            .is_some()
    }
}

/// This sets the activation policy of the app to Accessory, allowing rustcast to be visible ontop
/// of fullscreen apps
pub(super) fn set_activation_policy_accessory() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApp, NSApplicationActivationPolicy};

    let mtm = MainThreadMarker::new().expect("must be on main thread");
    let app = NSApp(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

/// This carries out the window configuration for the macos window (only things that are macos specific)
pub(super) fn macos_window_config(handle: &WindowHandle) {
    use iced::wgpu::rwh::RawWindowHandle;
    use objc2::rc::Retained;
    use objc2_app_kit::NSView;

    match handle.as_raw() {
        RawWindowHandle::AppKit(handle) => {
            let ns_view = handle.ns_view.as_ptr();
            let ns_view: Retained<NSView> = unsafe { Retained::retain(ns_view.cast()) }.unwrap();
            let ns_window = ns_view
                .window()
                .expect("view was not installed in a window");

            use objc2_app_kit::{NSFloatingWindowLevel, NSWindowCollectionBehavior};
            ns_window.setLevel(NSFloatingWindowLevel);

            ns_window.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces);
        }
        _ => {
            panic!(
                "Why are you running this as a non-appkit window? this is a macos only app as of now"
            );
        }
    }
}

/// This is the function that forces focus onto rustcast
#[allow(deprecated)]
pub(super) fn focus_this_app() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApp;

    let mtm = MainThreadMarker::new().expect("must be on main thread");
    let app = NSApp(mtm);

    app.activateIgnoringOtherApps(true);
}

/// This is the struct that represents the process serial number, allowing us to transform the process to a UI element
#[repr(C)]
struct ProcessSerialNumber {
    low: u32,
    hi: u32,
}

/// Check whether the system is in dark mode via NSUserDefaults.
pub fn is_dark_mode() -> bool {
    use objc2_foundation::{NSUserDefaults, ns_string};
    let defaults = NSUserDefaults::standardUserDefaults();
    defaults
        .stringForKey(ns_string!("AppleInterfaceStyle"))
        .map(|s| s.to_string() == "Dark")
        .unwrap_or(false)
}

/// Simulates Cmd+V (paste) targeted at the given process by PID.
/// Uses CGEventPostToPid so focus transfer timing is irrelevant.
pub fn simulate_paste(pid: libc::pid_t) {
    use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let source_ref = source.as_deref();
    let v_keycode: u16 = 9; // kVK_ANSI_V

    if let Some(keydown) = CGEvent::new_keyboard_event(source_ref, v_keycode, true) {
        CGEvent::set_flags(Some(&keydown), CGEventFlags::MaskCommand);
        CGEvent::post_to_pid(pid, Some(&keydown));
    }
    if let Some(keyup) = CGEvent::new_keyboard_event(source_ref, v_keycode, false) {
        CGEvent::set_flags(Some(&keyup), CGEventFlags::MaskCommand);
        CGEvent::post_to_pid(pid, Some(&keyup));
    }
}

/// This is the function that transforms the process to a UI element, and hides the dock icon
///
/// see mostly <https://github.com/electron/electron/blob/e181fd040f72becd135db1fa977622b81da21643/shell/browser/browser_mac.mm#L512C1-L532C2>
///
/// returns ApplicationServices OSStatus (u32)
///
/// doesn't seem to do anything if you haven't opened a window yet, so wait to call it until after that.
pub(super) fn transform_process_to_ui_element() -> u32 {
    use objc2_application_services::{
        TransformProcessType, kCurrentProcess, kProcessTransformToUIElementApplication,
    };
    use std::ptr;

    let psn = ProcessSerialNumber {
        low: 0,
        hi: kCurrentProcess,
    };

    unsafe {
        TransformProcessType(
            ptr::from_ref(&psn).cast(),
            kProcessTransformToUIElementApplication,
        )
    }
}
