//! Main logic for the app
use std::collections::HashMap;

use crate::app::apps::{App, AppCommand, ICNS_ICON};
use crate::commands::Function;
use crate::utils::icns_data_to_handle;
use crate::{app::tile::ExtSender, clipboard::ClipBoardContentType};

pub mod apps;
pub mod menubar;
pub mod pages;
pub mod tile;

use iced::window::{self, Id, Settings};
/// The default window width
pub const WINDOW_WIDTH: f32 = 500.;

/// The default window height
pub const DEFAULT_WINDOW_HEIGHT: f32 = 100.;

/// The rustcast descriptor name to be put for all rustcast commands
pub const RUSTCAST_DESC_NAME: &str = "Utility";

/// The different pages that rustcast can have / has
#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Main,
    FileSearch,
    ClipboardHistory,
    EmojiSearch,
}

impl std::fmt::Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.to_owned() {
            Page::Main => "App search",
            Page::FileSearch => "File search",
            Page::EmojiSearch => "Emoji search",
            Page::ClipboardHistory => "Clipboard history",
        })
    }
}

/// The types of arrow keys
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ArrowKey {
    Up,
    Down,
    Left,
    Right,
}

/// The ways the cursor can move when a key is pressed
#[derive(Debug, Clone)]
pub enum Move {
    Back,
    Forwards(String),
}

/// The message type that iced uses for actions that can do something
#[derive(Debug, Clone)]
pub enum Message {
    UpdateAvailable,
    ResizeWindow(Id, f32),
    OpenWindow,
    SearchQueryChanged(String, Id),
    KeyPressed(u32),
    FocusTextInput(Move),
    HideWindow(Id),
    RunFunction(Function),
    OpenFocused,
    ReturnFocus,
    EscKeyPressed(Id),
    ClearSearchResults,
    WindowFocusChanged(Id, bool),
    ClearSearchQuery,
    HideTrayIcon,
    SwitchMode(String),
    ReloadConfig,
    SetSender(ExtSender),
    SwitchToPage(Page),
    ClipboardHistory(ClipBoardContentType),
    ChangeFocus(ArrowKey, u32),
    AiQuery(String),
    AiResponse(String),
}

/// The window settings for rustcast
pub fn default_settings() -> Settings {
    Settings {
        resizable: false,
        decorations: false,
        minimizable: false,
        level: window::Level::AlwaysOnTop,
        transparent: true,
        blur: true,
        size: iced::Size {
            width: WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
        },
        ..Default::default()
    }
}

/// A Trait to define that a struct can be converted to an app
pub trait ToApp {
    /// Convert self into an app
    fn to_app(&self) -> App;
}

/// A Trait to define that a type (containing multiple elements) can be converted to multiple Apps
///
/// i.e. [`Vec<Box<dyn ToApp>>`] can implement ToApps but it doesn't make sense to do that
pub trait ToApps {
    /// convert self into a Vec of apps
    fn to_apps(&self) -> Vec<App>;
}

/// [`HashMap<String, String>`] is for storing the modes, and is an assumtion that the String
/// values are shell commands
impl ToApps for HashMap<String, String> {
    fn to_apps(&self) -> Vec<App> {
        let icons = icns_data_to_handle(ICNS_ICON.to_vec());

        let mut to_apps: Vec<App> = self
            .keys()
            .map(|key| {
                let display_name = format!(
                    "{}{} Mode",
                    key.split_at(1).0.to_uppercase(),
                    key.split_at(1).1
                );
                App {
                    ranking: 0,
                    open_command: apps::AppCommand::Message(Message::SwitchMode(
                        key.trim().to_owned(),
                    )),
                    search_name: key.to_owned(),
                    is_ai_response: false,
                    desc: "Switch Modes".to_string(),
                    icons: icons.clone(),
                    display_name,
                }
            })
            .collect();

        if self.get("default").is_none() {
            to_apps.push(App {
                ranking: 0,
                open_command: AppCommand::Message(Message::SwitchMode("Default".to_string())),
                desc: "Change mode".to_string(),
                icons: icons.clone(),
                display_name: "Default mode".to_string(),
                search_name: "default".to_string(),
                is_ai_response: false,
            });
        };

        to_apps
    }
}
