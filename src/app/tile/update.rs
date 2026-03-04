//! This handles the update logic for the tile (AKA rustcast's main window)
use std::cmp::min;
use std::fs;
use std::io::Cursor;
use std::thread;

use iced::Task;
use iced::widget::image::Handle;
use iced::widget::operation;
use iced::widget::operation::AbsoluteOffset;
use iced::window;
use iced::window::Id;
use log::info;
use rayon::slice::ParallelSliceMut;

use crate::app::ToApp;
use crate::app::ToApps;
use crate::app::WINDOW_WIDTH;
use crate::app::apps::App;
use crate::app::apps::AppCommand;
use crate::app::default_settings;
use crate::app::menubar::menu_icon;
use crate::app::tile::AppIndex;
use crate::app::{Message, Page, tile::Tile};
use crate::calculator::Expr;
use crate::clipboard::ClipBoardContentType;
use crate::commands::Function;
use crate::config::Config;
use crate::unit_conversion;
use crate::utils::is_valid_url;
use crate::{app::ArrowKey, platform::focus_this_app};
use crate::{app::DEFAULT_WINDOW_HEIGHT, platform::perform_haptic};
use crate::{app::Move, platform::HapticPattern};
use crate::{app::RUSTCAST_DESC_NAME, platform::get_installed_apps};

pub fn handle_update(tile: &mut Tile, message: Message) -> Task<Message> {
    match message {
        Message::OpenWindow => {
            tile.capture_frontmost();
            focus_this_app();
            tile.focused = true;
            tile.visible = true;
            Task::none()
        }

        Message::SwitchMode(mode) => {
            if let Some(command) = tile.config.modes.get(mode.trim()) {
                tile.current_mode = mode.clone();
                info!("Switched mode");
                Task::done(Message::RunFunction(Function::RunShellCommand(
                    command.to_owned(),
                )))
            } else {
                info!("Switching to default mode");
                tile.current_mode = "default".to_string();
                window::latest().map(|x| Message::HideWindow(x.unwrap()))
            }
        }

        Message::HideTrayIcon => {
            tile.tray_icon = None;
            tile.config.show_trayicon = false;
            let home = std::env::var("HOME").unwrap();
            let confg_str = toml::to_string(&tile.config).unwrap();
            thread::spawn(move || fs::write(home + "/.config/rustcast/config.toml", confg_str));
            Task::none()
        }

        Message::SetSender(sender) => {
            tile.sender = Some(sender.clone());
            if tile.config.show_trayicon {
                tile.tray_icon = Some(menu_icon(tile.config.clone(), sender));
            }
            Task::none()
        }

        Message::EscKeyPressed(id) => {
            if tile.page == Page::EmojiSearch && !tile.query_lc.is_empty() {
                return Task::none();
            }

            if tile.query_lc.is_empty() {
                Task::batch([
                    Task::done(Message::HideWindow(id)),
                    Task::done(Message::ReturnFocus),
                ])
            } else {
                tile.page = Page::Main;

                Task::batch(vec![
                    Task::done(Message::ClearSearchQuery),
                    Task::done(Message::ClearSearchResults),
                    window::resize(
                        id,
                        iced::Size {
                            width: WINDOW_WIDTH,
                            height: DEFAULT_WINDOW_HEIGHT,
                        },
                    ),
                ])
            }
        }

        Message::ClearSearchQuery => {
            tile.query_lc = String::new();
            tile.query = String::new();
            Task::none()
        }

        Message::ChangeFocus(key) => {
            let len = match tile.page {
                Page::ClipboardHistory => tile.clipboard_content.len() as u32,
                Page::EmojiSearch => tile.emoji_apps.search_prefix(&tile.query_lc).count() as u32, // or tile.results.len()
                _ => tile.results.len() as u32,
            };

            let old_focus_id = tile.focus_id;

            if len == 0 {
                return Task::none();
            }

            let change_by = match tile.page {
                Page::EmojiSearch => 6,
                _ => 1,
            };

            let task = match (&key, &tile.page) {
                (ArrowKey::Down, _) => {
                    tile.focus_id = (tile.focus_id + change_by) % len;
                    Task::none()
                }
                (ArrowKey::Up, _) => {
                    tile.focus_id = (tile.focus_id + len - change_by) % len;
                    Task::none()
                }
                (ArrowKey::Left, Page::EmojiSearch) => {
                    tile.focus_id = (tile.focus_id + len - 1) % len;
                    operation::focus("results")
                }
                (ArrowKey::Right, Page::EmojiSearch) => {
                    tile.focus_id = (tile.focus_id + 1) % len;
                    operation::focus("results")
                }
                _ => Task::none(),
            };

            let quantity = match tile.page {
                Page::Main => 66.5,
                Page::ClipboardHistory => 50.,
                Page::EmojiSearch => 5.,
            };

            let (wrapped_up, wrapped_down) = match &key {
                ArrowKey::Up => (tile.focus_id > old_focus_id, false),
                ArrowKey::Down => (false, tile.focus_id < old_focus_id),
                _ => (false, false),
            };

            let y = if wrapped_down {
                0.0
            } else if wrapped_up {
                (len.saturating_sub(1)) as f32 * quantity
            } else {
                tile.focus_id as f32 * quantity
            };

            Task::batch([
                task,
                operation::scroll_to(
                    "results",
                    AbsoluteOffset {
                        x: None,
                        y: Some(y),
                    },
                ),
            ])
        }

        Message::ResizeWindow(id, height) => {
            info!("Resizing rustcast window");
            tile.height = height;
            window::resize(
                id,
                iced::Size {
                    width: WINDOW_WIDTH,
                    height,
                },
            )
        }

        Message::OpenFocused => {
            // TODO: update ranking here
            match tile.results.get(tile.focus_id as usize) {
                Some(App {
                    search_name: name,
                    open_command: AppCommand::Function(func),
                    ..
                }) => {
                    info!("Updating ranking for: {name}");
                    tile.options.update_ranking(name);
                    Task::done(Message::RunFunction(func.to_owned()))
                }
                Some(App {
                    search_name: name,
                    open_command: AppCommand::Message(msg),
                    ..
                }) => {
                    info!("Updating ranking for: {name}");
                    tile.options.update_ranking(name);
                    Task::done(msg.to_owned())
                }
                Some(App {
                    open_command: AppCommand::Display,
                    ..
                }) => Task::done(Message::ReturnFocus),
                None => Task::none(),
            }
        }

        Message::ReloadConfig => {
            info!("Reloading config");
            let new_config: Config = match toml::from_str(
                &fs::read_to_string(
                    std::env::var("HOME").unwrap_or("".to_owned())
                        + "/.config/rustcast/config.toml",
                )
                .unwrap_or("".to_owned()),
            ) {
                Ok(a) => a,
                Err(_) => return Task::none(),
            };

            let mut new_options = get_installed_apps(new_config.theme.show_icons);
            new_options.extend(new_config.shells.iter().map(|x| x.to_app()));
            new_options.extend(new_config.modes.to_apps());
            new_options.extend(App::basic_apps());
            new_options.par_sort_by_key(|x| x.display_name.len());

            tile.tray_icon = if new_config.show_trayicon {
                Some(menu_icon(new_config.clone(), tile.sender.clone().unwrap()))
            } else {
                None
            };

            tile.theme = new_config.theme.to_owned().into();
            tile.config = new_config;
            tile.options = AppIndex::from_apps(new_options);
            Task::none()
        }

        Message::KeyPressed(hk_id) => {
            let is_clipboard_hotkey = tile.hotkeys.clipboard_hotkey.id == hk_id;
            let is_open_hotkey = hk_id == tile.hotkeys.toggle.id;

            let clipboard_page_task = if is_clipboard_hotkey {
                Task::done(Message::SwitchToPage(Page::ClipboardHistory))
            } else if is_open_hotkey {
                Task::done(Message::SwitchToPage(Page::Main))
            } else {
                Task::none()
            };

            if is_open_hotkey || is_clipboard_hotkey {
                if !tile.visible {
                    tile.height = if is_clipboard_hotkey {
                        ((7 * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32
                    } else {
                        DEFAULT_WINDOW_HEIGHT
                    };
                    return Task::batch([open_window(tile.height), clipboard_page_task]);
                }

                tile.visible = !tile.visible;

                let clear_search_query = if tile.config.buffer_rules.clear_on_hide {
                    Task::done(Message::ClearSearchQuery)
                } else {
                    Task::none()
                };

                let to_close = window::latest().map(|x| x.unwrap());
                Task::batch([
                    to_close.map(Message::HideWindow),
                    clear_search_query,
                    Task::done(Message::ReturnFocus),
                ])
            } else {
                Task::none()
            }
        }

        Message::SwitchToPage(page) => {
            tile.page = page;
            Task::batch([
                Task::done(Message::ClearSearchQuery),
                Task::done(Message::ClearSearchResults),
            ])
        }

        Message::RunFunction(command) => {
            command.execute(&tile.config);

            let return_focus_task = match &command {
                Function::OpenApp(_) | Function::OpenPrefPane | Function::GoogleSearch(_) => {
                    Task::none()
                }
                _ => Task::done(Message::ReturnFocus),
            };

            if tile.config.buffer_rules.clear_on_enter {
                window::latest()
                    .map(|x| x.unwrap())
                    .map(Message::HideWindow)
                    .chain(Task::done(Message::ClearSearchQuery))
                    .chain(return_focus_task)
            } else {
                Task::none()
            }
        }

        Message::HideWindow(a) => {
            info!("Hiding RustCast window");
            tile.visible = false;
            tile.focused = false;
            tile.page = Page::Main;
            Task::batch([window::close(a), Task::done(Message::ClearSearchResults)])
        }

        Message::ReturnFocus => {
            info!("Restoring frontmost app");
            tile.restore_frontmost();
            Task::none()
        }

        Message::FocusTextInput(update_query_char) => {
            match update_query_char {
                Move::Forwards(query_char) => {
                    tile.query += &query_char.clone();
                    tile.query_lc += &query_char.clone().to_lowercase();
                }
                Move::Back => {
                    tile.query.pop();
                    tile.query_lc.pop();
                }
            }
            let updated_query = tile.query.clone();
            Task::batch([
                operation::focus("query"),
                window::latest()
                    .map(|x| x.unwrap())
                    .map(move |x| Message::SearchQueryChanged(updated_query.clone(), x)),
            ])
        }

        Message::ClearSearchResults => {
            tile.results = vec![];
            Task::none()
        }
        Message::WindowFocusChanged(wid, focused) => {
            tile.focused = focused;
            if !focused {
                Task::done(Message::HideWindow(wid)).chain(Task::done(Message::ClearSearchQuery))
            } else {
                Task::none()
            }
        }

        Message::ClipboardHistory(content) => {
            tile.clipboard_content.insert(0, content);
            Task::none()
        }

        Message::SearchQueryChanged(input, id) => {
            let mut task = Task::none();
            tile.focus_id = 0;

            if tile.config.haptic_feedback {
                perform_haptic(HapticPattern::Alignment);
            }

            tile.query_lc = input.trim().to_lowercase();
            tile.query = input;
            let prev_size = tile.results.len();
            if tile.query_lc.is_empty() && tile.page != Page::ClipboardHistory {
                tile.results = vec![];
                return Task::done(Message::ResizeWindow(id, DEFAULT_WINDOW_HEIGHT));
            } else if tile.query_lc == "randomvar" {
                let rand_num = rand::random_range(0..100);
                tile.results = vec![App {
                    ranking: 0,
                    open_command: AppCommand::Function(Function::RandomVar(rand_num)),
                    desc: "Easter egg".to_string(),
                    icons: None,
                    display_name: rand_num.to_string(),
                    search_name: String::new(),
                }];
                return single_item_resize_task(id);
            } else if tile.query_lc == "67" {
                tile.results = vec![App {
                    ranking: 0,
                    open_command: AppCommand::Function(Function::RandomVar(67)),
                    desc: "Easter egg".to_string(),
                    icons: None,
                    display_name: 67.to_string(),
                    search_name: String::new(),
                }];
                return single_item_resize_task(id);
            } else if tile.query_lc.ends_with("?") {
                tile.results = vec![App {
                    ranking: 0,
                    open_command: AppCommand::Function(Function::GoogleSearch(tile.query.clone())),
                    icons: None,
                    desc: "Web Search".to_string(),
                    display_name: format!("Search for: {}", tile.query),
                    search_name: String::new(),
                }];
                return single_item_resize_task(id);
            } else if tile.query_lc == "cbhist" {
                task = task.chain(Task::done(Message::SwitchToPage(Page::ClipboardHistory)));
                tile.page = Page::ClipboardHistory
            } else if tile.query_lc == "main" && tile.page != Page::Main {
                task = task.chain(Task::done(Message::SwitchToPage(Page::Main)));
                tile.page = Page::Main;
            } else if tile.query_lc.starts_with(">") && tile.page == Page::Main {
                tile.results = vec![App {
                    ranking: 20,
                    open_command: AppCommand::Function(Function::RunShellCommand(
                        tile.query.strip_prefix(">").unwrap_or("").to_string(),
                    )),
                    display_name: format!(
                        "Shell Command: {}",
                        tile.query.strip_prefix(">").unwrap_or("")
                    ),
                    icons: None,
                    search_name: "".to_string(),
                    desc: "Shell Command".to_string(),
                }];
                return single_item_resize_task(id);
            }
            tile.handle_search_query_changed();

            if tile.results.is_empty()
                && let Some(res) = Expr::from_str(&tile.query).ok()
            {
                tile.results.push(App {
                    ranking: 0,
                    open_command: AppCommand::Function(Function::Calculate(res.clone())),
                    desc: RUSTCAST_DESC_NAME.to_string(),
                    icons: None,
                    display_name: res.eval().map(|x| x.to_string()).unwrap_or("".to_string()),
                    search_name: "".to_string(),
                });
            } else if tile.results.is_empty()
                && let Some(conversions) = unit_conversion::convert_query(&tile.query)
            {
                tile.results = conversions
                    .into_iter()
                    .map(|conversion| {
                        let source = format!(
                            "{} {}",
                            unit_conversion::format_number(conversion.source_value),
                            conversion.source_unit.name
                        );
                        let target = format!(
                            "{} {}",
                            unit_conversion::format_number(conversion.target_value),
                            conversion.target_unit.name
                        );
                        App {
                            ranking: 0,
                            open_command: AppCommand::Function(Function::CopyToClipboard(
                                ClipBoardContentType::Text(target.clone()),
                            )),
                            desc: source,
                            icons: None,
                            display_name: target,
                            search_name: String::new(),
                        }
                    })
                    .collect();
            } else if tile.results.is_empty() && is_valid_url(&tile.query) {
                tile.results.push(App {
                    ranking: 0,
                    open_command: AppCommand::Function(Function::OpenWebsite(tile.query.clone())),
                    desc: "Web Browsing".to_string(),
                    icons: None,
                    display_name: "Open Website: ".to_string() + &tile.query,
                    search_name: "".to_string(),
                });
            } else if tile.query_lc.split(' ').count() > 1 {
                tile.results.push(App {
                    ranking: 0,
                    open_command: AppCommand::Function(Function::GoogleSearch(tile.query.clone())),
                    icons: None,
                    desc: "Web Search".to_string(),
                    display_name: format!("Search for: {}", tile.query),
                    search_name: String::new(),
                });
            } else if tile.results.is_empty() && tile.query_lc == "lemon" {
                tile.results.push(App {
                    ranking: 0,
                    open_command: AppCommand::Display,
                    desc: "Easter Egg".to_string(),
                    icons: lemon_icon_handle(),
                    display_name: "Lemon".to_string(),
                    search_name: "".to_string(),
                });
            }
            if !tile.query_lc.is_empty() && tile.page == Page::EmojiSearch {
                tile.results = tile
                    .emoji_apps
                    .search_prefix("")
                    .map(|x| x.to_owned())
                    .collect();
            }

            tile.results.sort_by_key(|x| -x.ranking);

            let new_length = tile.results.len();
            let max_elem = min(5, new_length);

            if prev_size != new_length && tile.page != Page::ClipboardHistory {
                task.chain(Task::batch([
                    Task::done(Message::ResizeWindow(
                        id,
                        ((max_elem * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
                    )),
                    Task::done(Message::ChangeFocus(ArrowKey::Left)),
                ]))
            } else if tile.page == Page::ClipboardHistory {
                task.chain(Task::batch([
                    Task::done(Message::ResizeWindow(
                        id,
                        ((7 * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
                    )),
                    Task::done(Message::ChangeFocus(ArrowKey::Left)),
                ]))
            } else {
                task
            }
        }
    }
}

fn open_window(height: f32) -> Task<Message> {
    Task::batch([
        window::open(default_settings())
            .1
            .map(move |id| Message::ResizeWindow(id, height)),
        Task::done(Message::OpenWindow),
        operation::focus("query"),
    ])
}

fn single_item_resize_task(id: Id) -> Task<Message> {
    Task::done(Message::ResizeWindow(id, 55. + DEFAULT_WINDOW_HEIGHT))
}

fn lemon_icon_handle() -> Option<Handle> {
    image::ImageReader::new(Cursor::new(include_bytes!("../../../docs/lemon.png")))
        .with_guessed_format()
        .unwrap()
        .decode()
        .ok()
        .map(|img| Handle::from_rgba(img.width(), img.height(), img.into_bytes()))
}
