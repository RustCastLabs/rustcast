use std::io::Cursor;

use iced::widget::image::Handle;
use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};
use objc2_foundation::NSString;

use crate::{
    app::apps::{App, AppCommand},
    commands::Function,
    platform::macos::discovery::icon_of_path_ns,
};

pub fn get_open_apps(store_icons: bool) -> Vec<App> {
    let open_apps = NSWorkspace::sharedWorkspace().runningApplications();

    open_apps
        .iter()
        .filter_map(|app| {
            if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
                return None;
            }

            let name = app.localizedName().unwrap().to_string();

            let icon = icon_of_path_ns(
                &app.bundleURL()
                    .and_then(|x| x.path())
                    .unwrap_or(NSString::new())
                    .to_string(),
            )
            .unwrap_or(vec![]);
            let icons = if store_icons {
                image::ImageReader::new(Cursor::new(icon))
                    .with_guessed_format()
                    .unwrap()
                    .decode()
                    .ok()
                    .map(|img| Handle::from_rgba(img.width(), img.height(), img.into_bytes()))
            } else {
                None
            };

            Some(App {
                ranking: 0,
                open_command: AppCommand::Function(Function::QuitApp(name.clone())),
                display_name: format!("Quit {}", name),
                icons,
                search_name: format!("quit {}", name.to_lowercase()),
                desc: name.to_string(),
            })
        })
        .collect()
}

pub fn terminate_app(name: String) {
    let open_apps = NSWorkspace::sharedWorkspace().runningApplications();

    for app in open_apps {
        let is_regular_app = app.activationPolicy() == NSApplicationActivationPolicy::Regular;
        let name_matches = app.localizedName() == Some(NSString::from_str(&name));

        if is_regular_app && name_matches {
            app.terminate();
            break;
        }
    }
}
