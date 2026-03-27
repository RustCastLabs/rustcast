#![deny(clippy::dbg_macro)]

mod app;
mod calculator;
mod clipboard;
mod commands;
mod config;
mod database;
mod debounce;
mod platform;
mod quit;
mod styles;
mod unit_conversion;
mod utils;

use std::{fs::OpenOptions, path::Path};

use crate::{
    app::tile::{self, Tile},
    config::Config,
};

use global_hotkey::{
    GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use log::info;
use tracing_subscriber::{EnvFilter, Layer, util::SubscriberInitExt};

use self::platform::set_activation_policy_accessory;

fn main() -> iced::Result {
    set_activation_policy_accessory();

    let home = std::env::var("HOME").unwrap();

    let file_path = home.clone() + "/.config/rustcast/config.toml";
    if !Path::new(&file_path).exists() {
        std::fs::create_dir_all(home.clone() + "/.config/rustcast").unwrap();
        std::fs::write(
            &file_path,
            toml::to_string(&Config::default()).unwrap_or_else(|x| x.to_string()),
        )
        .unwrap();
    }

    let config: Config = match std::fs::read_to_string(&file_path) {
        Ok(a) => toml::from_str(&a).unwrap_or(Config::default()),
        Err(_) => Config::default(),
    };

    if cfg!(debug_assertions) {
        let sub = tracing_subscriber::fmt().finish();
        EnvFilter::new("rustcast=info").with_subscriber(sub).init();
    } else {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(config.log_path.replace("~", &home))
            .unwrap();

        let sub = tracing_subscriber::fmt().with_writer(file).finish();
        EnvFilter::new("rustcast=info").with_subscriber(sub).init();
    };

    info!("Config loaded");

    let manager = GlobalHotKeyManager::new().unwrap();

    let show_hide = config
        .toggle_hotkey
        .parse()
        .unwrap_or(HotKey::new(Some(Modifiers::ALT), Code::Space));

    let cbhist = config
        .clipboard_hotkey
        .parse()
        .unwrap_or("SUPER+SHIFT+C".parse().unwrap());

    let mut hotkeys = vec![show_hide, cbhist];
    for shell in &config.shells {
        if let Some(hk_str) = &shell.hotkey
            && let Ok(hk) = hk_str.parse::<HotKey>()
        {
            hotkeys.push(hk);
        }
    }

    manager
        .register_all(&hotkeys)
        .expect("Unable to register hotkeys");

    info!("Hotkeys loaded");
    info!("Starting rustcast");

    iced::daemon(
        move || tile::elm::new(show_hide, &config),
        tile::update::handle_update,
        tile::elm::view,
    )
    .subscription(Tile::subscription)
    .theme(Tile::theme)
    .run()
}
