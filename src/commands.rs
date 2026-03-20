//! This handles all the different commands that rustcast can perform, such as opening apps,
//! copying to clipboard, etc.
use std::{process::Command, thread};

use arboard::Clipboard;
use ignore::{DirEntry, WalkBuilder};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSURL;

use crate::{
    app::{
        ToApp,
        apps::{App, AppCommand},
    },
    calculator::Expr,
    clipboard::ClipBoardContentType,
    config::Config,
};

/// The different functions that rustcast can perform
#[derive(Debug, Clone, PartialEq)]
pub enum Function {
    OpenApp(String),
    RunShellCommand(String),
    OpenWebsite(String),
    RandomVar(i32), // Easter egg function
    CopyToClipboard(ClipBoardContentType),
    GoogleSearch(String),
    Calculate(Expr),
    OpenPrefPane,
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
            },

            Function::OpenPrefPane => {
                thread::spawn(move || {
                    NSWorkspace::new().openURL(&NSURL::fileURLWithPath(
                        &objc2_foundation::NSString::from_str(
                            &(std::env::var("HOME").unwrap_or("".to_string())
                                + "/.config/rustcast/config.toml"),
                        ),
                    ));
                });
            }
            Function::Quit => std::process::exit(0),
        }
    }
}

pub fn search(home: String, name: &str) -> impl Iterator<Item = App> {
    let mut builder = WalkBuilder::new(home);
    builder.follow_links(false);
    builder.threads(10);

    let name_clone = name.to_string();
    builder
        .build()
        .filter_map(move |x| {
            if let Ok(ent) = x {
                let name = ent.file_name().to_string_lossy();
                if name.contains(&name_clone) && !name.starts_with(".") {
                    Some(ent.to_app())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .take(400)
}

pub fn search_for_file(name: &str, dirs: Vec<&str>) -> Vec<App> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());

    dirs.iter().fold(Vec::with_capacity(400), move |vec, dir| {
        let mut apps = vec.clone();
        apps.extend(search(dir.replace("~", &home), name));
        apps
    })
}

impl ToApp for DirEntry {
    fn to_app(&self) -> App {
        let path = "~".to_string()
            + self
                .path()
                .to_str()
                .unwrap_or("")
                .to_string()
                .strip_prefix(&std::env::var("HOME").unwrap_or("".to_string()))
                .unwrap_or("");
        App {
            ranking: 0,
            open_command: AppCommand::Function(Function::OpenApp(path.clone())),
            desc: path,
            icons: None,
            display_name: self.file_name().to_str().unwrap_or("").to_string(),
            search_name: "".to_string(),
            is_ai_response: false,
        }
    }
}
