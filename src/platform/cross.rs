use std::{
    fs,
    path::{Path, PathBuf},
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
fn discover_apps(dir: impl AsRef<Path>, store_icons: bool) -> Vec<App> {
    info!("Indexing apps started");
    let entries = match fs::read_dir(dir.as_ref()) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(error) => {
            error!(
                "Could not read directry: {} because of:\n{}",
                dir.as_ref().to_string_lossy(),
                error
            );
            return Vec::new();
        }
    };

    entries
        .into_par_iter()
        .filter_map(move |x| {
            let file_type = match x.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    error!("Unable to map entries: {}", error);
                    return None;
                }
            };
            if !file_type.is_dir() {
                return None;
            }

            let file_name_os = x.file_name();
            let file_name = file_name_os.to_string_lossy().to_string();

            if !file_name.ends_with(".app") {
                return None;
            }

            let path = x.path();
            let path_str = path.to_string_lossy().to_string();

            let icons = if store_icons {
                find_bundle_icon_path(&path).and_then(|icon_path| handle_from_icns(&icon_path))
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
                display_name: name,
            })
        })
        .collect()
}

fn plist_icon_name(contents: &str) -> Option<String> {
    contents
        .lines()
        .scan(false, |expect_next, line| {
            if *expect_next {
                *expect_next = false;
                return Some(Some(line));
            }

            if line.trim() == "<key>CFBundleIconFile</key>" {
                *expect_next = true;
            }

            Some(None)
        })
        .flatten()
        .next()
        .map(|line| {
            line.trim()
                .strip_prefix("<string>")
                .unwrap_or("")
                .strip_suffix("</string>")
                .unwrap_or("")
                .to_string()
        })
        .filter(|line| !line.is_empty())
}

fn find_bundle_icon_path(bundle_path: &Path) -> Option<PathBuf> {
    let resources_dir = bundle_path.join("Contents/Resources");
    let plist_path = bundle_path.join("Contents/Info.plist");

    if let Ok(contents) = fs::read_to_string(&plist_path)
        && let Some(icon_name) = plist_icon_name(&contents)
    {
        let icon_path = resources_dir.join(icon_name);
        if icon_path.exists() {
            return Some(icon_path);
        }
    }

    let icns_files = fs::read_dir(resources_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "icns"))
        .collect::<Vec<PathBuf>>();

    if icns_files.len() > 1 {
        icns_files
            .iter()
            .find(|path| path.file_name().is_some_and(|name| name == "AppIcon.icns"))
            .cloned()
            .or_else(|| icns_files.first().cloned())
    } else {
        icns_files.first().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_app_bundle(base: &Path, name: &str, plist: Option<&str>) -> PathBuf {
        let bundle_path = base.join(format!("{name}.app"));
        fs::create_dir_all(bundle_path.join("Contents/Resources")).unwrap();
        if let Some(plist) = plist {
            fs::write(bundle_path.join("Contents/Info.plist"), plist).unwrap();
        }
        bundle_path
    }

    #[test]
    fn plist_icon_name_extracts_icon_file() {
        let plist = r#"
            <plist>
                <key>CFBundleIconFile</key>
                <string>Custom.icns</string>
            </plist>
        "#;

        assert_eq!(plist_icon_name(plist), Some("Custom.icns".to_string()));
    }

    #[test]
    fn find_bundle_icon_path_prefers_plist_icon_then_appicon() {
        let dir = tempdir().unwrap();
        let bundle = create_app_bundle(
            dir.path(),
            "Test",
            Some(
                r#"
                <key>CFBundleIconFile</key>
                <string>Custom.icns</string>
            "#,
            ),
        );

        let custom_icon = bundle.join("Contents/Resources/Custom.icns");
        let app_icon = bundle.join("Contents/Resources/AppIcon.icns");
        fs::write(&custom_icon, b"icon").unwrap();
        fs::write(&app_icon, b"fallback").unwrap();

        assert_eq!(find_bundle_icon_path(&bundle), Some(custom_icon));
    }

    #[test]
    fn find_bundle_icon_path_falls_back_to_appicon_when_needed() {
        let dir = tempdir().unwrap();
        let bundle = create_app_bundle(dir.path(), "Test", None);
        let app_icon = bundle.join("Contents/Resources/AppIcon.icns");
        fs::write(&app_icon, b"fallback").unwrap();

        assert_eq!(find_bundle_icon_path(&bundle), Some(app_icon));
    }

    #[test]
    fn discover_apps_skips_invalid_entries_instead_of_exiting() {
        let dir = tempdir().unwrap();
        create_app_bundle(dir.path(), "Safari", None);
        fs::create_dir_all(dir.path().join("NotAnApp")).unwrap();
        fs::write(dir.path().join("notes.txt"), b"ignore").unwrap();

        let apps = discover_apps(dir.path(), false);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].display_name, "Safari");
        assert_eq!(apps[0].search_name, "safari");
    }
}
