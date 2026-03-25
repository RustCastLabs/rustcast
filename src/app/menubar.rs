//! This has the menubar icon logic for the app

use std::{collections::HashMap, io::Cursor};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use image::{DynamicImage, ImageReader};
use log::info;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{
        AboutMetadataBuilder, Icon as Ico, IsMenuItem, Menu, MenuEvent, MenuItem,
        PredefinedMenuItem, Submenu, accelerator::Accelerator,
    },
};

use crate::{
    app::{Message, tile::ExtSender},
    config::Config,
    utils::open_url,
};

const DISCORD_LINK: &str = "https://discord.gg/bDfNYPbnC5";

use tokio::runtime::Runtime;

/// This create a new menubar icon for the app
pub fn menu_icon(config: Config, sender: ExtSender) -> TrayIcon {
    let builder = TrayIconBuilder::new();
    let menu = menu_builder(config, sender, false);

    let image = get_image();
    let icon = Icon::from_rgba(image.as_bytes().to_vec(), image.width(), image.height()).unwrap();

    builder
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .unwrap()
}

pub fn menu_builder(config: Config, sender: ExtSender, update_item: bool) -> Menu {
    let hotkey = config.toggle_hotkey.parse::<HotKey>().ok();

    let mut modes = config.modes;
    if !modes.contains_key("default") {
        modes.insert("Default".to_string(), "default".to_string());
    }

    init_event_handler(sender, hotkey.map(|x| x.id));

    Menu::with_items(&[
        &MenuItem::with_id(
            "update",
            if update_item {
                "Update available"
            } else {
                "Up to date"
            },
            update_item,
            None,
        ),
        &version_item(),
        &about_item(get_image()),
        &open_github_item(),
        &PredefinedMenuItem::separator(),
        &refresh_item(),
        &open_item(hotkey),
        &mode_item(modes),
        &PredefinedMenuItem::separator(),
        &open_issue_item(),
        &get_help_item(),
        &PredefinedMenuItem::separator(),
        &open_settings_item(),
        &discord_item(),
        &hide_tray_icon(),
        &quit_item(),
    ])
    .unwrap()
}

fn get_image() -> DynamicImage {
    ImageReader::new(Cursor::new(menubar_icon().unwrap_or_default()))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap()
}

fn init_event_handler(sender: ExtSender, hotkey_id: Option<u32>) {
    let runtime = Runtime::new().unwrap();

    MenuEvent::set_event_handler(Some(move |x: MenuEvent| {
        let sender = sender.clone();
        let sender = sender.0.clone();
        info!("Menubar event called: {}", x.id.0);
        match x.id().0.as_str() {
            "refresh_rustcast" => {
                runtime.spawn(async move {
                    sender.clone().try_send(Message::ReloadConfig).unwrap();
                });
            }
            "hide_tray_icon" => {
                runtime
                    .spawn(async move { sender.clone().try_send(Message::HideTrayIcon).unwrap() });
            }
            "open_issue_page" => {
                open_url("https://github.com/unsecretised/rustcast/issues/new");
            }
            "show_rustcast" => {
                if let Some(hk) = hotkey_id {
                    runtime.spawn(async move {
                        sender.clone().try_send(Message::KeyPressed(hk)).unwrap();
                    });
                }
            }
            "update" => {
                open_url("https://github.com/unsecretised/rustcast/releases/latest");
            }
            "open_discord" => {
                open_url(DISCORD_LINK);
            }
            "open_help_page" => {
                open_url("https://github.com/unsecretised/rustcast/discussions/new?category=q-a");
            }
            "open_preferences" => {
                runtime.spawn(async move {
                    sender.clone().try_send(Message::OpenToSettings).unwrap();
                });
            }
            "open_github_page" => {
                open_url("https://github.com/unsecretised/rustcast");
            }
            id => {
                if id.starts_with("mode_switch_") {
                    let id = id.to_string();
                    runtime.spawn(async move {
                        sender
                            .clone()
                            .try_send(Message::SwitchMode(
                                id.strip_prefix("mode_switch_").unwrap_or("").to_string(),
                            ))
                            .unwrap();
                    });
                }
            }
        }
    }));
}

fn version_item() -> MenuItem {
    let version = "RustCast: ".to_string() + option_env!("APP_VERSION").unwrap_or("Unknown");
    MenuItem::new(version, false, None)
}

fn discord_item() -> MenuItem {
    MenuItem::with_id("open_discord", "RustCast discord", true, None)
}

fn hide_tray_icon() -> MenuItem {
    MenuItem::with_id("hide_tray_icon", "Hide Tray Icon", true, None)
}

fn mode_item(modes: HashMap<String, String>) -> Submenu {
    let owned_items: Vec<MenuItem> = modes
        .keys()
        .map(|key| {
            MenuItem::with_id(
                format!("mode_switch_{}", key), // id uses the key
                format!("{}{}", key.split_at(1).0.to_uppercase(), key.split_at(1).1),
                true,
                None,
            )
        })
        .collect();

    let items: Vec<&dyn IsMenuItem> = owned_items.iter().map(|x| x as &dyn IsMenuItem).collect();

    Submenu::with_items("Modes", true, &items).unwrap()
}

fn open_item(hotkey: Option<HotKey>) -> MenuItem {
    MenuItem::with_id(
        "show_rustcast",
        "Toggle View",
        true,
        hotkey.map(|hk| Accelerator::new(Some(hk.mods), hk.key)),
    )
}

fn open_github_item() -> MenuItem {
    MenuItem::with_id("open_github_page", "Star on Github", true, None)
}

fn open_issue_item() -> MenuItem {
    MenuItem::with_id("open_issue_page", "Report an Issue", true, None)
}

fn refresh_item() -> MenuItem {
    MenuItem::with_id(
        "refresh_rustcast",
        "Refresh",
        true,
        Some(Accelerator::new(
            Some(Modifiers::SUPER),
            global_hotkey::hotkey::Code::KeyR,
        )),
    )
}

fn open_settings_item() -> MenuItem {
    MenuItem::with_id(
        "open_preferences",
        "Open Preferences",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
    )
}

fn get_help_item() -> MenuItem {
    MenuItem::with_id("open_help_page", "Help", true, None)
}

fn quit_item() -> PredefinedMenuItem {
    PredefinedMenuItem::quit(Some("Quit"))
}

fn about_item(image: DynamicImage) -> PredefinedMenuItem {
    let about_metadata_builder = AboutMetadataBuilder::new()
        .name(Some("RustCast"))
        .version(Some(
            option_env!("APP_VERSION").unwrap_or("Unknown Version"),
        ))
        .authors(Some(vec!["Unsecretised".to_string()]))
        .credits(Some("Unsecretised".to_string()))
        .icon(Ico::from_rgba(image.as_bytes().to_vec(), image.width(), image.height()).ok())
        .website(Some("https://rustcast.app"))
        .license(Some("MIT"))
        .build();

    PredefinedMenuItem::about(Some("About.."), Some(about_metadata_builder))
}

#[cfg(target_os = "macos")]
fn menubar_icon() -> Option<Vec<u8>> {
    objc2::rc::autoreleasepool(|_| -> Option<Vec<u8>> {
        use objc2::rc::Retained;
        use objc2_app_kit::NSBitmapImageRep;
        use objc2_app_kit::{NSBezierPath, NSBitmapImageFileType, NSColor, NSImage};
        use objc2_foundation::NSSize;
        use objc2_foundation::{NSData, NSDictionary, NSPoint};

        let size = 128.;
        let thickness = 4.;

        let center = NSPoint::new(size * 0.5, size * 0.5);

        let s = NSSize::new(size, size);
        let segments = [
            (-70., 145., size * 0.33, (size * 0.33 - thickness).max(0.0)),
            (0., 360., size * 0.2, (size * 0.2 - thickness).max(0.0)),
        ];
        let image: Retained<NSImage> = NSImage::imageWithSize_flipped_drawingHandler(
            s,
            false,
            &block2::RcBlock::new(move |doc_rect| {
                let doc =
                    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(doc_rect, 10., 10.);
                NSColor::colorWithCalibratedRed_green_blue_alpha(0.1, 0.1, 0.1, 0.).setFill();
                doc.fill();
                let path = NSBezierPath::bezierPath();

                for (start, end, outer_r, inner_r) in segments {
                    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
                        center, outer_r, start, end, false,
                    );

                    path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
                        center, inner_r, end, start, true,
                    );
                }

                NSColor::colorWithCalibratedRed_green_blue_alpha(1., 1., 1., 0.8).setFill();
                path.fill();

                path.closePath();
                true.into()
            }),
        );

        let tiff = image.TIFFRepresentation()?;
        let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
        let png: Retained<NSData> = unsafe {
            rep.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )?
        };
        Some(png.to_vec())
    })
}

#[cfg(not(target_os = "macos"))]
fn menubar_icon() -> Option<Vec<u8>> {
    Some(include_bytes!("../../docs/icon.png").to_vec())
}
