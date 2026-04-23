//! This has all the utility functions that rustcast uses
use std::{path::Path, thread};

use iced::widget::image::Handle;
use icns::IconFamily;
use image::RgbaImage;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSURL;

pub fn icns_data_to_handle(data: Vec<u8>) -> Option<Handle> {
    let family = IconFamily::read(std::io::Cursor::new(&data)).ok()?;

    let icon_type = family.available_icons();

    let icon = family.get_icon_with_type(*icon_type.first()?).ok()?;
    let image = RgbaImage::from_raw(icon.width(), icon.height(), icon.data().to_vec())?;
    Some(Handle::from_rgba(
        image.width(),
        image.height(),
        image.into_raw(),
    ))
}

/// This converts an icns file to an iced image handle
pub(crate) fn handle_from_icns(path: &Path) -> Option<Handle> {
    let data = std::fs::read(path).ok()?;
    icns_data_to_handle(data)
}

/// Open a provided URL (Platform specific)
pub fn open_url(url: &str) {
    let url = url.to_owned();
    thread::spawn(move || {
        NSWorkspace::new().openURL(
            &NSURL::URLWithString_relativeToURL(&objc2_foundation::NSString::from_str(&url), None)
                .unwrap(),
        );
    });
}

/// Check if the provided string is a valid url
pub fn is_valid_url(s: &str) -> bool {
    match s
        .chars()
        .rev()
        .fold(String::new(), |a, b| format!("{}{}", a, b))
        .split_once('.')
        .unwrap_or(("", ""))
        .0
    {
        "" => false,

        // Common gTLDs (reversed)
        "moc" | "gro" | "ten" | "ude" | "vog" | "lim" | "ofni" | "zib" | "eman" | "orp" | "ppa"
        | "ved" | "oi" | "ia" | "oc" | "em" => true,

        // Common ccTLDs (reversed)
        "su" | "ku" | "ed" | "rf" | "se" | "ti" | "ln" | "on" | "if" | "kd" | "lp" | "zc"
        | "ta" | "hc" | "eb" | "ei" | "tp" | "rg" | "ur" | "au" | "rt" | "ni" | "pj" | "rk"
        | "nc" | "wt" | "kh" | "gs" | "ym" | "di" | "ht" | "nv" | "rb" | "ra" | "xm" | "ac"
        | "ua" | "zn" | "az" | "ge" | "li" | "as" | "ea" => true,

        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_url;

    #[test]
    fn url_validation_accepts_supported_tlds() {
        assert!(is_valid_url("example.com"));
        assert!(is_valid_url("example.app"));
        assert!(is_valid_url("openai.ai"));
    }

    #[test]
    fn url_validation_rejects_non_urls() {
        assert!(!is_valid_url("localhost"));
        assert!(!is_valid_url("not a url"));
        assert!(!is_valid_url("example.invalidtld"));
    }
}
