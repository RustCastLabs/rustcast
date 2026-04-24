//! This handles the update logic for the tile (AKA rustcast's main window)
use std::cmp::min;
use std::collections::HashMap;
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
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSliceMut;
use url::Url;

use crate::app::Editable;
use crate::app::SetConfigBufferFields;
use crate::app::SetConfigFields;
use crate::app::SetConfigThemeFields;
use crate::app::ToApp;
use crate::app::ToApps;
use crate::app::WINDOW_WIDTH;
use crate::app::apps::App;
use crate::app::apps::AppCommand;
use crate::app::default_settings;
use crate::app::menubar::menu_builder;
use crate::app::menubar::menu_icon;
use crate::app::tile::AppIndex;
use crate::app::{Message, Page, tile::Tile};
use crate::calculator::Expr;
use crate::commands::Function;
use crate::config::Config;
use crate::config::MainPage;
use crate::debounce::DebouncePolicy;
use crate::platform::macos::events::Event;
use crate::platform::macos::launching::Shortcut;
use crate::platform::macos::launching::global_handler;
use crate::platform::macos::{start_at_login, stop_at_login};
use crate::quit::get_open_apps;
use crate::unit_conversion;
use crate::utils::is_valid_url;
use crate::{app::ArrowKey, platform::focus_this_app};
use crate::{app::DEFAULT_WINDOW_HEIGHT, platform::perform_haptic};
use crate::{app::Move, platform::HapticPattern};
use crate::{app::RUSTCAST_DESC_NAME, platform::get_installed_apps};

fn extract_target(url: &Url) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == "target")
        .map(|(_, value)| value.into_owned())
}

/// Handle the "elm" update
pub fn handle_update(tile: &mut Tile, message: Message) -> Task<Message> {
    match message {
        Message::OpenWindow => {
            tile.capture_frontmost();
            focus_this_app();
            tile.focused = true;
            tile.visible = true;

            if tile.page == Page::Main && tile.query_lc.is_empty() {
                window::latest()
                    .map(|x| x.unwrap())
                    .map(|id| Message::SearchQueryChanged(String::new(), id))
            } else {
                Task::none()
            }
        }

        Message::UpdateEvents => {
            tile.events = Event::get_events(tile.config.event_duration);
            Task::none()
        }

        Message::UriReceived(uri) => {
            let Ok(url) = Url::parse(&uri) else {
                return Task::none();
            };

            match url.host_str().unwrap_or("") {
                "open" => extract_target(&url)
                    .and_then(|x| tile.options.by_name.get(&x).map(|x| x.to_owned()))
                    .map(|app| match app.open_command {
                        AppCommand::Function(a) => Task::done(Message::RunFunction(a)),
                        AppCommand::Display => Task::none(),
                        AppCommand::Message(msg) => Task::done(msg),
                    })
                    .unwrap_or(Task::none()),

                "show" => open_window(DEFAULT_WINDOW_HEIGHT),

                "quit" => Task::done(Message::RunFunction(Function::Quit)),

                _ => Task::none(),
            }
        }

        Message::UpdateAvailable => {
            tile.update_available = true;
            Task::done(Message::ReloadConfig)
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
                Task::none()
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
            global_handler(sender.clone());
            if tile.config.show_trayicon {
                tile.tray_icon = Some(menu_icon(tile.config.clone(), sender));
            }
            Task::none()
        }

        Message::ToggleAutoStartup(set_to) => {
            if set_to {
                start_at_login();
                tile.config.start_at_login = true
            } else {
                stop_at_login();
                tile.config.start_at_login = false
            }
            Task::none()
        }

        Message::EscKeyPressed(id) => {
            if !tile.query_lc.is_empty() {
                return Task::batch([
                    Task::done(Message::ClearSearchQuery),
                    Task::done(Message::ClearSearchResults),
                ]);
            }

            match tile.page {
                Page::Main => {}
                Page::Settings => {
                    return Task::done(Message::WriteConfig(true));
                }
                _ => {
                    return Task::done(Message::SwitchToPage(Page::Main));
                }
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

        Message::ChangeFocus(key, amount) => {
            let mut return_task = Task::none();
            for _ in 0..amount {
                let len = match tile.page {
                    Page::ClipboardHistory => tile.clipboard_content.len() as u32,
                    Page::EmojiSearch => {
                        tile.emoji_apps.search_prefix(&tile.query_lc).count() as u32
                    } // or tile.results.len()
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
                    Page::Main | Page::FileSearch | Page::ClipboardHistory => 66.5,
                    Page::EmojiSearch => 5.,
                    Page::Settings => 0.,
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

                return_task = Task::batch([
                    task,
                    operation::scroll_to(
                        "results",
                        AbsoluteOffset {
                            x: None,
                            y: Some(y),
                        },
                    ),
                ]);
            }
            return_task
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
        Message::LoadRanking => {
            for (name, rank) in &tile.ranking {
                tile.options.set_ranking(name, rank.to_owned());
            }

            Task::none()
        }

        Message::SaveRanking => {
            tile.ranking = tile.options.get_rankings();
            let string_rep = toml::to_string(&tile.ranking).unwrap_or("".to_string());
            let ranking_file_path =
                std::env::var("HOME").unwrap_or("/".to_string()) + "/.config/rustcast/ranking.toml";
            fs::write(ranking_file_path, string_rep).ok();
            Task::none()
        }

        Message::OpenFocused => Task::done(Message::OpenResult(tile.focus_id)),
        Message::OpenResult(id) => open_result(tile, id as usize),

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

            if let Ok(hotkey) = Shortcut::parse(&new_config.clipboard_hotkey) {
                tile.hotkeys.clipboard_hotkey = hotkey
            }

            if let Ok(hotkey) = Shortcut::parse(&new_config.toggle_hotkey) {
                tile.hotkeys.toggle = hotkey
            }

            let mut shell_map = HashMap::new();

            for shell in &new_config.shells {
                if let Some(hotkey) = shell.hotkey.clone().and_then(|x| Shortcut::parse(&x).ok()) {
                    shell_map.insert(hotkey, shell.clone());
                }
            }

            tile.hotkeys.shells = shell_map;

            let update_apps_task = if tile.config.shells != new_config.shells {
                info!("App Update required");
                Task::done(Message::UpdateApps)
            } else {
                Task::none()
            };

            if let Some(icon) = tile.tray_icon.as_mut() {
                icon.set_visible(new_config.clone().show_trayicon)
                    .unwrap_or(());
                icon.set_menu(Some(Box::new(menu_builder(
                    new_config.clone(),
                    tile.sender.clone().unwrap(),
                    tile.update_available,
                ))));
            } else {
                tile.tray_icon = Some(menu_icon(new_config.clone(), tile.sender.clone().unwrap()));
                tile.tray_icon
                    .as_mut()
                    .unwrap()
                    .set_visible(tile.config.show_trayicon)
                    .ok();
            }

            tile.theme = new_config.theme.to_owned().into();
            tile.config = new_config;
            Task::batch([Task::done(Message::LoadRanking), update_apps_task])
        }

        Message::KeyPressed(shortcut) => {
            if let Some(cmd) = tile.hotkeys.shells.get(&shortcut) {
                return Task::done(Message::RunFunction(Function::RunShellCommand(
                    cmd.command.clone(),
                )));
            }

            let is_clipboard_hotkey = shortcut == tile.hotkeys.clipboard_hotkey;
            let is_open_hotkey = shortcut == tile.hotkeys.toggle;

            let clipboard_page_task = if is_clipboard_hotkey {
                info!("Switching to clipboard page");
                Task::done(Message::SwitchToPage(Page::ClipboardHistory))
            } else if is_open_hotkey {
                info!("Switching to main page");
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

        Message::OpenToSettings => {
            tile.page = Page::Settings;
            Task::batch([
                Task::done(Message::OpenWindow),
                open_window(((7 * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32),
            ])
        }

        Message::SwitchToPage(page) => {
            let task = match &page {
                Page::ClipboardHistory => {
                    if !tile.config.cbhist {
                        return Task::none();
                    }
                    window::latest().map(|x| {
                        let id = x.unwrap();
                        Message::ResizeWindow(
                            id,
                            ((7 * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
                        )
                    })
                }
                Page::Settings => window::latest().map(|x| {
                    let id = x.unwrap();
                    Message::ResizeWindow(
                        id,
                        ((7 * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
                    )
                }),
                _ => Task::none(),
            };

            tile.page = page;

            let refresh_empty_main_query = if tile.page == Page::Main {
                window::latest()
                    .map(|x| x.unwrap())
                    .map(|id| Message::SearchQueryChanged(String::new(), id))
            } else {
                Task::none()
            };

            Task::batch([
                Task::done(Message::ClearSearchQuery),
                Task::done(Message::ClearSearchResults),
                task,
                refresh_empty_main_query,
            ])
        }

        Message::RunFunction(command) => {
            command.execute(&tile.config);
            let page_task = match tile.page {
                Page::Settings => Task::done(Message::SwitchToPage(Page::Main)),
                _ => Task::none(),
            };

            let return_focus_task = match &command {
                Function::OpenApp(_) | Function::GoogleSearch(_) => Task::none(),
                _ => Task::done(Message::ReturnFocus),
            };

            if !tile.config.buffer_rules.clear_on_enter || !tile.visible {
                return Task::none();
            }

            window::latest()
                .map(|x| x.unwrap())
                .map(Message::HideWindow)
                .chain(page_task)
                .chain(Task::done(Message::ClearSearchQuery))
                .chain(return_focus_task)
        }

        Message::HideWindow(a) => {
            if tile.page == Page::Settings {
                return Task::none();
            }
            info!("Hiding RustCast window");
            tile.visible = false;
            tile.focused = false;
            tile.page = Page::Main;
            tile.focus_id = 0;

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

        Message::ToggleFavouriteApp(app_name) => {
            let ranking = match tile.options.by_name.get(&app_name) {
                None => return Task::none(),
                Some(app) => {
                    if app.ranking == -1 {
                        0
                    } else {
                        -1
                    }
                }
            };
            tile.options.set_ranking(&app_name, ranking);
            Task::none()
        }

        Message::UpdateApps => {
            let mut new_options = get_installed_apps(tile.config.theme.show_icons);
            new_options.extend(tile.config.shells.iter().map(|x| x.to_app()));
            new_options.extend(tile.config.modes.to_apps());
            new_options.extend(App::basic_apps());
            new_options.par_sort_by_key(|x| x.display_name.len());
            tile.options = AppIndex::from_apps(new_options);

            let mut shell_map = HashMap::new();

            for shell in &tile.config.shells {
                if let Some(has_hk) = &shell.hotkey
                    && let Some(hotkey) = Shortcut::parse(has_hk).ok()
                {
                    shell_map.insert(hotkey, shell.clone());
                }
            }

            tile.hotkeys.shells = shell_map;

            Task::none()
        }

        Message::ClearSearchResults => {
            tile.results = Vec::new();
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

        Message::EditClipboardHistory(action) => {
            if !tile.config.cbhist {
                return Task::none();
            }
            match action {
                Editable::Create(content) => {
                    if !tile.clipboard_content.contains(&content) {
                        tile.clipboard_content.insert(0, content);
                        return Task::none();
                    }

                    let new_content_vec = tile
                        .clipboard_content
                        .par_iter()
                        .filter_map(|x| {
                            if *x == content {
                                None
                            } else {
                                Some(x.to_owned())
                            }
                        })
                        .collect();

                    tile.clipboard_content = new_content_vec;
                    tile.clipboard_content.insert(0, content);
                }
                Editable::Delete(content) => {
                    tile.clipboard_content = tile
                        .clipboard_content
                        .iter()
                        .filter_map(|x| {
                            if *x == content {
                                None
                            } else {
                                Some(x.to_owned())
                            }
                        })
                        .collect();
                }
                Editable::Update { old, new } => {
                    tile.clipboard_content = tile
                        .clipboard_content
                        .iter()
                        .map(|x| if x == &old { new.clone() } else { x.to_owned() })
                        .collect();
                }
            }
            Task::none()
        }

        Message::SetFileSearchSender(sender) => {
            tile.file_search_sender = Some(sender);
            Task::none()
        }

        Message::FileSearchResult(apps) => {
            assert!(apps.len() <= 50, "Batch must not exceed 50 results.");
            if tile.page == Page::FileSearch {
                let prev_display_count = std::cmp::min(5, tile.results.len());
                tile.results.extend(apps);
                let new_display_count = std::cmp::min(5, tile.results.len());
                // Only resize when the visible row count changes (max 5).
                if new_display_count != prev_display_count && new_display_count > 0 {
                    return window::latest().map(move |x| {
                        Message::ResizeWindow(
                            x.unwrap(),
                            ((new_display_count * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
                        )
                    });
                }
            }
            Task::none()
        }

        Message::FileSearchClear => {
            if tile.page == Page::FileSearch {
                tile.results.clear();
            }
            Task::none()
        }

        Message::SearchQueryChanged(input, id) => {
            tile.focus_id = 0;

            if tile.config.haptic_feedback {
                perform_haptic(HapticPattern::Alignment);
            }

            tile.query_lc = input.trim().to_lowercase();
            tile.query = input.clone();

            if let Some(alias) = tile.config.aliases.get(&input.trim().to_lowercase()) {
                tile.query_lc = alias.to_string();
            }

            // Return a task that waits for the debounce delay before executing search
            if let Some(delay) = tile.page.debounce_delay(&tile.config) {
                tile.debouncer.reset();
                Task::perform(
                    async move {
                        tokio::time::sleep(delay).await;
                        id
                    },
                    Message::DebouncedSearch,
                )
            } else {
                execute_query(tile, id)
            }
        }

        Message::OpenFileDialogue(mode_name) => rfd::FileDialog::new()
            .add_filter("shell", &["sh", "bash", "zsh"])
            .set_directory(
                std::env::var("HOME").unwrap_or("".to_string()) + "/.config/rustcast/config.toml",
            )
            .pick_file()
            .and_then(|path| {
                path.to_str().map(|path_str| {
                    Task::batch([
                        Task::done(Message::SetConfig(SetConfigFields::Modes(
                            Editable::Create((mode_name, path_str.to_string())),
                        ))),
                        Task::done(Message::WriteConfig(false)),
                    ])
                })
            })
            .unwrap_or(Task::none()),

        Message::SetConfig(config) => {
            let mut final_config = tile.config.clone();
            match config {
                SetConfigFields::ToggleHotkey(hk) => final_config.toggle_hotkey = hk,
                SetConfigFields::ClipboardHotkey(hk) => final_config.clipboard_hotkey = hk,
                SetConfigFields::ClipboardHistory(cbhist) => final_config.cbhist = cbhist,
                SetConfigFields::Modes(Editable::Create((key, value))) => {
                    final_config.modes.insert(key, value);
                }
                SetConfigFields::SetEventDuration(duration) => {
                    if duration.trim().is_empty() {
                        final_config.event_duration = 0;
                    } else if let Ok(duration) = duration.parse::<u32>() {
                        final_config.event_duration = duration;
                    }

                    tile.events = Event::get_events(final_config.event_duration);
                }
                SetConfigFields::Modes(Editable::Delete((key, _))) => {
                    final_config.modes.remove(&key);
                }
                SetConfigFields::Modes(Editable::Update { old, new }) => {
                    final_config.modes.remove(&old.0);
                    final_config.modes.insert(new.0, new.1);
                }
                SetConfigFields::Aliases(Editable::Create((key, value))) => {
                    final_config.aliases.entry(key).or_insert(value);
                }
                SetConfigFields::Aliases(Editable::Delete((key, _))) => {
                    final_config.aliases.remove(&key);
                }
                SetConfigFields::Aliases(Editable::Update { old, new }) => {
                    final_config.aliases.remove(&old.0);
                    final_config.aliases.insert(new.0, new.1);
                }
                SetConfigFields::SearchDirs(Editable::Create(dir)) => {
                    if !final_config.search_dirs.contains(&dir) {
                        final_config.search_dirs.push(dir);
                    }
                }
                SetConfigFields::SearchDirs(Editable::Delete(dirs)) => {
                    final_config.search_dirs = final_config
                        .search_dirs
                        .iter()
                        .filter_map(|dir| {
                            if &dirs != dir {
                                Some(dir.to_owned())
                            } else {
                                None
                            }
                        })
                        .collect();
                }
                SetConfigFields::SearchDirs(Editable::Update { old, new }) => {
                    final_config.search_dirs = final_config
                        .search_dirs
                        .iter()
                        .map(|dir| {
                            if dir == &old {
                                new.clone()
                            } else {
                                dir.to_owned()
                            }
                        })
                        .collect();
                }

                SetConfigFields::ShellCommands(Editable::Create(shell_command)) => {
                    if !final_config.shells.contains(&shell_command) {
                        final_config.shells.push(shell_command);
                    }
                }

                SetConfigFields::ShellCommands(Editable::Delete(shell_command)) => {
                    final_config.shells = final_config
                        .shells
                        .iter()
                        .filter_map(|shell| {
                            if &shell_command != shell {
                                Some(shell.to_owned())
                            } else {
                                None
                            }
                        })
                        .collect();
                }

                SetConfigFields::ShellCommands(Editable::Update { old, new }) => {
                    final_config.shells = final_config
                        .shells
                        .iter()
                        .map(|shell| {
                            if shell == &old {
                                new.clone()
                            } else {
                                shell.to_owned()
                            }
                        })
                        .collect();
                }

                SetConfigFields::SearchUrl(url) => final_config.search_url = url,
                SetConfigFields::PlaceHolder(placeholder) => final_config.placeholder = placeholder,
                SetConfigFields::SetPage(page) => final_config.main_page = page,
                SetConfigFields::DebounceDelay(delay) => final_config.debounce_delay = delay,
                SetConfigFields::HapticFeedback(haptic_feedback) => {
                    final_config.haptic_feedback = haptic_feedback
                }
                SetConfigFields::ShowMenubarIcon(show) => final_config.show_trayicon = show,
                SetConfigFields::SetThemeFields(SetConfigThemeFields::Font(fnt)) => {
                    final_config.theme.font = Some(fnt)
                }
                SetConfigFields::SetThemeFields(SetConfigThemeFields::TextColor(r, g, b)) => {
                    final_config.theme.text_color = (r, g, b)
                }
                SetConfigFields::SetThemeFields(SetConfigThemeFields::ShowIcons(icns)) => {
                    final_config.theme.show_icons = icns
                }
                SetConfigFields::SetThemeFields(SetConfigThemeFields::ShowScrollBar(show)) => {
                    final_config.theme.show_scroll_bar = show
                }
                SetConfigFields::SetThemeFields(SetConfigThemeFields::BackgroundColor(r, g, b)) => {
                    final_config.theme.background_color = (r, g, b)
                }
                SetConfigFields::SetBufferFields(SetConfigBufferFields::ClearOnHide(clear)) => {
                    final_config.buffer_rules.clear_on_hide = clear;
                }
                SetConfigFields::SetBufferFields(SetConfigBufferFields::ClearOnEnter(clear)) => {
                    final_config.buffer_rules.clear_on_enter = clear
                }
                SetConfigFields::ToDefault => {
                    final_config = Config::default();
                }
            };

            tile.config = final_config;
            Task::none()
        }

        Message::WriteConfig(page_switch) => {
            let config_file_path =
                std::env::var("HOME").unwrap_or("".to_string()) + "/.config/rustcast/config.toml";

            tile.config.aliases.remove("");
            tile.config.modes.remove("");

            let config_string = match toml::to_string_pretty(&tile.config) {
                Ok(a) => a,
                Err(e) => {
                    log::error!("Invalid config: {e}");
                    return Task::none();
                }
            };

            fs::write(config_file_path, config_string)
                .map_err(|e| {
                    log::error!("Error writing to config file: {e}");
                    log::error!("Config file changes not saved");
                    e
                })
                .ok();

            Task::batch([
                Task::done(Message::ReloadConfig),
                if page_switch {
                    Task::done(Message::SwitchToPage(Page::Main))
                } else {
                    Task::none()
                },
            ])
        }

        Message::ClearClipboardHistory => {
            tile.clipboard_content.clear();
            Task::none()
        }

        Message::DebouncedSearch(id) => {
            // Only execute if this is still the most recent debounce timer
            if !tile.debouncer.is_ready() {
                return Task::none();
            }

            execute_query(tile, id)
        }
    }
}

/// helper function for the tasks needed to open a window
fn open_window(height: f32) -> Task<Message> {
    Task::batch([
        window::open(default_settings())
            .1
            .map(move |id| Message::ResizeWindow(id, height)),
        Task::done(Message::OpenWindow),
        operation::focus("query"),
    ])
}

/// A helper function for resizing rustcast when only one result is found
fn single_item_resize_task(id: Id) -> Task<Message> {
    resize_task(id, 1)
}

/// A helper function for resizing rustcast when zero results are found
fn zero_item_resize_task(id: Id) -> Task<Message> {
    resize_task(id, 0)
}

fn resize_task(id: Id, count: u32) -> Task<Message> {
    Task::done(Message::ResizeWindow(
        id,
        (55 * count) as f32 + DEFAULT_WINDOW_HEIGHT,
    ))
}

fn resize_for_results_count(id: Id, count: usize) -> Task<Message> {
    if count == 0 {
        return zero_item_resize_task(id);
    }
    if count == 1 {
        return single_item_resize_task(id);
    }

    let max_elem = min(5, count);
    Task::done(Message::ResizeWindow(
        id,
        ((max_elem * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
    ))
}

fn open_result(tile: &mut Tile, id: usize) -> Task<Message> {
    let results = if tile.page == Page::ClipboardHistory {
        tile.clipboard_content
            .iter()
            .map(|x| x.to_app().to_owned())
            .collect()
    } else {
        tile.results.clone()
    };

    let Some(app) = results.get(id).cloned() else {
        return Task::none();
    };

    let search_name = app.search_name.clone();

    match app.open_command {
        AppCommand::Function(func) => {
            info!("Updating ranking for: {search_name}");
            tile.options.update_ranking(&search_name);
            Task::done(Message::RunFunction(func))
        }
        AppCommand::Message(msg) => {
            info!("Updating ranking for: {search_name}");
            tile.options.update_ranking(&search_name);
            Task::done(msg)
        }
        AppCommand::Display => Task::done(Message::ReturnFocus),
    }
}

/// Handling the lemon easter egg icon
fn lemon_icon_handle() -> Option<Handle> {
    image::ImageReader::new(Cursor::new(include_bytes!("../../../docs/lemon.png")))
        .with_guessed_format()
        .unwrap()
        .decode()
        .ok()
        .map(|img| Handle::from_rgba(img.width(), img.height(), img.into_bytes()))
}

fn execute_query(tile: &mut Tile, id: Id) -> Task<Message> {
    let mut task = Task::none();
    let prev_size = tile.results.len();

    match tile.page {
        Page::ClipboardHistory | Page::Settings => {
            if tile.query_lc != "main" {
                return Task::none();
            }
        }
        _ => {}
    }

    if tile.page == Page::Main && tile.query_lc.is_empty() {
        tile.results = match tile.config.main_page {
            MainPage::FrequentlyUsed => tile.frequent_results(),
            MainPage::Events => tile.events.iter().map(|x| x.to_app()).collect(),
            MainPage::Blank => vec![],
            MainPage::Favourites => tile.options.get_favourites(),
        };
        return resize_for_results_count(id, tile.results.len());
    }

    if tile.query_lc.is_empty()
        || (tile.query_lc.chars().count() < 2 && tile.page == Page::FileSearch)
    {
        tile.results = Vec::new();
        return zero_item_resize_task(id);
    };

    let quittables = if tile.query_lc.starts_with("quit") {
        get_open_apps(tile.config.theme.show_icons)
    } else {
        vec![]
    };

    match tile.query_lc.as_str() {
        "randomvar" => {
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
        }
        "zombo" => {
            tile.results = vec![App {
                ranking: 0,
                open_command: AppCommand::Function(Function::OpenWebsite(
                    "https://zombo.com".to_string(),
                )),
                desc: "Easter Egg".to_string(),
                icons: None,
                display_name: "🫳 🌱".to_string(),
                search_name: "".to_string(),
            }];
            return single_item_resize_task(id);
        }
        "lemon" => {
            tile.results = vec![App {
                ranking: 0,
                open_command: AppCommand::Display,
                desc: "Easter Egg".to_string(),
                icons: lemon_icon_handle(),
                display_name: "Lemon".to_string(),
                search_name: "".to_string(),
            }];
            return single_item_resize_task(id);
        }
        "67" => {
            tile.results = vec![App {
                ranking: 0,
                open_command: AppCommand::Function(Function::RandomVar(67)),
                desc: "Easter egg".to_string(),
                icons: None,
                display_name: 67.to_string(),
                search_name: String::new(),
            }];
            return single_item_resize_task(id);
        }
        "cbhist" => {
            task = task.chain(Task::done(Message::SwitchToPage(Page::ClipboardHistory)));
        }
        "main" => {
            if tile.page != Page::Main {
                task = task.chain(Task::done(Message::SwitchToPage(Page::Main)));
                return Task::batch([zero_item_resize_task(id), task]);
            }
        }
        "fav" => {
            tile.results = tile.options.get_favourites();
            return resize_for_results_count(id, tile.results.len());
        }
        query => 'a: {
            if !query.starts_with(">") || tile.page != Page::Main {
                break 'a;
            }
            let command = tile.query.strip_prefix(">").unwrap_or("");
            tile.results = vec![App {
                ranking: 20,
                open_command: AppCommand::Function(Function::RunShellCommand(command.to_string())),
                display_name: format!("Shell Command: {}", command),
                icons: None,
                search_name: "".to_string(),
                desc: "Shell Command".to_string(),
            }];
            return single_item_resize_task(id);
        }
    }

    match tile.page {
        Page::FileSearch => {
            if let Some(ref sender) = tile.file_search_sender {
                tile.results.clear();
                sender
                    .send((tile.query_lc.clone(), tile.config.search_dirs.clone()))
                    .ok();
            }

            return task;
        }
        _ => tile.handle_search_query_changed(),
    }

    tile.handle_search_query_changed();
    if tile.query_lc.starts_with("quit") {
        let query = tile.query_lc.clone();
        tile.results.extend(quittables.iter().filter_map(move |x| {
            if x.search_name.starts_with(&query) {
                Some(x.to_owned())
            } else {
                None
            }
        }))
    }

    if !tile.results.is_empty() {
        tile.results.par_sort_by_key(|x| -x.ranking);

        let new_length = tile.results.len();
        let max_elem = min(5, new_length);

        if prev_size == new_length {
            return task;
        }

        return task.chain(Task::batch([
            Task::done(Message::ResizeWindow(
                id,
                ((max_elem * 55) + 35 + DEFAULT_WINDOW_HEIGHT as usize) as f32,
            )),
            Task::done(Message::ChangeFocus(ArrowKey::Left, 1)),
        ]));
    }

    if is_valid_url(&tile.query) {
        tile.results.push(App {
            ranking: 0,
            open_command: AppCommand::Function(Function::OpenWebsite(tile.query.clone())),
            desc: "Web Browsing".to_string(),
            icons: None,
            display_name: "Open Website: ".to_string() + &tile.query,
            search_name: String::new(),
        });
    } else if let Some(conversions) = unit_conversion::convert_query(&tile.query) {
        tile.results = conversions
            .into_iter()
            .map(|conversion| conversion.to_app())
            .collect();
        return single_item_resize_task(id);
    } else if let Ok(res) = Expr::from_str(&tile.query) {
        tile.results.push(App {
            ranking: 0,
            open_command: AppCommand::Function(Function::Calculate(res.clone())),
            desc: RUSTCAST_DESC_NAME.to_string(),
            icons: None,
            display_name: res.eval().map(|x| x.to_string()).unwrap_or("".to_string()),
            search_name: "".to_string(),
        });
        return single_item_resize_task(id);
    } else if tile.query.ends_with("?") || tile.query.split_whitespace().nth(2).is_some() {
        tile.results = vec![App {
            ranking: 0,
            open_command: AppCommand::Function(Function::GoogleSearch(tile.query.clone())),
            icons: None,
            desc: "Web Search".to_string(),
            display_name: format!("Search for: {}", tile.query),
            search_name: String::new(),
        }];
        return single_item_resize_task(id);
    }
    task
}
