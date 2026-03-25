//! This module handles the logic for the tile, AKA rustcast's main window
pub mod elm;
pub mod update;

use crate::app::apps::App;
use crate::app::{
    ArrowKey, Message, Move, Page, FILE_SEARCH_BATCH_SIZE, FILE_SEARCH_ICON_BATCH_SIZE,
    FILE_SEARCH_MAX_ICONS, FILE_SEARCH_MAX_RESULTS,
};
use crate::clipboard::ClipBoardContentType;
use crate::config::Config;
use crate::debounce::Debouncer;
use crate::platform::{default_app_paths, icon_of_path_ns};

use arboard::Clipboard;
use block2::RcBlock;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

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
use objc2_foundation::{
    NSArray, NSDate, NSDefaultRunLoopMode, NSMetadataItemPathKey, NSMetadataQuery,
    NSMetadataQueryDidFinishGatheringNotification, NSNotificationCenter, NSPredicate, NSRunLoop,
    NSString,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::slice::ParallelSliceMut;
use tray_icon::TrayIcon;

use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
            if name.starts_with(prefix) || name.contains(format!(" {prefix}").as_str()) {
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
    pub toggle: HotKey,
    pub clipboard_hotkey: HotKey,
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
            Subscription::run(handle_hotkeys),
            Subscription::run(handle_hot_reloading),
            keyboard,
            Subscription::run(handle_recipient),
            Subscription::run(handle_version_and_rankings),
            Subscription::run(handle_clipboard_history),
            Subscription::run(handle_file_search),
            window::close_events().map(Message::HideWindow),
            keyboard::listen().filter_map(|event| {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    match key {
                        keyboard::Key::Named(Named::ArrowUp) => {
                            return Some(Message::ChangeFocus(ArrowKey::Up, 1));
                        }
                        keyboard::Key::Named(Named::ArrowLeft) => {
                            return Some(Message::ChangeFocus(ArrowKey::Left, 1));
                        }
                        keyboard::Key::Named(Named::ArrowRight) => {
                            return Some(Message::ChangeFocus(ArrowKey::Right, 1));
                        }
                        keyboard::Key::Named(Named::ArrowDown) => {
                            return Some(Message::ChangeFocus(ArrowKey::Down, 1));
                        }
                        keyboard::Key::Character(chr) => {
                            if modifiers.command() && chr.to_string() == "r" {
                                return Some(Message::ReloadConfig);
                            } else if chr.to_string() == "p" && modifiers.control() {
                                return Some(Message::ChangeFocus(ArrowKey::Up, 1));
                            } else if chr.to_string() == "n" && modifiers.control() {
                                return Some(Message::ChangeFocus(ArrowKey::Down, 1));
                            } else {
                                return Some(Message::FocusTextInput(Move::Forwards(
                                    chr.to_string(),
                                )));
                            }
                        }
                        keyboard::Key::Named(Named::Enter) => return Some(Message::OpenFocused),
                        keyboard::Key::Named(Named::Backspace) => {
                            return Some(Message::FocusTextInput(Move::Back));
                        }
                        _ => {}
                    }
                    None
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

/// This is the subscription function that handles hotkeys, e.g. for hiding / showing the window
fn handle_hotkeys() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            info!("Hotkey received");
            if let Ok(event) = receiver.recv()
                && event.state == HotKeyState::Pressed
            {
                output.try_send(Message::KeyPressed(event.id)).unwrap();
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
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

/// This represents the messages sent from the NSMetadataQuery thread back to the iced subscription
enum QueryThreadMsg {
    Batch(Vec<crate::app::apps::App>),
    Clear,
    Icons(Vec<(usize, iced::widget::image::Handle)>),
}

/// This is the subscription function that bridges the NSMetadataQuery thread to the UI
///
/// Creates a watch channel for query input and a tokio mpsc channel for results.
/// Spawns a dedicated thread for NSMetadataQuery (which needs an NSRunLoop).
fn handle_file_search() -> impl futures::Stream<Item = Message> {
    stream::channel(100, async |mut output| {
        let (watch_tx, watch_rx) =
            tokio::sync::watch::channel((String::new(), Vec::<String>::new()));
        output
            .send(Message::SetFileSearchSender(watch_tx))
            .await
            .expect("Failed to send file search sender.");

        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<QueryThreadMsg>(64);

        std::thread::Builder::new()
            .name("nsmetadata-query".into())
            .spawn(move || metadata_query_thread(watch_rx, msg_tx))
            .expect("Failed to spawn metadata query thread.");

        while let Some(msg) = msg_rx.recv().await {
            match msg {
                QueryThreadMsg::Batch(apps) => {
                    output.send(Message::FileSearchResult(apps)).await.ok();
                }
                QueryThreadMsg::Clear => {
                    output.send(Message::FileSearchClear).await.ok();
                }
                QueryThreadMsg::Icons(icons) => {
                    output.send(Message::FileSearchIcons(icons)).await.ok();
                }
            }
        }
    })
}

/// This configures the NSMetadataQuery predicate and search scopes
///
/// Uses NSPredicate LIKE[cd] for case+diacritic insensitive glob matching.
/// Whitespace-separated tokens are joined with `*` wildcards so "img jpg"
/// matches filenames containing "img" followed by "jpg".
///
/// NSPredicate LIKE uses `*` (multi-char) and `?` (single-char) as wildcards.
/// The underscore `_` is NOT special in LIKE (unlike SQL), so filenames like
/// `IMG_1234.jpg` match correctly.
fn configure_metadata_query(
    query: &objc2_foundation::NSMetadataQuery,
    search_text: &str,
    dirs: &[String],
    home_dir: &str,
) {
    assert!(!search_text.is_empty(), "Search text must not be empty.");
    assert!(!home_dir.is_empty(), "Home dir must not be empty.");

    // Escape LIKE wildcards in each token, then join with `*`.
    let tokens: Vec<String> = search_text
        .split_whitespace()
        .map(|t| t.replace('*', "\\*").replace('?', "\\?"))
        .collect();
    assert!(!tokens.is_empty(), "Tokens must not be empty.");
    let pattern = format!("*{}*", tokens.join("*"));

    // Use predicateWithFormat with %@ so the pattern is properly quoted.
    let format_str = NSString::from_str("kMDItemDisplayName LIKE[cd] %@");
    let pattern_ns = NSString::from_str(&pattern);
    let pattern_obj: objc2::rc::Retained<objc2::runtime::AnyObject> =
        objc2::rc::Retained::into_super(objc2::rc::Retained::into_super(pattern_ns));
    let args = NSArray::from_retained_slice(&[pattern_obj]);
    // SAFETY: predicateWithFormat_argumentArray is an NSPredicate class method that
    // parses the format string and substitutes %@ with the argument array values.
    // The format string is a compile-time constant and args contains a single valid
    // NSString, so the call is well-formed.
    let predicate = unsafe {
        NSPredicate::predicateWithFormat_argumentArray(&format_str, Some(&args))
    };
    query.setPredicate(Some(&predicate));

    let scope_strings: Vec<objc2::rc::Retained<NSString>> = dirs
        .iter()
        .map(|d| NSString::from_str(&d.replace("~", home_dir)))
        .collect();
    let scope_objects: Vec<objc2::rc::Retained<objc2::runtime::AnyObject>> = scope_strings
        .into_iter()
        .map(|s| objc2::rc::Retained::into_super(objc2::rc::Retained::into_super(s)))
        .collect();
    let scopes = NSArray::from_retained_slice(&scope_objects);
    // SAFETY: setSearchScopes expects an NSArray of scope objects (NSString paths
    // or NSURL). We pass an array of NSString path values which is a valid scope type.
    unsafe { query.setSearchScopes(&scopes) };
}

/// This extracts paths from an NSMetadataQuery and sends them as batched App results
///
/// Called on the query thread after the gather-complete notification fires.
/// Disables updates during iteration to prevent mutation.
/// Returns the absolute paths of accepted results for subsequent icon loading.
fn drain_metadata_results(
    query: &objc2_foundation::NSMetadataQuery,
    home_dir: &str,
    msg_tx: &tokio::sync::mpsc::Sender<QueryThreadMsg>,
) -> Vec<String> {
    assert!(!home_dir.is_empty(), "Home dir must not be empty.");
    assert!(!msg_tx.is_closed(), "Message channel must be open.");

    query.disableUpdates();
    let count = query.resultCount();
    let limit = count.min(FILE_SEARCH_MAX_RESULTS as usize);
    let attr_key = unsafe { NSMetadataItemPathKey };

    let mut batch: Vec<crate::app::apps::App> =
        Vec::with_capacity(FILE_SEARCH_BATCH_SIZE as usize);
    let mut paths: Vec<String> = Vec::with_capacity(limit);
    let mut idx: usize = 0;

    while idx < limit {
        let path_str = query
            .resultAtIndex(idx)
            .downcast::<objc2_foundation::NSMetadataItem>()
            .ok()
            .and_then(|item| item.valueForAttribute(attr_key))
            .and_then(|val| val.downcast::<NSString>().ok())
            .map(|ns| ns.to_string());

        if let Some(path_str) = path_str {
            if let Some(app) = crate::commands::path_to_app(&path_str, home_dir) {
                batch.push(app);
                paths.push(path_str);
            }
        }
        idx += 1;

        if batch.len() as u32 >= FILE_SEARCH_BATCH_SIZE {
            if let Err(e) = msg_tx.try_send(QueryThreadMsg::Batch(std::mem::take(&mut batch))) {
                warn!("Failed to send file search batch: {e}");
            }
        }
    }

    if !batch.is_empty() {
        if let Err(e) = msg_tx.try_send(QueryThreadMsg::Batch(batch)) {
            warn!("Failed to send final file search batch: {e}");
        }
    }
    query.enableUpdates();
    paths
}

/// This loads file icons for search results and sends them to the UI
///
/// Loads icons via NSWorkspace::iconForFile on the query thread (which has
/// Cocoa runtime). Only loads the first `max_icons` results. Checks for
/// new queries between batches to allow cancellation.
fn load_file_search_icons(
    paths: &[String],
    msg_tx: &tokio::sync::mpsc::Sender<QueryThreadMsg>,
    watch_rx: &tokio::sync::watch::Receiver<(String, Vec<String>)>,
) {
    let limit = paths.len().min(FILE_SEARCH_MAX_ICONS);

    assert!(FILE_SEARCH_ICON_BATCH_SIZE > 0, "Batch size must be positive.");

    let mut icon_batch: Vec<(usize, iced::widget::image::Handle)> =
        Vec::with_capacity(FILE_SEARCH_ICON_BATCH_SIZE);

    let mut idx: usize = 0;
    while idx < limit {
        // Cancel if a new query has arrived.
        if watch_rx.has_changed().unwrap_or(false) {
            return;
        }

        if let Some(png_data) = icon_of_path_ns(&paths[idx]) {
            let handle = image::ImageReader::new(std::io::Cursor::new(png_data))
                .with_guessed_format()
                .ok()
                .and_then(|r| r.decode().ok())
                .map(|img| {
                    let rgba = img.to_rgba8();
                    iced::widget::image::Handle::from_rgba(
                        rgba.width(),
                        rgba.height(),
                        rgba.into_raw(),
                    )
                });
            if let Some(h) = handle {
                icon_batch.push((idx, h));
            }
        }
        idx += 1;

        if icon_batch.len() >= FILE_SEARCH_ICON_BATCH_SIZE || idx >= limit {
            if !icon_batch.is_empty() {
                if let Err(e) = msg_tx.try_send(QueryThreadMsg::Icons(std::mem::take(&mut icon_batch))) {
                    warn!("Failed to send icon batch: {e}");
                }
            }
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

/// This creates an NSMetadataQuery and registers a gather-complete notification observer
///
/// Returns the query, the results-ready flag, and the observer handle.
/// The observer sets the AtomicBool flag when NSMetadataQueryDidFinishGathering fires.
fn metadata_query_thread_setup() -> (
    objc2::rc::Retained<objc2_foundation::NSMetadataQuery>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>,
) {
    let query = NSMetadataQuery::new();
    query.setNotificationBatchingInterval(0.0);

    let results_ready = Arc::new(AtomicBool::new(false));
    let flag = results_ready.clone();
    let block = RcBlock::new(move |_: core::ptr::NonNull<objc2_foundation::NSNotification>| {
        flag.store(true, Ordering::Release);
    });

    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: addObserverForName registers a notification block with the default center.
    // The block and notification name are valid for the lifetime of the returned observer.
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSMetadataQueryDidFinishGatheringNotification),
            None,
            None,
            &block,
        )
    };

    assert!(!results_ready.load(Ordering::Acquire), "Flag must start false.");
    assert!(query.resultCount() == 0, "Query must start empty.");

    (query, results_ready, observer)
}

/// This is the dedicated thread that runs NSMetadataQuery with an NSRunLoop
///
/// Polls the watch channel for new queries every 50ms run-loop tick.
/// When the gather-complete notification fires (via AtomicBool flag),
/// drains results and sends them back through the mpsc channel.
fn metadata_query_thread(
    mut watch_rx: tokio::sync::watch::Receiver<(String, Vec<String>)>,
    msg_tx: tokio::sync::mpsc::Sender<QueryThreadMsg>,
) {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    assert!(!home_dir.is_empty(), "HOME must not be empty.");
    assert!(!msg_tx.is_closed(), "Message channel must be open.");

    let (query, results_ready, observer) = metadata_query_thread_setup();

    let run_loop = NSRunLoop::currentRunLoop();
    let run_loop_mode = unsafe { NSDefaultRunLoopMode };
    let tick_seconds: f64 = 0.05;

    loop {
        // Tick the run loop to process notifications.
        let timeout = NSDate::dateWithTimeIntervalSinceNow(tick_seconds);
        run_loop.runMode_beforeDate(run_loop_mode, &timeout);

        // Drain results only when the finish-gathering notification has fired.
        if results_ready.swap(false, Ordering::AcqRel) {
            if query.resultCount() > 0 {
                let paths = drain_metadata_results(&query, &home_dir, &msg_tx);
                load_file_search_icons(&paths, &msg_tx, &watch_rx);
            }
        }

        // Check for new query from the UI.
        if watch_rx.has_changed().unwrap_or(false) {
            let (ref q, ref dirs) = *watch_rx.borrow_and_update();

            // Clear the flag before stopping so a final gather-complete
            // notification from the old query does not leak through.
            results_ready.store(false, Ordering::Release);
            query.stopQuery();

            // Tell the UI to discard previous results before new ones arrive.
            if let Err(e) = msg_tx.try_send(QueryThreadMsg::Clear) {
                warn!("Failed to send file search clear: {e}");
            }

            if q.len() < 2 {
                continue;
            }
            assert!(q.len() < 1024, "Query too long.");

            configure_metadata_query(&query, q, dirs, &home_dir);
            let started = query.startQuery();
            if !started {
                warn!("NSMetadataQuery failed to start.");
            }
        }

        if msg_tx.is_closed() {
            break;
        }
    }

    query.stopQuery();
    let center = NSNotificationCenter::defaultCenter();
    let observer_ref: &objc2::runtime::AnyObject =
        objc2::runtime::ProtocolObject::as_ref(&*observer);
    unsafe { center.removeObserver(observer_ref) };
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
                    info!("Sending a message");
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
            "https://api.github.com/repos/unsecretised/rustcast/releases/latest",
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
