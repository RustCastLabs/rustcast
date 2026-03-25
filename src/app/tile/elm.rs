//! This module handles the logic for the new and view functions according to the elm
//! architecture. If the subscription function becomes too large, it should be moved to this file

use global_hotkey::hotkey::HotKey;
use iced::border::Radius;
use iced::widget::scrollable::{Anchor, Direction, Scrollbar};
use iced::widget::text::LineHeight;
use iced::widget::{Column, Row, Scrollable, Text, container, space};
use iced::{Alignment, Color, Length, Vector, window};
use iced::{Element, Task};
use iced::{Length::Fill, widget::text_input};

use log::info;
use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSliceMut;

use crate::app::pages::emoji::emoji_page;
use crate::app::pages::settings::settings_page;
use crate::app::tile::{AppIndex, Hotkeys};
use crate::app::{DEFAULT_WINDOW_HEIGHT, ToApp, ToApps};
use crate::config::Theme;
use crate::debounce::Debouncer;
use crate::styles::{
    contents_style, glass_border, glass_surface, results_scrollbar_style, rustcast_text_input_style,
};
use crate::{app::WINDOW_WIDTH, platform};
use crate::{app::pages::clipboard::clipboard_view, platform::get_installed_apps};
use crate::{
    app::{Message, Page, apps::App, default_settings, tile::Tile},
    config::Config,
    platform::transform_process_to_ui_element,
};

/// Initialise the base window
pub fn new(hotkey: HotKey, config: &Config) -> (Tile, Task<Message>) {
    let (id, open) = window::open(default_settings());
    info!("Opening window");

    let open = open.discard().chain(window::run(id, |handle| {
        platform::window_config(&handle.window_handle().expect("Unable to get window handle"));
        transform_process_to_ui_element();
    }));
    info!("MacOS platform config applied");

    let store_icons = config.theme.show_icons;

    let mut options = get_installed_apps(store_icons);

    options.extend(config.shells.iter().map(|x| x.to_app()));
    info!("Loaded shell commands");

    options.extend(config.modes.to_apps());
    info!("Loaded modes");

    options.extend(App::basic_apps());
    info!("Loaded basic apps / default apps");
    options.par_sort_by_key(|x| x.display_name.len());
    let options = AppIndex::from_apps(options);

    let hotkeys = Hotkeys {
        toggle: hotkey,
        clipboard_hotkey: config
            .clipboard_hotkey
            .parse()
            .unwrap_or("SUPER+SHIFT+C".parse().unwrap()),
    };

    (
        Tile {
            update_available: false,
            current_mode: "Default".to_string(),
            query: String::new(),
            query_lc: String::new(),
            focus_id: 0,
            results: vec![],
            options,
            emoji_apps: AppIndex::from_apps(App::emoji_apps()),
            hotkeys,
            visible: true,
            frontmost: None,
            focused: false,
            config: config.clone(),
            theme: config.theme.to_owned().clone().into(),
            clipboard_content: vec![],
            tray_icon: None,
            sender: None,
            page: Page::Main,
            height: DEFAULT_WINDOW_HEIGHT,
            file_search_sender: None,
            debouncer: Debouncer::new(config.debounce_delay),
        },
        Task::batch([open.map(|_| Message::OpenWindow)]),
    )
}

/// The elm View function that renders the entire rustcast window
pub fn view(tile: &Tile, wid: window::Id) -> Element<'_, Message> {
    if tile.visible {
        let title_input = text_input(tile.config.placeholder.as_str(), &tile.query)
            .on_input(move |a| Message::SearchQueryChanged(a, wid))
            .on_paste(move |a| Message::SearchQueryChanged(a, wid))
            .font(tile.config.theme.font())
            .on_submit(Message::OpenFocused)
            .id("query")
            .width(Fill)
            .line_height(LineHeight::Relative(1.75))
            .style(move |_, _| rustcast_text_input_style(&tile.config.theme))
            .padding(20);

        let scrollbar_direction =
            if !tile.config.theme.show_scroll_bar || tile.page == Page::Settings {
                Direction::Vertical(Scrollbar::hidden())
            } else {
                Direction::Vertical(
                    Scrollbar::new()
                        .width(1)
                        .scroller_width(1.1)
                        .anchor(Anchor::Start),
                )
            };

        let results = match tile.page {
            Page::ClipboardHistory => clipboard_view(
                tile.clipboard_content.clone(),
                tile.focus_id,
                tile.config.theme.clone(),
            ),
            Page::EmojiSearch => emoji_page(
                tile.config.theme.clone(),
                tile.emoji_apps
                    .search_prefix(&tile.query_lc)
                    .map(|x| x.to_owned())
                    .collect(),
                tile.focus_id,
            ),
            Page::Settings => settings_page(tile.config.clone()),
            Page::FileSearch | Page::Main => container(Column::from_iter(
                tile.results.iter().enumerate().map(|(i, app)| {
                    app.clone().render(
                        tile.config.theme.clone(),
                        i as u32,
                        tile.focus_id,
                        Some(Message::OpenResult(i as u32)),
                    )
                }),
            ))
            .into(),
        };

        let results_count = match &tile.page {
            Page::Main | Page::EmojiSearch | Page::FileSearch => tile.results.len(),
            Page::ClipboardHistory => tile.clipboard_content.len(),
            Page::Settings => 0,
        };

        // This determines the height of the scrollable window
        let height = match tile.page {
            Page::ClipboardHistory | Page::Settings => 385,
            // Height of each emoji is EMOJI_HEIGHT + 20 for padding
            Page::EmojiSearch => std::cmp::min(tile.results.len().div_ceil(6) * 90, 290),
            _ => std::cmp::min(tile.results.len() * 60, 290),
        };

        let theme = tile.config.theme.clone();
        let scrollable = Scrollable::with_direction(results, scrollbar_direction)
            .style(move |_, _| results_scrollbar_style(&theme))
            .id("results")
            .height(height as u32);

        let text = if tile.query_lc.is_empty() {
            if tile.page == Page::Main {
                "Frequently used".to_string()
            } else {
                tile.page.to_string()
            }
        } else {
            match results_count {
                1 => "1 result found".to_string(),
                0 => "No results found".to_string(),
                count => {
                    format!("{count} results found")
                }
            }
        };

        let contents = container(
            Column::new()
                .push(title_input)
                .push(scrollable)
                .push(footer(
                    tile.config.theme.clone(),
                    tile.current_mode.clone(),
                    text,
                ))
                .spacing(0),
        )
        .style(|_| container::Style {
            text_color: None,
            background: None,
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.,
                radius: Radius::new(15),
            },
            ..Default::default()
        });

        container(contents)
            .style(|_| contents_style(&tile.config.theme))
            .into()
    } else {
        space().into()
    }
}

/// The footer at the bottom displaying the mode and results found, and its styling
fn footer(theme: Theme, current_mode: String, text: String) -> Element<'static, Message> {
    let radius = 15.0;

    let current_mode = format!(
        "{}{} Mode",
        current_mode.split_at(1).0.to_uppercase(),
        current_mode.split_at(1).1
    );
    container(
        Row::new()
            .push(
                Text::new(text)
                    .size(12)
                    .height(30)
                    .color(theme.text_color(0.7))
                    .font(theme.font())
                    .align_y(Alignment::Center)
                    .align_x(Alignment::Center),
            )
            .push(
                Text::new(current_mode)
                    .size(12)
                    .height(30)
                    .color(theme.text_color(0.7))
                    .font(theme.font())
                    .width(Fill)
                    .align_y(Alignment::Center)
                    .align_x(Alignment::End),
            )
            .align_y(Alignment::Center)
            .padding(4)
            .width(Fill)
            .height(Fill),
    )
    .align_y(Alignment::Center)
    .center(Length::Fill)
    .width(WINDOW_WIDTH)
    .padding(5)
    .height(30)
    .style(move |_| container::Style {
        text_color: None,
        background: Some(iced::Background::Color(glass_surface(
            theme.bg_color(),
            false,
        ))),
        border: iced::Border {
            color: glass_border(theme.text_color(1.0), false),
            width: 0.,
            radius: Radius::new(radius).top(0.0),
        },

        shadow: iced::Shadow {
            color: Color::TRANSPARENT,
            offset: Vector::ZERO,
            blur_radius: 0.,
        },
        snap: false,
    })
    .into()
}
