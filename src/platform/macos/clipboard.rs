use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::NSString;

/// Get any copied file URLs from the macOS general pasteboard.
pub fn get_copied_files() -> Option<Vec<String>> {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        let ns_filenames_type = NSString::from_str("NSFilenamesPboardType");

        let data: Option<Retained<AnyObject>> =
            objc2::msg_send![&pb, propertyListForType: &*ns_filenames_type];

        let mut files = Vec::new();
        if let Some(array) = data {
            let count: usize = objc2::msg_send![&array, count];
            for i in 0..count {
                let item: Option<Retained<NSString>> = objc2::msg_send![&array, objectAtIndex: i];
                if let Some(s) = item {
                    files.push(s.to_string());
                }
            }
        }

        if files.is_empty() { None } else { Some(files) }
    }
}

/// Write paths back to the macOS pasteboard.
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
