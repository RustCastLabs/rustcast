use std::sync::{Arc, Mutex};

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

use crate::{app::{Message, tile::ExtSender}, platform::macos::accessibility::ensure_accessibility_permission};

pub fn global_handler(sender: ExtSender) {
    ensure_accessibility_permission();
    local_handler(sender.clone());
    let mask = NSEventMask::KeyDown | NSEventMask::FlagsChanged;
    let sender = Arc::new(Mutex::new(sender.0.clone()));

    let block = RcBlock::new({
        move |event: std::ptr::NonNull<NSEvent>| {
            let event = unsafe { event.as_ref() };
            let event_type = event.r#type();

            let key_code = event.keyCode();
            let mods = event.modifierFlags()
                & (NSEventModifierFlags::Command
                    | NSEventModifierFlags::Option
                    | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Function
                    | NSEventModifierFlags::CapsLock
                    | NSEventModifierFlags::Shift);

            let shortcut = match event_type {
                NSEventType::KeyDown => Shortcut {
                    key_code: Some(key_code),
                    mods: if mods.0 != 0 { Some(mods.0) } else { None },
                },
                NSEventType::FlagsChanged => Shortcut {
                    key_code: None,
                    mods: if mods.0 != 0 { Some(mods.0) } else { None },
                },
                _ => return,
            };

            let mut s = sender.lock().unwrap();
            let _ = s.try_send(Message::KeyPressed(shortcut));
        }
    });

    NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block);
}

pub fn local_handler(sender: ExtSender) {
    let mask = NSEventMask::KeyDown | NSEventMask::FlagsChanged;
    let sender = Arc::new(Mutex::new(sender.0.clone()));

    let block = RcBlock::new({
        move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
            let event_ref = unsafe { event.as_ref() };
            let event_type = event_ref.r#type();

            let key_code = event_ref.keyCode();
            let mods = event_ref.modifierFlags()
                & (NSEventModifierFlags::Command
                    | NSEventModifierFlags::Option
                    | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Function
                    | NSEventModifierFlags::CapsLock
                    | NSEventModifierFlags::Shift);

            let shortcut = match event_type {
                NSEventType::KeyDown => Shortcut {
                    key_code: Some(key_code),
                    mods: if mods.0 != 0 { Some(mods.0) } else { None },
                },
                NSEventType::FlagsChanged => Shortcut {
                    key_code: None,
                    mods: if mods.0 != 0 { Some(mods.0) } else { None },
                },
                _ => return event.as_ptr(), // pass through unhandled events
            };

            let mut s = sender.lock().unwrap();
            let _ = s.try_send(Message::KeyPressed(shortcut));

            event.as_ptr()
        }
    });

    unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block);
    }
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
