//! Main logic for the app
use std::collections::HashMap;

use crate::app::apps::{App, AppCommand, ICNS_ICON};
use crate::commands::Function;
use crate::config::Config;
use crate::debounce::DebouncePolicy;
use crate::utils::icns_data_to_handle;
use crate::{app::tile::ExtSender, clipboard::ClipBoardContentType};
use iced::time::Duration;

pub mod apps;
pub mod menubar;
pub mod pages;
pub mod tile;

use iced::window::{self, Id, Settings};
/// The default window width
pub const WINDOW_WIDTH: f32 = 500.;

/// The default window height
pub const DEFAULT_WINDOW_HEIGHT: f32 = 100.;

/// Maximum file search results returned by a single mdfind invocation.
pub const FILE_SEARCH_MAX_RESULTS: u32 = 400;

/// Number of results to accumulate before flushing a batch to the UI.
pub const FILE_SEARCH_BATCH_SIZE: u32 = 10;

/// The rustcast descriptor name to be put for all rustcast commands
pub const RUSTCAST_DESC_NAME: &str = "Utility";

/// The different pages that rustcast can have / has
#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Main,
    FileSearch,
    ClipboardHistory,
    EmojiSearch,
    Settings,
}

impl std::fmt::Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.to_owned() {
            Page::Main => "App search",
            Page::FileSearch => "File search",
            Page::EmojiSearch => "Emoji search",
            Page::ClipboardHistory => "Clipboard history",
            Page::Settings => "Settings",
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

#[derive(Debug, Clone)]
pub enum Editable<T> {
    Create(T),
    Delete(T),
    Update { old: T, new: T },
}

/// The message type that iced uses for actions that can do something
#[derive(Debug, Clone)]
pub enum Message {
    WriteConfig(bool),
    UpdateAvailable,
    ResizeWindow(Id, f32),
    OpenWindow,
    OpenResult(u32),
    OpenToSettings,
    SearchQueryChanged(String, Id),
    KeyPressed(u32),
    FocusTextInput(Move),
    HideWindow(Id),
    RunFunction(Function),
    OpenFocused,
    SetConfig(SetConfigFields),
    OpenFileDialogue(String),
    ReturnFocus,
    EscKeyPressed(Id),
    ClearSearchResults,
    WindowFocusChanged(Id, bool),
    ClearSearchQuery,
    HideTrayIcon,
    SwitchMode(String),
    ReloadConfig,
    UpdateApps,
    SetSender(ExtSender),
    SwitchToPage(Page),
    EditClipboardHistory(Editable<ClipBoardContentType>),
    ClearClipboardHistory,
    ChangeFocus(ArrowKey, u32),
    FileSearchResult(Vec<App>),
    FileSearchClear,
    SetFileSearchSender(tokio::sync::watch::Sender<(String, Vec<String>)>),
    DebouncedSearch(Id),
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum SetConfigFields {
    ToDefault,
    ToggleHotkey(String),
    ClipboardHotkey(String),
    PlaceHolder(String),
    SearchUrl(String),
    HapticFeedback(bool),
    ShowMenubarIcon(bool),
    Modes(Editable<(String, String)>),
    Aliases(Editable<(String, String)>),
    SearchDirs(Editable<Vec<String>>),
    DebounceDelay(u64),
    SetThemeFields(SetConfigThemeFields),
    SetBufferFields(SetConfigBufferFields),
}

#[derive(Debug, Clone)]
pub enum SetConfigThemeFields {
    ShowScrollBar(bool),
    TextColor(f32, f32, f32),
    BackgroundColor(f32, f32, f32),
    ShowIcons(bool),
    Font(String),
}

#[derive(Debug, Clone)]
pub enum SetConfigBufferFields {
    ClearOnHide(bool),
    ClearOnEnter(bool),
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
            });
        };

        to_apps
    }
}

impl DebouncePolicy for Page {
    fn debounce_delay(&self, config: &Config) -> Option<Duration> {
        match self {
            Page::Main | Page::ClipboardHistory | Page::Settings => None,
            Page::FileSearch | Page::EmojiSearch => {
                Some(Duration::from_millis(config.debounce_delay))
            }
        }
    }
}
