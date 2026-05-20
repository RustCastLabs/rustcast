//! This modules handles the logic for each "app" that rustcast can load
//!
//! An "app" is effectively, one of the results that rustcast returns when you search for something

use std::io::Cursor;

use iced::{
    Alignment,
    Length::{self, Fill},
    widget::{
        Button, Row, Text, container,
        image::{Handle, Viewer},
        text::Wrapping,
    },
};

use crate::{
    app::{Message, Page, RUSTCAST_DESC_NAME},
    clipboard::ClipBoardContentType,
    commands::Function,
    styles::{favourite_button_style, result_button_style, result_row_container_style},
    utils::icns_data_to_handle,
};

/// The rustcast icns icons bytes
pub const ICNS_ICON: &[u8] = include_bytes!("../../docs/icon.icns");

/// This tells each "App" what to do when it is clicked, whether it is a function, a message, or a display
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppCommand {
    Function(Function),
    Message(Message),
    Display,
}

/// The main app struct, that represents an "App"
///
/// This struct represents a command that rustcast can perform, providing the rustcast
/// the data needed to search for the app, to display the app in search results, and to actually
/// "run" the app.
#[derive(Debug, Clone)]
pub struct App {
    pub ranking: i32,
    pub open_command: AppCommand,
    pub desc: String,
    pub icons: Option<iced::widget::image::Handle>,
    pub display_name: String,
    pub search_name: String,
}

impl PartialEq for App {
    fn eq(&self, other: &Self) -> bool {
        self.search_name == other.search_name
            && self.icons == other.icons
            && self.desc == other.desc
            && self.display_name == other.display_name
    }
}

impl App {
    pub fn new(name: String, icon: Option<Handle>, desc: String, command: AppCommand) -> Self {
        Self {
            ranking: 0,
            open_command: command,
            icons: icon,
            search_name: name.to_lowercase(),
            display_name: name,
            desc,
        }
    }
    /// A vec of all the emojis as App structs
    pub fn emoji_apps() -> Vec<App> {
        emojis::iter()
            .filter(|x| x.unicode_version() < emojis::UnicodeVersion::new(17, 13))
            .map(|x| App {
                ranking: 0,
                icons: None,
                display_name: x.to_string(),
                search_name: x.name().to_string(),
                open_command: AppCommand::Function(Function::CopyToClipboard(
                    ClipBoardContentType::Text(x.to_string()),
                )),
                desc: x.name().to_string(),
            })
            .collect()
    }
    /// This returns the basic apps that rustcast has, such as quiting rustcast and opening preferences
    pub fn basic_apps() -> Vec<App> {
        let app_version = option_env!("APP_VERSION").unwrap_or("Unknown Version");

        let icons = icns_data_to_handle(ICNS_ICON.to_vec());

        let ferris_handle =
            image::ImageReader::new(Cursor::new(include_bytes!("../../docs/ferris_rs.png")))
                .with_guessed_format()
                .unwrap()
                .decode()
                .ok()
                .map(|img| Handle::from_rgba(img.width(), img.height(), img.into_bytes()));

        vec![
            App {
                ranking: 0,
                open_command: AppCommand::Function(Function::OpenWebsite(
                    "https://ferris.rs".to_string(),
                )),
                icons: ferris_handle,
                desc: "Easter Egg".to_string(),
                display_name: "Ferris Plushies".to_string(),
                search_name: "ferris.rs".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Function(Function::Quit),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Quit RustCast".to_string(),
                search_name: "quit".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Function(Function::QuitAllApps),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Quit All Apps".to_string(),
                search_name: "quit all apps".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Message(Message::SwitchToPage(Page::Settings)),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Open RustCast Preferences".to_string(),
                search_name: "settings".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Message(Message::SwitchToPage(Page::EmojiSearch)),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Search for an Emoji".to_string(),
                search_name: "emoji".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Message(Message::SwitchToPage(Page::ClipboardHistory)),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Clipboard History".to_string(),
                search_name: "clipboard".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Message(Message::SwitchToPage(Page::FileSearch)),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Search for a file".to_string(),
                search_name: "file search".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Message(Message::ReloadConfig),
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: "Reload RustCast".to_string(),
                search_name: "refresh".to_string(),
            },
            App {
                ranking: 0,
                open_command: AppCommand::Display,
                desc: RUSTCAST_DESC_NAME.to_string(),
                icons: icons.clone(),
                display_name: format!("Current RustCast Version: {app_version}"),
                search_name: "version".to_string(),
            },
        ]
    }

    /// This renders the app into an iced element, allowing it to be displayed in the search results
    pub fn render(
        self,
        theme: crate::config::Theme,
        id_num: u32,
        focussed_id: u32,
        on_press: Option<Message>,
    ) -> iced::Element<'static, Message> {
        let focused = focussed_id == id_num;

        // Title + subtitle (Raycast style)
        let text_block = iced::widget::Column::new()
            .spacing(2)
            .push(
                Text::new(self.display_name)
                    .font(theme.font())
                    .size(16)
                    .wrapping(Wrapping::None)
                    .color(theme.text_color(1.0)),
            )
            .push(
                Text::new(self.desc)
                    .font(theme.font())
                    .size(13)
                    .color(theme.text_color(0.55)),
            );

        let mut row = Row::new()
            .align_y(Alignment::Center)
            .width(Fill)
            .spacing(10)
            .height(50);

        if theme.show_icons
            && let Some(icon) = &self.icons
        {
            row = row.push(
                container(Viewer::new(icon).height(40).width(40))
                    .width(40)
                    .height(40),
            );
        }
        row = row.push(container(text_block).width(Fill));

        let name = self.search_name.clone();
        let theme_clone = theme.clone();
        let is_favourite = self.ranking == -1;
        row = row.push(
            Button::new(Text::new("♥️").width(Length::Fill).align_x(Alignment::End))
                .on_press_with(move || Message::ToggleFavouriteApp(name.clone()))
                .width(Length::Fill)
                .style(move |_, status| favourite_button_style(&theme_clone, status, is_favourite)),
        );

        let msg = on_press.or(match self.open_command.clone() {
            AppCommand::Function(func) => Some(Message::RunFunction(func)),
            AppCommand::Message(msg) => Some(msg),
            AppCommand::Display => None,
        });

        let theme_clone = theme.clone();

        let content = Button::new(row)
            .on_press_maybe(msg)
            .style(move |_, _| result_button_style(&theme_clone))
            .width(Fill)
            .padding(0)
            .height(50);

        container(content)
            .id(format!("result-{}", id_num))
            .style(move |_| result_row_container_style(&theme, focused))
            .padding(8)
            .width(Fill)
            .into()
    }
}
