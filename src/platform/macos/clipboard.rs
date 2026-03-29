use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::NSString;

/// Get any copied file URLs from the macOS general pasteboard.
///
/// # Safety
/// This function executes raw Objective-C messaging. To manually prevent segfaults, 
/// it validates `isKindOfClass: NSArray` at runtime before iterating or invoking array-specific selectors.
pub fn get_copied_files() -> Option<Vec<String>> {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        let ns_filenames_type = NSString::from_str("NSFilenamesPboardType");

        let data: Option<Retained<AnyObject>> =
            objc2::msg_send![&pb, propertyListForType: &*ns_filenames_type];

        let mut files = Vec::new();
        if let Some(array) = data {
            let is_array: bool = objc2::msg_send![&array, isKindOfClass: objc2::class!(NSArray)];
            if is_array {
                let count: usize = objc2::msg_send![&array, count];
                for i in 0..count {
                    let item: Option<Retained<NSString>> = objc2::msg_send![&array, objectAtIndex: i];
                    if let Some(s) = item {
                        files.push(s.to_string());
                    }
                }
            }
        }

        if files.is_empty() { None } else { Some(files) }
    }
}

/// Write paths back to the macOS pasteboard.
///
/// # Safety
/// Interacts directly with the Core Foundation pasteboard server. Standard Foundation classes
/// like `NSMutableArray` and `NSString` are utilized directly via `msg_send!` without external linkage.
pub fn put_copied_files(paths: &[String]) {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();

        let ns_filenames_type = NSString::from_str("NSFilenamesPboardType");
        let ns_array_class = objc2::class!(NSMutableArray);

        // Use Retained<AnyObject> to bypass strict array types
        let array: Retained<AnyObject> =
            objc2::msg_send![ns_array_class, arrayWithCapacity: paths.len()];

        for p in paths {
            let ns_str = NSString::from_str(p);
            let _: () = objc2::msg_send![&array, addObject: &*ns_str];
        }

        let _: bool = objc2::msg_send![&pb, setPropertyList: &*array, forType: &*ns_filenames_type];
    }
}

pub fn get_copied_text() -> Option<String> {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        let ns_string_type = NSString::from_str("public.utf8-plain-text");
        let data: Option<Retained<NSString>> = objc2::msg_send![&pb, stringForType: &*ns_string_type];
        data.map(|s| s.to_string())
    }
}

pub fn get_copied_image() -> Option<crate::clipboard::ImageData<'static>> {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        
        let tiff_data: Option<Retained<objc2_foundation::NSData>> = 
            objc2::msg_send![&pb, dataForType: &*NSString::from_str("public.tiff")];
        
        let png_data: Option<Retained<objc2_foundation::NSData>> = 
            if tiff_data.is_none() {
                objc2::msg_send![&pb, dataForType: &*NSString::from_str("public.png")]
            } else {
                None
            };
        
        let data = tiff_data.or(png_data)?;
        
        let slice = data.to_vec();
        
        if let Ok(img) = image::load_from_memory(&slice) {
            let rgba = img.into_rgba8();
            return Some(crate::clipboard::ImageData {
                width: rgba.width() as usize,
                height: rgba.height() as usize,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            });
        }
        None
    }
}

pub fn put_copied_text(text: &str) {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        
        let ns_str = NSString::from_str(text);
        let _: bool = objc2::msg_send![&pb, setString: &*ns_str, forType: &*NSString::from_str("public.utf8-plain-text")];
    }
}

pub fn put_copied_image(img: &crate::clipboard::ImageData) {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        
        let mut buf = std::io::Cursor::new(Vec::new());
        if let Some(rgba) = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec()) {
            if image::write_buffer_with_format(
                &mut buf,
                &rgba,
                img.width as u32,
                img.height as u32,
                image::ColorType::Rgba8,
                image::ImageFormat::Png
            ).is_ok() {
                let png_bytes = buf.into_inner();
                let ns_data = objc2_foundation::NSData::with_bytes(&png_bytes);
                let _: bool = objc2::msg_send![&pb, setData: &*ns_data, forType: &*NSString::from_str("public.png")];
            }
        }
    }
}
