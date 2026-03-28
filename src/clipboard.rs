//! This has all the logic regarding the cliboard history
use arboard::ImageData;

use crate::{
    app::{ToApp, apps::App},
    commands::Function,
};

/// The kinds of clipboard content that rustcast can handle and their contents
#[derive(Debug, Clone)]
pub enum ClipBoardContentType {
    Text(String),
    Image(ImageData<'static>),
    Files(Vec<String>, Option<ImageData<'static>>),
}

impl ToApp for ClipBoardContentType {
    /// Returns the iced element for rendering the clipboard item, and the entire content since the
    /// display name is only the first line
    fn to_app(&self) -> App {
        let (mut display_name, desc) = match self {
            ClipBoardContentType::Image(_) => ("Image".to_string(), "Clipboard Item".to_string()),
            ClipBoardContentType::Text(a) => (
                a.get(0..25).unwrap_or(a).to_string(),
                "Clipboard Item".to_string(),
            ),
            ClipBoardContentType::Files(f, _) => {
                if f.len() == 1 {
                    let path = std::path::Path::new(&f[0]);
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    // Fall back to the raw path string if the file name was entirely empty
                    let mut final_name = name;
                    if final_name.is_empty() {
                        final_name = f[0].clone();
                    }
                    (final_name, f[0].clone())
                } else {
                    (
                        format!("{} Files", f.len()),
                        "Multiple files copied".to_string(),
                    )
                }
            }
        };

        let self_clone = self.clone();
        let search_name = display_name.clone();

        // only get the first line from the contents
        display_name = display_name.lines().next().unwrap_or("").to_string();

        App {
            ranking: 0,
            open_command: crate::app::apps::AppCommand::Function(Function::CopyToClipboard(
                self_clone.to_owned(),
            )),
            desc,
            icons: None,
            display_name,
            search_name,
        }
    }
}

impl PartialEq for ClipBoardContentType {
    /// Let cliboard items be comparable
    fn eq(&self, other: &Self) -> bool {
        if let Self::Text(a) = self
            && let Self::Text(b) = other
        {
            return a == b;
        } else if let Self::Image(image_data) = self
            && let Self::Image(other_image_data) = other
        {
            return image_data.bytes == other_image_data.bytes;
        } else if let Self::Files(f1, img1) = self
            && let Self::Files(f2, img2) = other
        {
            if f1 != f2 {
                return false;
            }
            return match (img1, img2) {
                (Some(a), Some(b)) => a.bytes == b.bytes,
                (None, None) => true,
                _ => false,
            };
        }
        false
    }
}
