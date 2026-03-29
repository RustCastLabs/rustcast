//! This has all the logic regarding the cliboard history
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct ImageData<'a> {
    pub width: usize,
    pub height: usize,
    pub bytes: Cow<'a, [u8]>,
}

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
        let mut display_name = match self {
            ClipBoardContentType::Image(_) => "Image".to_string(),
            ClipBoardContentType::Text(a) => a.get(0..25).unwrap_or(a).to_string(),
            ClipBoardContentType::Files(f, _) => {
                if f.len() == 1 {
                    let path = std::path::Path::new(&f[0]);
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if name.is_empty() {
                        f[0].clone()
                    } else {
                        name
                    }
                } else {
                    format!("{} Files", f.len())
                }
            }
        };

        let search_name = match self {
            ClipBoardContentType::Image(img) => format!("Image ({})", img.bytes.len()),
            ClipBoardContentType::Text(a) => a.to_string(),
            ClipBoardContentType::Files(f, _) => {
                if f.len() == 1 {
                    display_name.clone()
                } else {
                    format!("{} Files: {}", f.len(), f.join(", "))
                }
            }
        };

        let self_clone = self.clone();

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
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Image(a), Self::Image(b)) => a.bytes == b.bytes,
            (Self::Files(f1, img1), Self::Files(f2, img2)) => {
                if f1 != f2 {
                    return false;
                }
                match (img1, img2) {
                    (Some(a), Some(b)) => a.bytes == b.bytes,
                    (None, None) => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

pub fn rotate_image(img: &image::RgbaImage, angle_radians: f32) -> image::RgbaImage {
    let (width, height) = img.dimensions();
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let cos_a = angle_radians.cos();
    let sin_a = angle_radians.sin();

    let corners = [
        (-cx, -cy),
        (width as f32 - cx, -cy),
        (width as f32 - cx, height as f32 - cy),
        (-cx, height as f32 - cy)
    ];

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for &(x, y) in &corners {
        let rx = x * cos_a - y * sin_a;
        let ry = x * sin_a + y * cos_a;
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }

    let new_width = (max_x - min_x).ceil() as u32;
    let new_height = (max_y - min_y).ceil() as u32;
    let new_cx = new_width as f32 / 2.0;
    let new_cy = new_height as f32 / 2.0;

    let mut res = image::RgbaImage::new(new_width, new_height);
    for y in 0..new_height {
        for x in 0..new_width {
            let tx = x as f32 - new_cx;
            let ty = y as f32 - new_cy;
            
            let src_x = tx * cos_a + ty * sin_a + cx;
            let src_y = -tx * sin_a + ty * cos_a + cy;

            if src_x >= 0.0 && src_x < width as f32 && src_y >= 0.0 && src_y < height as f32 {
                let p = img.get_pixel(src_x as u32, src_y as u32);
                res.put_pixel(x, y, *p);
            }
        }
    }
    res
}

pub fn generate_multi_file_thumbnail(files: &[String]) -> Option<ImageData<'static>> {
    use std::borrow::Cow;

    if files.len() <= 1 { return None; }
    let limit = std::cmp::min(files.len(), 3);
    let mut images = Vec::new();

    for i in 0..limit {
        let mut path_str: String = files[i].clone();
        if path_str.starts_with("file://") {
            path_str = path_str.strip_prefix("file://").unwrap().replace("%20", " ");
        }
        
        let p = std::path::Path::new(&path_str);
        let is_image = if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            matches!(
                ext.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "tiff"
            )
        } else {
            false
        };

        let mut loaded_img = None;
        if is_image {
            if let Ok(img) = image::open(&path_str) {
                loaded_img = Some(img);
            }
        }
        
        if loaded_img.is_none() {
            if let Some(sys_icon_bytes) = crate::platform::icon_of_path_ns(&path_str) {
                if let Ok(img) = image::load_from_memory(&sys_icon_bytes) {
                    loaded_img = Some(img);
                }
            }
        }

        if let Some(img) = loaded_img {
            let resized = img.thumbnail(150, 150).into_rgba8();
            images.push(resized);
        }
    }

    if images.is_empty() { return None; }

    let mut canvas = image::RgbaImage::new(250, 250);
    let rotations: [f32; 3] = [-0.15, 0.15, 0.0];

    for (i, img) in images.iter().enumerate() {
        let angle = rotations[i % rotations.len()];
        let rotated = rotate_image(img, angle);
        
        let cx = 125 - rotated.width() as i32 / 2;
        let cy = 125 - rotated.height() as i32 / 2;
        
        image::imageops::overlay(&mut canvas, &rotated, cx as i64, cy as i64);
    }
    
    Some(ImageData {
        width: canvas.width() as usize,
        height: canvas.height() as usize,
        bytes: Cow::Owned(canvas.into_raw())
    })
}
