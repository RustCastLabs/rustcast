//! This handles all the different commands that rustcast can perform, such as opening apps,
//! copying to clipboard, etc.
use std::{process::Command, thread};

use arboard::Clipboard;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSURL;

use crate::{
    app::apps::{App, AppCommand},
    calculator::Expr,
    clipboard::ClipBoardContentType,
    config::Config,
    quit::{terminate_all_apps, terminate_app},
};

/// The different functions that rustcast can perform
#[derive(Debug, Clone, PartialEq)]
pub enum Function {
    OpenApp(String),
    QuitApp(String),
    QuitAllApps,
    RunShellCommand(String),
    OpenWebsite(String),
    RandomVar(i32), // Easter egg function
    CopyToClipboard(ClipBoardContentType),
    GoogleSearch(String),
    Calculate(Expr),
    Quit,
}

impl Function {
    /// Run the command
    pub fn execute(&self, config: &Config) {
        match self {
            Function::OpenApp(path) => {
                let path = path.to_owned();
                thread::spawn(move || {
                    NSWorkspace::new().openURL(&NSURL::fileURLWithPath(
                        &objc2_foundation::NSString::from_str(&path),
                    ));
                });
            }
            Function::RunShellCommand(command) => {
                Command::new("sh").arg("-c").arg(command).spawn().ok();
            }
            Function::RandomVar(var) => {
                Clipboard::new()
                    .unwrap()
                    .set_text(var.to_string())
                    .unwrap_or(());
            }

            Function::QuitAllApps => {
                terminate_all_apps();
            }

            Function::QuitApp(name) => {
                terminate_app(name.to_owned());
            }

            Function::GoogleSearch(query_string) => {
                let query_args = query_string.replace(" ", "+");
                let query = config.search_url.replace("%s", &query_args);
                let query = query.strip_suffix("?").unwrap_or(&query).to_string();
                thread::spawn(move || {
                    NSWorkspace::new().openURL(
                        &NSURL::URLWithString_relativeToURL(
                            &objc2_foundation::NSString::from_str(&query),
                            None,
                        )
                        .unwrap(),
                    );
                });
            }

            Function::OpenWebsite(url) => {
                let open = if url.starts_with("http") {
                    url.to_owned()
                } else {
                    format!("https://{}", url)
                };
                thread::spawn(move || {
                    NSWorkspace::new().openURL(
                        &NSURL::URLWithString_relativeToURL(
                            &objc2_foundation::NSString::from_str(&open),
                            None,
                        )
                        .unwrap(),
                    );
                });
            }

            Function::Calculate(expr) => {
                Clipboard::new()
                    .unwrap()
                    .set_text(expr.eval().map(|x| x.to_string()).unwrap_or("".to_string()))
                    .unwrap_or(());
            }

            Function::CopyToClipboard(clipboard_content) => match clipboard_content {
                ClipBoardContentType::Text(text) => {
                    Clipboard::new().unwrap().set_text(text).ok();
                }
                ClipBoardContentType::Image(img) => {
                    Clipboard::new().unwrap().set_image(img.to_owned_img()).ok();
                }
                ClipBoardContentType::Files(files, _) => {
                    crate::platform::put_copied_files(files);
                }
            },

            Function::Quit => std::process::exit(0),
        }
    }
}

/// Convert an absolute file path into an App for display in file search results.
///
/// Returns None for dotfiles or paths that cannot be parsed.
pub fn path_to_app(absolute_path: &str, home_dir: &str) -> Option<App> {
    assert!(!home_dir.is_empty(), "Home directory must not be empty.");
    let path = absolute_path.trim();
    if path.is_empty() {
        return None;
    }

    let filename = std::path::Path::new(path).file_name()?.to_str()?;
    if filename.starts_with('.') {
        return None;
    }

    let display_path = if let Some(suffix) = path.strip_prefix(home_dir) {
        format!("~{suffix}")
    } else {
        path.to_string()
    };

    Some(App {
        ranking: 0,
        open_command: AppCommand::Function(Function::OpenApp(path.to_string())),
        desc: display_path,
        icons: None,
        display_name: filename.to_string(),
        search_name: filename.to_lowercase(),
    })
}
