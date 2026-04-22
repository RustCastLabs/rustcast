//! macOS URL scheme handler for `nexus://` deep links
//!
//! On macOS, clicking a `nexus://` link delivers the URL via Apple Events
//! (`kInternetEventClass` / `kAEGetURL`), not as a command-line argument.
//!
//! This module registers a handler with `NSAppleEventManager` to receive
//! those events and forwards URLs through a crossbeam channel consumed by
//! an async stream subscription.
//!
//! **Why NSAppleEventManager instead of NSApplicationDelegate?**
//! Iced/winit owns the `NSApplication` delegate for window and input event
//! handling. Replacing it with our own delegate breaks the entire event
//! chain and causes crashes. `NSAppleEventManager` hooks into URL delivery
//! at a lower level without touching the delegate.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{NSObject, NSObjectProtocol};
use once_cell::sync::Lazy;

use crate::app::Message;

/// Channel for forwarding URLs from the Apple Event handler to the Iced event loop.
///
/// `crossbeam_channel` is used because both `Sender` and `Receiver` are
/// `Send + Sync`, which is required for use in a `static`. The standard
/// library's `mpsc::Receiver` is not `Sync` and would fail to compile.
static URL_CHANNEL: Lazy<(Sender<String>, Receiver<String>)> =
    Lazy::new(crossbeam_channel::unbounded);

/// Flag set during app shutdown so the `spawn_blocking` recv loop can exit.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Apple Event FourCharCode for `kInternetEventClass` and `kAEGetURL` (both `'GURL'`).
const K_AE_GET_URL: u32 = u32::from_be_bytes(*b"GURL");

/// Apple Event FourCharCode for `keyDirectObject` (`'----'`), the parameter
/// key that contains the URL string in a "get URL" event.
const KEY_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NexusURLHandler"]
    struct UrlHandler;

    unsafe impl NSObjectProtocol for UrlHandler {}

    /// Handler method registered with `NSAppleEventManager`. The selector
    /// `handleGetURLEvent:withReplyEvent:` matches what AppKit expects for
    /// Apple Event callbacks.
    impl UrlHandler {
        #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
        fn handle_get_url_event(&self, event: &AnyObject, _reply: &AnyObject) {
            // event is an NSAppleEventDescriptor. Extract the direct object
            // parameter which contains the URL as an NSAppleEventDescriptor,
            // then get its stringValue (an NSString).
            let descriptor: *mut AnyObject =
                unsafe { msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT] };
            if descriptor.is_null() {
                return;
            }
            let ns_string: *mut AnyObject = unsafe { msg_send![&*descriptor, stringValue] };
            if ns_string.is_null() {
                return;
            }
            let utf8: *const std::ffi::c_char = unsafe { msg_send![&*ns_string, UTF8String] };
            if utf8.is_null() {
                return;
            }
            let url_str = unsafe { std::ffi::CStr::from_ptr(utf8) }
                .to_string_lossy()
                .to_string();
            if url_str.to_lowercase().starts_with("rustcast://") {
                let _ = URL_CHANNEL.0.send(url_str);
            }
        }
    }
);

impl UrlHandler {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // SAFETY: `Self::alloc(mtm)` returns a valid allocated instance of our
        // NSObject subclass. Sending `init` to a freshly allocated NSObject
        // subclass with no custom ivars is the standard Objective-C
        // initialisation pattern and always succeeds.
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

/// Install the macOS URL scheme handler via `NSAppleEventManager`.
///
/// Must be called **after** the Iced/winit event loop has been created
/// (i.e. from `NexusApp::new()`), so that AppKit is fully initialized.
///
/// Registers for `kInternetEventClass` / `kAEGetURL` events, which macOS
/// sends when a `nexus://` URL is opened (clicked in browser, Finder, etc.).
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("macos_url: not on main thread, skipping URL handler install");
        return;
    };

    let handler = UrlHandler::new(mtm);

    // Get [NSAppleEventManager sharedAppleEventManager]
    let mgr: *mut AnyObject = unsafe {
        msg_send![
            objc2::runtime::AnyClass::get(c"NSAppleEventManager")
                .expect("NSAppleEventManager class not found"),
            sharedAppleEventManager
        ]
    };
    assert!(
        !mgr.is_null(),
        "macos_url: sharedAppleEventManager returned nil"
    );

    // Register: [mgr setEventHandler:handler
    //                 andSelector:@selector(handleGetURLEvent:withReplyEvent:)
    //                 forEventClass:kInternetEventClass
    //                 andEventID:kAEGetURL]
    let handler_sel = sel!(handleGetURLEvent:withReplyEvent:);
    unsafe {
        let _: () = msg_send![
            &*mgr,
            setEventHandler: &*handler,
            andSelector: handler_sel,
            forEventClass: K_AE_GET_URL,
            andEventID: K_AE_GET_URL
        ];
    }

    // Leak the handler so it lives for the entire process.
    //
    // `UrlHandler` is `MainThreadOnly` (`!Send + !Sync`), so
    // `Retained<UrlHandler>` cannot be stored in a `static`. Leaking
    // is the standard pattern for process-lifetime Objective-C objects.
    //
    // Unlike `NSApplication.delegate` (which is weak), the Apple Event
    // Manager retains a strong reference — but leaking is still correct
    // because we never want to unregister the handler.
    std::mem::forget(handler);
}

/// Signal the URL stream to stop so the `spawn_blocking` task can exit
/// and tokio's runtime drop won't hang.
///
/// Must be called before `iced::window::close()` on macOS.
#[allow(unused)]
pub fn shutdown() {
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
}

/// Async stream that yields URLs received via Apple Events.
///
/// Uses `recv_timeout` inside `spawn_blocking` so the blocking thread
/// wakes periodically and can exit when the tokio runtime shuts down
/// (e.g., on app quit). Without this, `recv()` blocks indefinitely and
/// causes a hang on macOS during quit.
pub fn url_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::futures::stream::unfold((), |()| async {
        let url = tokio::task::spawn_blocking(|| {
            loop {
                if SHUTTING_DOWN.load(Ordering::Relaxed) {
                    return None;
                }
                match URL_CHANNEL.1.recv_timeout(Duration::from_millis(500)) {
                    Ok(url) => return Some(url),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return None,
                }
            }
        })
        .await
        .ok()
        .flatten()?;

        Some((Message::UriReceived(url), ()))
    })
}
