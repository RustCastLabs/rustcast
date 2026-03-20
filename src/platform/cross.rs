use std::{
    fs,
    path::{Path, PathBuf},
    process::exit,
};

use log::{error, info};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator as _};

use crate::{
    app::apps::{App, AppCommand},
    commands::Function,
    utils::handle_from_icns,
};

pub fn default_app_paths()
-> impl IntoParallelIterator<Item = String> + for<'a> IntoParallelRefIterator<'a, Item = &'a String>
{
    let user_local_path = std::env::var("HOME").unwrap() + "/Applications/";

    [
        "/Applications/".to_string(),
        user_local_path,
        "/System/Applications/".to_string(),
        "/System/Applications/Utilities/".to_string(),
    ]
}

pub(crate) fn get_installed_apps(store_icons: bool) -> Vec<App> {
    default_app_paths()
        .into_par_iter()
        .flat_map(|path| discover_apps(path, store_icons))
        .collect()
}

/// This gets all the installed apps in the given directory
///
/// Is a fallback from the method in [`crate::platform::macos::discovery::get_installed_apps`]
///
/// the directories are defined in [`crate::app::tile::elm::new`]
fn discover_apps(
    dir: impl AsRef<Path>,
    store_icons: bool,
) -> impl IntoParallelIterator<Item = App> {
    info!("Indexing apps started");
    let entries: Vec<_> = fs::read_dir(dir.as_ref())
        .unwrap_or_else(|x| {
            error!(
                "Could not read directry: {} because of:\n{}",
                dir.as_ref().to_string_lossy(),
                x
            );
            exit(1)
        })
        .filter_map(|x| x.ok())
        .collect();

    entries.into_par_iter().filter_map(move |x| {
        let file_type = x.file_type().unwrap_or_else(|e| {
            error!("Unable to map entries: {}", &e.to_string());
            exit(1)
        });
        if !file_type.is_dir() {
            return None;
        }

        let file_name_os = x.file_name();
        let file_name = file_name_os.into_string().unwrap_or_else(|e| {
            error!(
                "Unable to get file name due to: {}",
                e.to_str().unwrap_or("")
            );
            exit(1)
        });

        if !file_name.ends_with(".app") {
            return None;
        }

        let path = x.path();
        let path_str = path.to_str().map(|x| x.to_string()).unwrap_or_else(|| {
            error!("Unable to get file_name");
            exit(1)
        });

        let icons = if store_icons {
            match fs::read_to_string(format!("{}/Contents/Info.plist", path_str)).map(|content| {
                let icon_line = content
                    .lines()
                    .scan(false, |expect_next, line| {
                        if *expect_next {
                            *expect_next = false;
                            // Return this line to the iterator
                            return Some(Some(line));
                        }

                        if line.trim() == "<key>CFBundleIconFile</key>" {
                            *expect_next = true;
                        }

                        // For lines that are not the one after the key, return None to skip
                        Some(None)
                    })
                    .flatten() // remove the Nones
                    .next()
                    .map(|x| {
                        x.trim()
                            .strip_prefix("<string>")
                            .unwrap_or("")
                            .strip_suffix("</string>")
                            .unwrap_or("")
                    });

                handle_from_icns(Path::new(&format!(
                    "{}/Contents/Resources/{}",
                    path_str,
                    icon_line.unwrap_or("AppIcon.icns")
                )))
            }) {
                Ok(Some(a)) => Some(a),
                _ => {
                    // Fallback method
                    let direntry = fs::read_dir(format!("{}/Contents/Resources", path_str))
                        .into_iter()
                        .flatten()
                        .filter_map(|x| {
                            let file = x.ok()?;
                            let name = file.file_name();
                            let file_name = name.to_str()?;
                            if file_name.ends_with(".icns") {
                                Some(file.path())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<PathBuf>>();

                    if direntry.len() > 1 {
                        let icns_vec = direntry
                            .iter()
                            .filter(|x| x.ends_with("AppIcon.icns"))
                            .collect::<Vec<&PathBuf>>();
                        handle_from_icns(icns_vec.first().unwrap_or(&&PathBuf::new()))
                    } else if !direntry.is_empty() {
                        handle_from_icns(direntry.first().unwrap_or(&PathBuf::new()))
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        };

        let name = file_name.strip_suffix(".app").unwrap().to_string();
        Some(App {
            ranking: 0,
            open_command: AppCommand::Function(Function::OpenApp(path_str)),
            desc: "Application".to_string(),
            icons,
            search_name: name.to_lowercase(),
            is_ai_response: false,
            display_name: name,
        })
    })
}
