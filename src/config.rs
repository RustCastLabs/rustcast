//! This is the config file type definitions for rustcast
use std::{collections::HashMap, path::Path, sync::Arc};

use iced::{Font, font::Family, theme::Custom, widget::image::Handle};
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        ToApp,
        apps::{App, AppCommand},
    },
    commands::Function,
    utils::handle_from_icns,
};

/// The main config struct (effectively the config file's "schema")
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub toggle_hotkey: String,
    pub clipboard_hotkey: String,
    pub buffer_rules: Buffer,
    pub main_page: MainPage,
    pub theme: Theme,
    pub placeholder: String,
    pub search_url: String,
    pub haptic_feedback: bool,
    pub show_trayicon: bool,
    pub shells: Vec<Shelly>,
    pub modes: HashMap<String, String>,
    pub aliases: HashMap<String, String>,
    pub search_dirs: Vec<String>,
    pub log_path: String,
    pub debounce_delay: u64,
}

impl Default for Config {
    /// The default config
    fn default() -> Self {
        Self {
            toggle_hotkey: "ALT+SPACE".to_string(),
            clipboard_hotkey: "SUPER+SHIFT+C".to_string(),
            buffer_rules: Buffer::default(),
            theme: Theme::default(),
            placeholder: String::from("Time to be productive!"),
            search_url: "https://duckduckgo.com/search?q=%s".to_string(),
            haptic_feedback: false,
            show_trayicon: true,
            main_page: MainPage::default(),
            search_dirs: vec!["~".to_string()],
            log_path: "/tmp/rustcast.log".to_string(),
            modes: HashMap::new(),
            aliases: HashMap::new(),
            shells: vec![],
            debounce_delay: 300,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default, Eq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum MainPage {
    Favourites,
    FrequentlyUsed,
    #[default]
    Blank,
}

/// The settings you can set for the theme
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Theme {
    pub text_color: (f32, f32, f32),
    pub background_color: (f32, f32, f32),
    pub blur: bool,
    pub show_icons: bool,
    pub show_scroll_bar: bool,
    pub font: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text_color: (0.95, 0.95, 0.96),
            background_color: (0.0, 0.0, 0.0),
            blur: false,
            show_icons: true,
            show_scroll_bar: false,
            font: None,
        }
    }
}

impl From<Theme> for iced::Theme {
    fn from(value: Theme) -> Self {
        let palette = iced::theme::Palette {
            background: value.bg_color(),
            text: value.text_color(1.),
            primary: iced::Color {
                r: 0.22,
                g: 0.55,
                b: 0.96,
                a: 1.0,
            },
            danger: iced::Color {
                r: 0.95,
                g: 0.26,
                b: 0.21,
                a: 1.0,
            },
            warning: iced::Color {
                r: 1.0,
                g: 0.76,
                b: 0.03,
                a: 1.0,
            },
            success: iced::Color {
                r: 0.30,
                g: 0.69,
                b: 0.31,
                a: 1.0,
            },
        };
        iced::Theme::Custom(Arc::new(Custom::new("RustCast Theme".to_string(), palette)))
    }
}

impl Theme {
    /// Return the text color in the theme config of type [`iced::Color`]
    pub fn text_color(&self, opacity: f32) -> iced::Color {
        let theme = self.to_owned();
        iced::Color {
            r: theme.text_color.0,
            g: theme.text_color.1,
            b: theme.text_color.2,
            a: opacity,
        }
    }

    /// Return the background color in the theme config of type [`iced::Color`]
    pub fn bg_color(&self) -> iced::Color {
        iced::Color {
            r: self.background_color.0,
            g: self.background_color.1,
            b: self.background_color.2,
            a: 0.,
        }
    }

    /// Return the font in the theme config of type [`iced::Font`]
    pub fn font(&self) -> Font {
        let opt_font_name = self.font.clone();
        match opt_font_name {
            Some(font_name) => Font {
                family: Family::Name(font_name.leak()),
                ..Default::default()
            },
            None => Font {
                family: Family::SansSerif,
                ..Default::default()
            },
        }
    }
}

/// The rules for the buffer AKA search results
///
/// - clear_on_hide is whether the buffer should be cleared when the window is hidden
/// - clear_on_enter is whether the buffer should be cleared when the user presses enter after
///   searching
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Buffer {
    pub clear_on_hide: bool,
    pub clear_on_enter: bool,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            clear_on_hide: true,
            clear_on_enter: true,
        }
    }
}

/// Command is the command it will run when the button is clicked
/// Icon_path is the path to an icon, but this is optional
/// Alias is the text that is used to call this command / search for it
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Shelly {
    pub command: String,
    pub icon_path: Option<String>,
    pub alias: String,
    pub alias_lc: String,
    pub hotkey: Option<String>,
}

impl ToApp for Shelly {
    fn to_app(&self) -> App {
        let self_clone = self.clone();
        let icon = self_clone.icon_path.and_then(|x| {
            let x = x.replace("~", &std::env::var("HOME").unwrap());
            if x.ends_with(".icns") {
                handle_from_icns(Path::new(&x))
            } else {
                Some(Handle::from_path(Path::new(&x)))
            }
        });
        App {
            ranking: 0,
            open_command: AppCommand::Function(Function::RunShellCommand(self_clone.command)),
            desc: "Shell Command".to_string(),
            icons: icon,
            display_name: self_clone.alias,
            search_name: self_clone.alias_lc,
        }
    }
}
