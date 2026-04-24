#![deny(clippy::dbg_macro)]

mod app;
mod autoupdate;
mod calculator;
mod clipboard;
mod commands;
mod config;
mod debounce;
mod platform;
mod quit;
mod styles;
mod unit_conversion;
mod utils;

use std::{collections::HashMap, fs::OpenOptions, path::Path};

use crate::{
    app::tile::{self, Hotkeys, Tile},
    config::Config,
    platform::macos::{get_autostart_status, launching::Shortcut},
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

    let mut config: Config = match std::fs::read_to_string(&file_path) {
        Ok(a) => toml::from_str(&a).unwrap_or(Config::default()),
        Err(_) => Config::default(),
    };

    config.start_at_login = get_autostart_status();

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

    let show_hide =
        Shortcut::parse(&config.toggle_hotkey).unwrap_or(Shortcut::parse("option+space").unwrap());

    let cbhist = Shortcut::parse(&config.clipboard_hotkey.to_lowercase())
        .unwrap_or_else(|_| Shortcut::parse("cmd+shift+c").unwrap());

    let mut shell_map = HashMap::new();

    for shell in &config.shells {
        if let Some(hk_str) = &shell.hotkey
            && let Ok(hk) = Shortcut::parse(hk_str)
        {
            shell_map.insert(hk, shell.clone());
        }
    }

    let hotkeys = Hotkeys {
        toggle: show_hide,
        clipboard_hotkey: cbhist,
        shells: shell_map,
    };

    info!("Hotkeys loaded");
    info!("Starting rustcast");

    iced::daemon(
        move || tile::elm::new(hotkeys.clone(), &config),
        tile::update::handle_update,
        tile::elm::view,
    )
    .subscription(Tile::subscription)
    .theme(Tile::theme)
    .run()
}
