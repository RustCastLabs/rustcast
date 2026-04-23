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
}

impl ToApp for ClipBoardContentType {
    /// Returns the iced element for rendering the clipboard item, and the entire content since the
    /// display name is only the first line
    fn to_app(&self) -> App {
        let mut display_name = match self {
            ClipBoardContentType::Image(_) => "Image".to_string(),
            ClipBoardContentType::Text(a) => a.get(0..25).unwrap_or(a).to_string(),
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
            desc: "Clipboard Item".to_string(),
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
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_equality_is_content_based() {
        assert_eq!(
            ClipBoardContentType::Text("hello".to_string()),
            ClipBoardContentType::Text("hello".to_string())
        );
        assert_ne!(
            ClipBoardContentType::Text("hello".to_string()),
            ClipBoardContentType::Text("world".to_string())
        );
    }

    #[test]
    fn clipboard_to_app_truncates_and_uses_first_line_for_display() {
        let item = ClipBoardContentType::Text(
            "abcdefghijklmnopqrstuvwxyz\nsecond line".to_string(),
        );

        let app = item.to_app();

        assert_eq!(app.display_name, "abcdefghijklmnopqrstuvwxy");
        assert_eq!(app.search_name, "abcdefghijklmnopqrstuvwxy");
        assert_eq!(app.desc, "Clipboard Item");
    }
}
