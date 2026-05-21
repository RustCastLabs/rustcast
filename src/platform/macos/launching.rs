use std::{
    ffi::c_void,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use objc2_app_kit::NSEventModifierFlags;
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes,
};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType,
};

use crate::{
    app::{Message, tile::ExtSender},
    platform::macos::accessibility::ensure_accessibility_permission,
};

#[derive(Clone, Debug)]
pub struct EventTapHandle {
    tap_port: CFRetained<CFMachPort>,
    loop_source: CFRetained<CFRunLoopSource>,
    callback_data: *mut c_void,
}

impl Drop for EventTapHandle {
    fn drop(&mut self) {
        CGEvent::tap_enable(&self.tap_port, false);

        let run_loop = CFRunLoop::main().expect("Failed to get main CFRunLoop");
        run_loop.remove_source(Some(&self.loop_source), unsafe { kCFRunLoopCommonModes });

        // Free the callback data
        if !self.callback_data.is_null() {
            unsafe {
                drop(Box::from_raw(self.callback_data as *mut CallbackData));
            }
        }
    }
}

extern "C-unwind" fn keyboard_event_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    mut event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    if user_info.is_null() {
        log::error!("Null user_info in keyboard_event_callback");
        return unsafe { event.as_mut() };
    }

    let data = unsafe { &*(user_info as *const CallbackData) };

    let key_code: u16 = unsafe {
        CGEvent::integer_value_field(Some(event.as_ref()), CGEventField::KeyboardEventKeycode)
    } as u16;

    let flags: CGEventFlags = unsafe { CGEvent::flags(Some(event.as_ref())) };

    let mut mods = NSEventModifierFlags::empty();

    if flags.contains(CGEventFlags::MaskCommand) {
        mods |= NSEventModifierFlags::Command;
    }
    if flags.contains(CGEventFlags::MaskAlternate) {
        mods |= NSEventModifierFlags::Option;
    }
    if flags.contains(CGEventFlags::MaskControl) {
        mods |= NSEventModifierFlags::Control;
    }
    if flags.contains(CGEventFlags::MaskShift) {
        mods |= NSEventModifierFlags::Shift;
    }
    if flags.contains(CGEventFlags::MaskAlphaShift) {
        mods |= NSEventModifierFlags::CapsLock;
    }
    if flags.contains(CGEventFlags::MaskSecondaryFn) {
        mods |= NSEventModifierFlags::Function;
    }

    let shortcut = match event_type {
        CGEventType::KeyDown => Shortcut {
            key_code: Some(key_code),
            mods: if mods.0 != 0 { Some(mods.0) } else { None },
        },
        CGEventType::FlagsChanged => {
            let is_press = match key_code {
                56 | 60 => flags.contains(CGEventFlags::MaskShift), // LSHIFT | RSHIFT
                59 | 62 => flags.contains(CGEventFlags::MaskControl), // LCTRL  | RCTRL
                58 | 61 => flags.contains(CGEventFlags::MaskAlternate), // LOPT   | ROPT
                55 | 54 => flags.contains(CGEventFlags::MaskCommand), // LCMD   | RCMD
                63 => flags.contains(CGEventFlags::MaskSecondaryFn), // FN
                57 => flags.contains(CGEventFlags::MaskAlphaShift), // CAPSLOCK
                _ => false,
            };

            if !is_press {
                return unsafe { event.as_mut() };
            }

            let self_flag = match key_code {
                56 | 60 => NSEventModifierFlags::Shift,   // LSHIFT | RSHIFT
                59 | 62 => NSEventModifierFlags::Control, // LCTRL  | RCTRL
                58 | 61 => NSEventModifierFlags::Option,  // LOPT   | ROPT
                55 | 54 => NSEventModifierFlags::Command, // LCMD   | RCMD
                63 => NSEventModifierFlags::Function,     // FN
                57 => NSEventModifierFlags::CapsLock,     // CAPSLOCK
                _ => NSEventModifierFlags::empty(),
            };

            mods.remove(self_flag);

            Shortcut {
                key_code: Some(key_code),
                mods: if mods.is_empty() { None } else { Some(mods.0) },
            }
        }
        _ => return unsafe { event.as_mut() },
    };

    if !data.targets.iter().any(|t| *t == shortcut) {
        return unsafe { event.as_mut() };
    }

    if let Ok(mut sender) = data.sender.lock() {
        sender.0.try_send(Message::KeyPressed(shortcut)).unwrap();
    }

    unsafe { event.as_mut() }
}

pub struct CallbackData {
    sender: Arc<Mutex<ExtSender>>,
    targets: Vec<Shortcut>,
}

pub fn global_handler(sender: ExtSender, targets: Vec<Shortcut>) -> Result<EventTapHandle, String> {
    ensure_accessibility_permission(); // make it return Result

    let callback_data = Box::new(CallbackData {
        sender: Arc::new(Mutex::new(sender)),
        targets,
    });
    let user_info = Box::into_raw(callback_data) as *mut c_void;

    let mask =
        (1u64 << CGEventType::KeyDown.0 as u64) | (1u64 << CGEventType::FlagsChanged.0 as u64);

    let tap_port = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            Some(keyboard_event_callback),
            user_info,
        )
    }
    .unwrap();

    let loop_source = CFMachPort::new_run_loop_source(None, Some(&tap_port), 0)
        .ok_or_else(|| "Failed to create run loop source".to_string())?;

    let run_loop = CFRunLoop::main().ok_or_else(|| "Failed to get main run loop".to_string())?;
    run_loop.add_source(Some(&loop_source), unsafe { kCFRunLoopCommonModes });

    CGEvent::tap_enable(&tap_port, true);

    Ok(EventTapHandle {
        tap_port,
        loop_source,
        callback_data: user_info,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key_code: Option<u16>,
    pub mods: Option<usize>,
}

impl Shortcut {
    pub fn new(key_code: Option<u16>, mods: Option<usize>) -> Self {
        Self { key_code, mods }
    }

    pub fn parse(s: &str) -> Result<Shortcut, String> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();

        let mut mods: usize = 0;
        let mut key_code: Option<u16> = None;
        let mut has_mods = false;

        for part in &parts {
            match part.to_lowercase().as_str() {
                "cmd" | "command" | "super" => {
                    mods |= NSEventModifierFlags::Command.0;
                    has_mods = true;
                }
                "opt" | "option" | "alt" => {
                    mods |= NSEventModifierFlags::Option.0;
                    has_mods = true;
                }
                "capslock" | "caps" | "caps lock" => mods |= NSEventModifierFlags::CapsLock.0,
                "ctrl" | "control" => {
                    mods |= NSEventModifierFlags::Control.0;
                    has_mods = true;
                }
                "shift" => {
                    mods |= NSEventModifierFlags::Shift.0;
                    has_mods = true;
                }
                "fn" | "function" => {
                    mods |= NSEventModifierFlags::Function.0;
                    has_mods = true;
                }
                key => {
                    if key_code.is_some() {
                        return Err(format!("Multiple keys specified: '{}'", s));
                    }
                    key_code = Some(str_to_keycode(key)?);
                }
            }
        }

        Ok(Shortcut::new(
            key_code,
            if has_mods { Some(mods) } else { None },
        ))
    }
}

fn str_to_keycode(s: &str) -> Result<u16, String> {
    let code = match s.to_lowercase().as_str() {
        // Letters
        "a" => 0x00,
        "s" => 0x01,
        "d" => 0x02,
        "f" => 0x03,
        "h" => 0x04,
        "g" => 0x05,
        "z" => 0x06,
        "x" => 0x07,
        "c" => 0x08,
        "v" => 0x09,
        "b" => 0x0b,
        "q" => 0x0c,
        "w" => 0x0d,
        "e" => 0x0e,
        "r" => 0x0f,
        "y" => 0x10,
        "t" => 0x11,
        "o" => 0x1f,
        "u" => 0x20,
        "i" => 0x22,
        "p" => 0x23,
        "l" => 0x25,
        "j" => 0x26,
        "k" => 0x28,
        "n" => 0x2d,
        "m" => 0x2e,

        // Numbers
        "1" => 0x12,
        "2" => 0x13,
        "3" => 0x14,
        "4" => 0x15,
        "5" => 0x17,
        "6" => 0x16,
        "7" => 0x1a,
        "8" => 0x1c,
        "9" => 0x19,
        "0" => 0x1d,

        // Special keys
        "return" | "enter" => 0x24,
        "tab" => 0x30,
        "space" => 0x31,
        "delete" | "backspace" => 0x33,
        "escape" | "esc" => 0x35,
        "left" | "arrowleft" => 0x7b,
        "right" | "arrowright" => 0x7c,
        "down" | "arrowdown" => 0x7d,
        "up" | "arrowup" => 0x7e,
        "home" => 0x73,
        "end" => 0x77,
        "pageup" => 0x74,
        "pagedown" => 0x79,

        // Function keys
        "f1" => 0x7a,
        "f2" => 0x78,
        "f3" => 0x63,
        "f4" => 0x76,
        "f5" => 0x60,
        "f6" => 0x61,
        "f7" => 0x62,
        "f8" => 0x64,
        "f9" => 0x65,
        "f10" => 0x6d,
        "f11" => 0x67,
        "f12" => 0x6f,

        // Symbols
        "-" | "minus" => 0x1b,
        "=" | "equal" => 0x18,
        "[" | "bracketleft" => 0x21,
        "]" | "bracketright" => 0x1e,
        "\\" | "backslash" => 0x2a,
        ";" | "semicolon" => 0x29,
        "'" | "quote" => 0x27,
        "`" | "backquote" | "grave" => 0x32,
        "," | "comma" => 0x2b,
        "." | "period" => 0x2f,
        "/" | "slash" => 0x2c,

        _ => return Err(format!("Unknown key: '{}'", s)),
    };

    Ok(code)
}
