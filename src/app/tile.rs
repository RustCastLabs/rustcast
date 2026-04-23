//! This module handles the logic for the tile, AKA rustcast's main window
pub mod elm;
pub mod update;

use crate::app::apps::App;
use crate::app::{ArrowKey, Message, Move, Page};
use crate::clipboard::ClipBoardContentType;
use crate::config::{Config, Shelly};
use crate::debounce::Debouncer;
use crate::platform::default_app_paths;
use crate::platform::macos::launching::Shortcut;

use arboard::Clipboard;

use iced::futures::SinkExt;
use iced::futures::channel::mpsc::{Sender, channel};
use iced::keyboard::Modifiers;
use iced::{
    Subscription, Theme, futures,
    keyboard::{self, key::Named},
    stream,
};
use iced::{event, window};

use log::{info, warn};
use objc2::rc::Retained;
use objc2_app_kit::NSRunningApplication;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::slice::ParallelSliceMut;
use tokio::io::{AsyncBufReadExt, AsyncRead};
use tray_icon::TrayIcon;

use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use std::time::Duration;

/// This is a wrapper around the sender to disable dropping
#[derive(Clone, Debug)]
pub struct ExtSender(pub Sender<Message>);

/// Disable dropping the sender
impl Drop for ExtSender {
    fn drop(&mut self) {}
}

/// All the indexed apps that rustcast can search for
#[derive(Clone, Debug)]
struct AppIndex {
    by_name: HashMap<String, App>,
}

impl AppIndex {
    /// Search for an element in the index that starts with the provided prefix
    fn search_prefix<'a>(&'a self, prefix: &'a str) -> impl ParallelIterator<Item = &'a App> + 'a {
        self.by_name.par_iter().filter_map(move |(name, app)| {
            if name.starts_with(prefix)
                || name.contains(format!(" {prefix}").as_str())
                || name.contains(format!("-{prefix}").as_str())
            {
                Some(app)
            } else {
                None
            }
        })
    }

    fn update_ranking(&mut self, name: &str) {
        let app = match self.by_name.get_mut(name) {
            Some(a) => a,
            None => return,
        };

        app.ranking += 1;
    }

    fn set_ranking(&mut self, name: &str, rank: i32) {
        let app = match self.by_name.get_mut(name) {
            Some(a) => a,
            None => return,
        };

        app.ranking = rank;
    }

    fn get_rankings(&self) -> HashMap<String, i32> {
        HashMap::from_iter(self.by_name.iter().filter_map(|(name, app)| {
            if app.ranking > 0 {
                Some((name.to_owned(), app.ranking.to_owned()))
            } else {
                None
            }
        }))
    }

    fn top_ranked(&self, limit: usize) -> Vec<App> {
        let mut ranked: Vec<App> = self
            .by_name
            .values()
            .filter(|app| app.ranking > 0)
            .cloned()
            .collect();

        ranked.par_sort_by(|left, right| {
            right
                .ranking
                .cmp(&left.ranking)
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        ranked.truncate(limit);
        ranked
    }

    fn get_favourites(&self) -> Vec<App> {
        let mut favs: Vec<App> = self
            .by_name
            .values()
            .filter(|x| x.ranking == -1)
            .cloned()
            .collect();
        favs.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        favs
    }

    fn empty() -> AppIndex {
        AppIndex {
            by_name: HashMap::new(),
        }
    }

    /// Factory function for creating
    pub fn from_apps(options: Vec<App>) -> Self {
        let mut hmap = HashMap::new();
        for app in options {
            hmap.insert(app.search_name.clone(), app);
        }

        AppIndex { by_name: hmap }
    }
}

fn build_mdfind_args(query: &str, dirs: &[String], home_dir: &str) -> Option<Vec<String>> {
    assert!(query.len() < 1024, "Query too long.");
    if query.len() < 2 {
        return None;
    }

    let mut args = vec!["-name".to_string(), query.to_string()];
    for dir in dirs {
        args.push("-onlyin".to_string());
        args.push(dir.replace("~", home_dir));
    }

    Some(args)
}

/// This is the base window, and its a "Tile"
/// Its fields are:
/// - Theme ([`iced::Theme`])
/// - Focus "ID" (which element in the choices is currently selected)
/// - Query (String)
/// - Query Lowercase (String, but lowercase)
/// - Previous Query Lowercase (String)
/// - Results (Vec<[`App`]>) the results of the search
/// - Options ([`AppIndex`]) the options to search through (is a HashMap wrapper)
/// - Emoji Apps ([`AppIndex`]) emojis that are considered as "apps"
/// - Visible (bool) whether the window is visible or not
/// - Focused (bool) whether the window is focused or not
/// - Frontmost ([`Option<Retained<NSRunningApplication>>`]) the frontmost application before the window was opened
/// - Config ([`Config`]) the app's config
/// - Hotkeys, storing the hotkey used for directly opening to the clipboard history page, and
///   opening the app
/// - Sender (The [`ExtSender`] that sends messages, used by the tray icon currently)
/// - Clipboard Content (`Vec<`[`ClipBoardContentType`]`>`) all of the cliboard contents
/// - Page ([`Page`]) the current page of the window (main or clipboard history)
/// - RustCast's height: to figure out which height to resize to
#[derive(Clone)]
pub struct Tile {
    pub theme: iced::Theme,
    pub focus_id: u32,
    pub query: String,
    pub current_mode: String,
    pub update_available: bool,
    pub ranking: HashMap<String, i32>,
    query_lc: String,
    results: Vec<App>,
    options: AppIndex,
    emoji_apps: AppIndex,
    visible: bool,
    focused: bool,
    frontmost: Option<Retained<NSRunningApplication>>,
    pub config: Config,
    hotkeys: Hotkeys,
    clipboard_content: Vec<ClipBoardContentType>,
    tray_icon: Option<TrayIcon>,
    sender: Option<ExtSender>,
    page: Page,
    pub height: f32,
    pub file_search_sender: Option<tokio::sync::watch::Sender<(String, Vec<String>)>>,
    debouncer: Debouncer,
}

/// A struct to store all the hotkeys
///
/// Stores the toggle [`HotKey`] and the Clipboard [`HotKey`]
#[derive(Clone, Debug)]
pub struct Hotkeys {
    pub toggle: Shortcut,
    pub clipboard_hotkey: Shortcut,
    pub shells: HashMap<Shortcut, Shelly>,
}

impl Tile {
    /// This returns the theme of the window
    pub fn theme(&self, _: window::Id) -> Option<Theme> {
        Some(self.theme.clone())
    }

    /// This handles the subscriptions of the window
    ///
    /// The subscriptions are:
    /// - Hotkeys
    /// - Hot reloading
    /// - Clipboard history
    /// - Window close events
    /// - Keypresses (escape to close the window)
    /// - Window focus changes
    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard = event::listen_with(|event, _, id| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => Some(Message::EscKeyPressed(id)),
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(cha),
                modifiers: Modifiers::LOGO,
                ..
            }) => {
                if cha.to_string() == "," {
                    return Some(Message::SwitchToPage(Page::Settings));
                }
                None
            }
            _ => None,
        });
        Subscription::batch([
            Subscription::run(handle_hot_reloading),
            keyboard,
            Subscription::run(crate::platform::macos::urlscheme::url_stream),
            Subscription::run(handle_recipient),
            Subscription::run(handle_version_and_rankings),
            Subscription::run(handle_clipboard_history),
            Subscription::run(handle_file_search),
            window::close_events().map(Message::HideWindow),
            keyboard::listen().filter_map(|event| {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    match key {
                        keyboard::Key::Named(Named::ArrowUp) => {
                            Some(Message::ChangeFocus(ArrowKey::Up, 1))
                        }
                        keyboard::Key::Named(Named::ArrowLeft) => {
                            Some(Message::ChangeFocus(ArrowKey::Left, 1))
                        }
                        keyboard::Key::Named(Named::ArrowRight) => {
                            Some(Message::ChangeFocus(ArrowKey::Right, 1))
                        }
                        keyboard::Key::Named(Named::ArrowDown) => {
                            Some(Message::ChangeFocus(ArrowKey::Down, 1))
                        }
                        keyboard::Key::Character(chr) => {
                            if modifiers.command() && chr.to_string() == "r" {
                                Some(Message::ReloadConfig)
                            } else if chr.to_string() == "p" && modifiers.control() {
                                Some(Message::ChangeFocus(ArrowKey::Up, 1))
                            } else if chr.to_string() == "n" && modifiers.control() {
                                Some(Message::ChangeFocus(ArrowKey::Down, 1))
                            } else {
                                Some(Message::FocusTextInput(Move::Forwards(chr.to_string())))
                            }
                        }
                        keyboard::Key::Named(Named::Enter) => Some(Message::OpenFocused),
                        keyboard::Key::Named(Named::Backspace) => {
                            Some(Message::FocusTextInput(Move::Back))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }),
            window::events()
                .with(self.focused)
                .filter_map(|(focused, (wid, event))| match event {
                    window::Event::Unfocused => {
                        if focused {
                            Some(Message::WindowFocusChanged(wid, false))
                        } else {
                            None
                        }
                    }
                    window::Event::Focused => Some(Message::WindowFocusChanged(wid, true)),
                    _ => None,
                }),
        ])
    }

    /// Handles the search query changed event.
    ///
    /// This is separate from the `update` function because it has a decent amount of logic, and
    /// should be separated out to make it easier to test. This function is called by the `update`
    /// function to handle the search query changed event.
    pub fn handle_search_query_changed(&mut self) {
        let query = self.query_lc.clone();
        let options = if self.page == Page::Main {
            &self.options
        } else if self.page == Page::EmojiSearch {
            &self.emoji_apps
        } else {
            &AppIndex::empty()
        };
        let results: Vec<App> = options
            .search_prefix(&query)
            .map(|x| x.to_owned())
            .collect();

        self.results = results;
    }

    pub fn frequent_results(&self) -> Vec<App> {
        self.options.top_ranked(5)
    }

    /// Gets the frontmost application to focus later.
    pub fn capture_frontmost(&mut self) {
        use objc2_app_kit::NSWorkspace;

        let ws = NSWorkspace::sharedWorkspace();
        self.frontmost = ws.frontmostApplication();
    }

    /// Restores the frontmost application.
    #[allow(deprecated)]
    pub fn restore_frontmost(&mut self) {
        use objc2_app_kit::NSApplicationActivationOptions;

        if let Some(app) = self.frontmost.take() {
            app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
        }
    }
}

/// This is the subscription function that handles the change in clipboard history
fn handle_clipboard_history() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let mut clipboard = Clipboard::new().unwrap();
        let mut prev_byte_rep: Option<ClipBoardContentType> = None;

        loop {
            let byte_rep = if let Ok(a) = clipboard.get_image() {
                Some(ClipBoardContentType::Image(a))
            } else if let Ok(a) = clipboard.get_text()
                && !a.trim().is_empty()
            {
                Some(ClipBoardContentType::Text(a))
            } else {
                None
            };

            if byte_rep != prev_byte_rep
                && let Some(content) = &byte_rep
            {
                info!("Adding item to cbhist");
                output
                    .send(Message::EditClipboardHistory(crate::app::Editable::Create(
                        content.to_owned(),
                    )))
                    .await
                    .ok();
                prev_byte_rep = byte_rep;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
}

/// Read mdfind stdout line-by-line, sending batched results to the UI.
///
/// Returns when stdout reaches EOF, the receiver signals a new query, or
/// max results are reached. Caller is responsible for process lifetime.
async fn read_mdfind_results<R: AsyncRead + Unpin>(
    stdout: R,
    home_dir: &str,
    receiver: &mut tokio::sync::watch::Receiver<(String, Vec<String>)>,
    output: &mut iced::futures::channel::mpsc::Sender<Message>,
) -> bool {
    use crate::app::{FILE_SEARCH_BATCH_SIZE, FILE_SEARCH_MAX_RESULTS};

    let mut reader = tokio::io::BufReader::new(stdout);
    let mut batch: Vec<crate::app::apps::App> = Vec::with_capacity(FILE_SEARCH_BATCH_SIZE as usize);
    let mut total_sent: u32 = 0;

    loop {
        let mut line = String::new();
        let read_result = tokio::select! {
            result = reader.read_line(&mut line) => result,
            _ = receiver.changed() => {
                // New query arrived — caller will handle it.
                return true;
            }
        };

        match read_result {
            Ok(0) => {
                // EOF — flush remaining batch.
                if !batch.is_empty() {
                    output
                        .send(Message::FileSearchResult(std::mem::take(&mut batch)))
                        .await
                        .ok();
                }
                return false;
            }
            Ok(_) => {
                if let Some(app) = crate::commands::path_to_app(line.trim(), home_dir) {
                    batch.push(app);
                    total_sent += 1;
                }
                if batch.len() as u32 >= FILE_SEARCH_BATCH_SIZE {
                    output
                        .send(Message::FileSearchResult(std::mem::take(&mut batch)))
                        .await
                        .ok();
                }
                if total_sent >= FILE_SEARCH_MAX_RESULTS {
                    if !batch.is_empty() {
                        output
                            .send(Message::FileSearchResult(std::mem::take(&mut batch)))
                            .await
                            .ok();
                    }
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
}

fn handle_hot_reloading() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let paths = default_app_paths();
        let mut total_files: usize = paths
            .par_iter()
            .map(|dir| count_dirs_in_dir(std::path::Path::new(dir)))
            .sum();

        loop {
            let current_total_files: usize = paths
                .par_iter()
                .map(|dir| count_dirs_in_dir(std::path::Path::new(dir)))
                .sum();

            if total_files != current_total_files {
                total_files = current_total_files;
                info!("App count was changed");
                let _ = output.send(Message::UpdateApps).await;
            }

            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    })
}

/// Helper fn for counting directories (since macos `.app`'s are directories) inside a directory
fn count_dirs_in_dir(dir: impl AsRef<std::path::Path>) -> usize {
    // Read the directory; if it fails, treat as empty
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .count()
}

/// Async subscription that spawns `mdfind` for file search queries.
///
/// Uses a `watch` channel so the Tile can push new (query, dirs) pairs.
/// Each query change cancels any running `mdfind` and starts a fresh one.
fn handle_file_search() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (sender, mut receiver) =
            tokio::sync::watch::channel((String::new(), Vec::<String>::new()));
        output
            .send(Message::SetFileSearchSender(sender))
            .await
            .expect("Failed to send file search sender.");

        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        assert!(!home_dir.is_empty(), "HOME must not be empty.");

        let mut child: Option<tokio::process::Child> = None;
        let mut wait_for_change = true;

        loop {
            if wait_for_change && receiver.changed().await.is_err() {
                break;
            }

            wait_for_change = true;

            // Kill previous mdfind if still running.
            if let Some(ref mut proc) = child {
                proc.kill().await.ok();
                proc.wait().await.ok();
            }
            child = None;

            let (query, dirs) = receiver.borrow_and_update().clone();

            let Some(args) = build_mdfind_args(&query, &dirs, &home_dir) else {
                output.send(Message::FileSearchClear).await.ok();
                continue;
            };

            let mut command = tokio::process::Command::new("mdfind");
            command.args(&args);
            command.stdout(std::process::Stdio::piped());
            command.stderr(std::process::Stdio::null());

            let mut spawned = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    warn!("Failed to spawn mdfind: {error}");
                    continue;
                }
            };

            let stdout = match spawned.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    warn!("mdfind stdout was not captured");
                    spawned.kill().await.ok();
                    spawned.wait().await.ok();
                    continue;
                }
            };

            child = Some(spawned);

            let canceled = read_mdfind_results(stdout, &home_dir, &mut receiver, &mut output).await;

            if let Some(ref mut proc) = child {
                if canceled {
                    proc.kill().await.ok();
                }
                proc.wait().await.ok();
            }
            child = None;

            // `read_mdfind_results` consumed the watch notification when canceled,
            // so process the latest query immediately.
            if canceled {
                wait_for_change = false;
            }
        }

        if let Some(ref mut proc) = child {
            proc.kill().await.ok();
            proc.wait().await.ok();
        }
    })
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::app::apps::{App, AppCommand};
    use crate::commands::Function;
    use iced::futures::StreamExt;
    use tokio::io::{AsyncWriteExt, duplex};

    fn test_app(name: &str, ranking: i32) -> App {
        App {
            ranking,
            open_command: AppCommand::Function(Function::OpenApp(format!(
                "/Applications/{name}.app"
            ))),
            desc: "Application".to_string(),
            icons: None,
            display_name: name.to_string(),
            search_name: name.to_lowercase(),
        }
    }

    #[test]
    fn app_index_search_prefix_matches_prefix_and_word_boundaries() {
        let index = AppIndex::from_apps(vec![
            test_app("Safari", 0),
            App {
                search_name: "visual studio code".to_string(),
                display_name: "Visual Studio Code".to_string(),
                ..test_app("Visual Studio Code", 0)
            },
            App {
                search_name: "signal-desktop".to_string(),
                display_name: "Signal Desktop".to_string(),
                ..test_app("Signal Desktop", 0)
            },
        ]);

        let prefix_results: Vec<_> = index
            .search_prefix("sa")
            .map(|app| app.display_name.clone())
            .collect();
        let spaced_results: Vec<_> = index
            .search_prefix("studio")
            .map(|app| app.display_name.clone())
            .collect();
        let hyphen_results: Vec<_> = index
            .search_prefix("desktop")
            .map(|app| app.display_name.clone())
            .collect();

        assert_eq!(prefix_results, vec!["Safari".to_string()]);
        assert_eq!(spaced_results, vec!["Visual Studio Code".to_string()]);
        assert_eq!(hyphen_results, vec!["Signal Desktop".to_string()]);
    }

    #[test]
    fn app_index_ranking_helpers_work() {
        let mut index = AppIndex::from_apps(vec![
            test_app("Safari", 1),
            test_app("Notes", -1),
            test_app("Arc", 3),
            test_app("Alfred", 3),
        ]);

        index.update_ranking("safari");
        index.set_ranking("notes", -1);

        assert_eq!(index.get_rankings().get("safari"), Some(&2));
        assert_eq!(index.get_rankings().get("notes"), None);

        let top_ranked = index.top_ranked(2);
        assert_eq!(top_ranked.len(), 2);
        assert_eq!(top_ranked[0].display_name, "Alfred");
        assert_eq!(top_ranked[1].display_name, "Arc");

        let favourites = index.get_favourites();
        assert_eq!(favourites.len(), 1);
        assert_eq!(favourites[0].display_name, "Notes");
    }

    #[test]
    fn build_mdfind_args_expands_home_and_omits_onlyin_for_empty_dirs() {
        assert_eq!(
            build_mdfind_args("ab", &[], "/Users/test").unwrap(),
            vec!["-name".to_string(), "ab".to_string()]
        );

        assert_eq!(
            build_mdfind_args(
                "report",
                &[String::from("~/Documents"), String::from("/tmp")],
                "/Users/test"
            )
            .unwrap(),
            vec![
                "-name".to_string(),
                "report".to_string(),
                "-onlyin".to_string(),
                "/Users/test/Documents".to_string(),
                "-onlyin".to_string(),
                "/tmp".to_string(),
            ]
        );
    }

    #[test]
    fn build_mdfind_args_rejects_short_queries() {
        assert!(build_mdfind_args("a", &[], "/Users/test").is_none());
    }

    #[tokio::test]
    async fn read_mdfind_results_batches_and_limits_results() {
        let total_lines = crate::app::FILE_SEARCH_BATCH_SIZE + 1;
        let input = (0..=total_lines)
            .map(|idx| format!("/Users/test/Documents/file-{idx}.txt\n"))
            .collect::<String>();

        let (mut writer, reader) = duplex(16 * 1024);
        writer.write_all(input.as_bytes()).await.unwrap();
        drop(writer);

        let (_notify_sender, mut receiver) =
            tokio::sync::watch::channel((String::new(), Vec::<String>::new()));
        receiver.borrow_and_update();
        let (mut sender, mut out_receiver) = iced::futures::channel::mpsc::channel(16);

        let canceled = read_mdfind_results(reader, "/Users/test", &mut receiver, &mut sender).await;
        assert!(!canceled);

        drop(sender);

        let mut batches = Vec::new();
        while let Some(message) = out_receiver.next().await {
            if let Message::FileSearchResult(apps) = message {
                batches.push(apps);
            }
        }

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len() as u32, crate::app::FILE_SEARCH_BATCH_SIZE);
        assert_eq!(batches[1].len(), 2);
        assert_eq!(batches[0][0].desc, "~/Documents/file-0.txt");
    }

    #[tokio::test]
    async fn read_mdfind_results_cancels_when_query_changes() {
        let (notify_sender, mut receiver) =
            tokio::sync::watch::channel((String::new(), Vec::<String>::new()));
        receiver.borrow_and_update();

        let (mut out_sender, _out_receiver) = iced::futures::channel::mpsc::channel(4);
        let (writer, reader) = duplex(1024);

        let task = tokio::spawn(async move {
            read_mdfind_results(reader, "/Users/test", &mut receiver, &mut out_sender).await
        });

        // Keep the writer alive so the read loop waits on the watch channel.
        let _writer = writer;

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        notify_sender
            .send((String::from("next"), Vec::<String>::new()))
            .unwrap();

        assert!(task.await.unwrap());
    }
}

/// Handles the rx / receiver for sending and receiving messages
fn handle_recipient() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (sender, mut recipient) = channel(100);
        output
            .send(Message::SetSender(ExtSender(sender)))
            .await
            .expect("Sender not sent");
        loop {
            let abcd = recipient
                .try_recv()
                .map(async |msg| {
                    output.send(msg).await.unwrap();
                })
                .ok();

            if let Some(abcd) = abcd {
                abcd.await;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
}

fn handle_version_and_rankings() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let current_version = format!("\"{}\"", option_env!("APP_VERSION").unwrap_or(""));

        if current_version.is_empty() {
            println!("empty version");
            return;
        }

        let req = minreq::Request::new(
            minreq::Method::Get,
            "https://api.github.com/repos/RustCastLabs/rustcast/releases/latest",
        )
        .with_header("User-Agent", "rustcast-update-checker")
        .with_header("Accept", "application/vnd.github+json")
        .with_header("X-GitHub-Api-Version", "2022-11-28");

        loop {
            let resp = req
                .clone()
                .send()
                .and_then(|x| x.as_str().map(serde_json::Value::from_str));

            info!("Made a req for latest version");

            if let Ok(Ok(val)) = resp {
                let new_ver = val
                    .get("name")
                    .map(|x| x.to_string())
                    .unwrap_or("".to_string());

                // new_ver is in the format "\"v0.0.0\""
                // note that it is encapsulated in double quotes
                if new_ver.trim() != current_version
                    && !new_ver.is_empty()
                    && new_ver.starts_with("\"v")
                {
                    info!("new version available: {new_ver}");
                    output.send(Message::UpdateAvailable).await.ok();
                }
            } else {
                warn!("Error getting resp");
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
            output.send(Message::SaveRanking).await.ok();
            info!("Sent save ranking");
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    })
}
