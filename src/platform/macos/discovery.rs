//! macOS application discovery using Launch Services.
//!
//! This module uses the undocumented `LSCopyAllApplicationURLs` API to enumerate
//! all registered applications on the system. This private API has been stable
//! since macOS 10.5 and is widely used by launcher applications (Alfred, Raycast, etc.).
//!
//! Since the symbol is not exported in Apple's `.tbd` stub files (which only list
//! documented APIs), we load it at runtime via `dlsym` from the LaunchServices
//! framework. If loading fails, we fall back to the cross-platform directory
//! scanning approach.

use core::{
    ffi::{CStr, c_void},
    mem,
    ptr::{self, NonNull},
};
use std::{
    env,
    io::Cursor,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use iced::widget::image::Handle;
use log::error;
use objc2::{Message, rc::Retained};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSImageRep, NSWorkspace};
use objc2_core_foundation::{CFArray, CFRetained, CFURL};
use objc2_foundation::{
    NSBundle, NSData, NSDictionary, NSNumber, NSSize, NSString, NSURL, ns_string,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator as _};

use crate::{
    app::apps::{App, AppCommand},
    commands::Function,
};

use super::super::cross;

/// Function signature for `LSCopyAllApplicationURLs`.
///
/// This undocumented Launch Services function retrieves URLs for all applications
/// registered with the system. It follows Core Foundation's "Copy Rule" - the
/// caller owns the returned `CFArray` and is responsible for releasing it.
///
/// # Parameters
/// - `out`: Pointer to receive the `CFArray<CFURL>` of application URLs
///
/// # Returns
/// - `0` (`noErr`) on success
/// - Non-zero `OSStatus` error code on failure
type LSCopyAllApplicationURLsFn = unsafe extern "C" fn(out: *mut *const CFArray<CFURL>) -> i32;

/// Path to the LaunchServices framework binary within CoreServices.
const LAUNCHSERVICES_PATH: &CStr =
    c"/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/LaunchServices";

/// Logs the last `dlerror` message with a prefix.
///
/// # Safety
///
/// Must be called immediately after a failed `dlopen`/`dlsym` call,
/// before any other dl* functions are invoked.
unsafe fn log_dlerror(prefix: &str) {
    let error = unsafe { libc::dlerror() };
    let message = if error.is_null() {
        "unknown error".into()
    } else {
        unsafe { CStr::from_ptr(error) }.to_string_lossy()
    };

    error!("{prefix}: {message}");
}

/// Dynamically loads `LSCopyAllApplicationURLs` from the LaunchServices framework.
///
/// This function is called once and cached via `LazyLock`. We use dynamic loading
/// because the symbol is undocumented and not present in Apple's `.tbd` stub files,
/// which prevents static linking on modern macOS.
///
/// The library handle is intentionally kept open for the process lifetime since
/// we cache the function pointer.
///
/// # Returns
///
/// The function pointer if successfully loaded, `None` otherwise.
fn load_symbol() -> Option<LSCopyAllApplicationURLsFn> {
    // SAFETY: We pass a valid null-terminated path string to dlopen.
    // RTLD_NOW resolves symbols immediately; RTLD_LOCAL keeps them private.
    let lib = unsafe {
        libc::dlopen(
            LAUNCHSERVICES_PATH.as_ptr(),
            libc::RTLD_NOW | libc::RTLD_LOCAL,
        )
    };

    let Some(lib) = NonNull::new(lib) else {
        // SAFETY: dlopen has returned a null pointer, indicating failure.
        unsafe { log_dlerror("failed to load LaunchServices framework") };
        return None;
    };

    // Clear any prior error before checking dlsym result.
    unsafe { libc::dlerror() };

    // SAFETY: We pass a valid library handle and null-terminated symbol name.
    let sym = unsafe { libc::dlsym(lib.as_ptr(), c"_LSCopyAllApplicationURLs".as_ptr()) };
    let Some(sym) = NonNull::new(sym) else {
        // SAFETY: dlsym has returned a null pointer, indicating failure.
        unsafe { log_dlerror("failed to find symbol `LSCopyAllApplicationURLs`") };

        // SAFETY: lib is a valid handle from successful dlopen.
        unsafe { libc::dlclose(lib.as_ptr()) };
        return None;
    };

    // SAFETY: We've verified the symbol exists. The function signature matches
    // the known (though undocumented) API based on reverse engineering and
    // widespread usage in other applications.
    Some(unsafe { mem::transmute::<*mut c_void, LSCopyAllApplicationURLsFn>(sym.as_ptr()) })
}

/// Retrieves URLs for all applications registered with Launch Services.
///
/// Uses the cached function pointer from [`load_symbol`] to call the
/// undocumented `LSCopyAllApplicationURLs` API.
///
/// # Returns
///
/// `Some(CFRetained<CFArray<CFURL>>)` containing application URLs on success,
/// `None` if the symbol couldn't be loaded or the API call failed.
fn registered_app_urls() -> Option<CFRetained<CFArray<CFURL>>> {
    static SYM: LazyLock<Option<LSCopyAllApplicationURLsFn>> = LazyLock::new(load_symbol);

    let sym = (*SYM)?;
    let mut urls_ptr = ptr::null();

    // SAFETY: We've verified `sym` is a valid function pointer. We pass a valid
    // mutable pointer to receive the output. The function follows the "Copy Rule"
    // so we take ownership of the returned CFArray.
    let err = unsafe { sym(&mut urls_ptr) };

    if err != 0 {
        error!("LSCopyAllApplicationURLs failed with error code: {err}");
        return None;
    }

    let Some(url_ptr) = NonNull::new(urls_ptr.cast_mut()) else {
        error!("LSCopyAllApplicationURLs returned null on success");
        return None;
    };

    // SAFETY: LSCopyAllApplicationURLs returns a +1 retained CFArray on success.
    // We transfer ownership to CFRetained which will call CFRelease when dropped.
    Some(unsafe { CFRetained::from_raw(url_ptr) })
}

/// Directories that contain user-facing applications.
/// Apps in these directories are included by default (after LSUIElement check).
static USER_APP_DIRECTORIES: LazyLock<&'static [&'static Path]> = LazyLock::new(|| {
    // These strings live for the lifetime of the program, so are safe to leak.
    let items = [
        Path::new("/Applications/"),
        Path::new("/System/Applications/"),
    ];

    let Some(home) = env::var_os("HOME") else {
        return Box::leak(Box::new(items));
    };

    let home_apps = Path::new(&home).join("Applications/");
    let home_apps = PathBuf::leak(home_apps);

    Box::leak(Box::new([items[0], items[1], home_apps]))
});

/// Checks if an app path is in a trusted user-facing application directory.
fn is_in_user_app_directory(path: &Path) -> bool {
    USER_APP_DIRECTORIES
        .iter()
        .any(|directory| path.starts_with(directory))
}

/// Extracts application metadata from a bundle URL.
///
/// Queries the bundle's `Info.plist` for display name and icon, with the
/// following fallback chain for the app name:
/// 1. `CFBundleDisplayName` - localized display name
/// 2. `CFBundleName` - short bundle name
/// 3. File stem from path (e.g., "Safari" from "Safari.app")
///
/// # Returns
///
/// `Some(App)` if the bundle is valid and has a determinable name, `None` otherwise.
fn query_app(url: impl AsRef<NSURL>, store_icons: bool) -> Option<App> {
    let url = url.as_ref();
    let path = url.to_file_path()?;
    if is_nested_inside_another_app(&path) || is_helper_location(&path) {
        return None;
    }

    let bundle = NSBundle::bundleWithURL(url)?;
    let info = bundle.infoDictionary()?;

    let get_string = |key: &NSString| -> Option<String> {
        info.objectForKey(key)?
            .downcast::<NSString>()
            .ok()
            .map(|s| s.to_string())
    };

    let is_truthy = |key: &NSString| -> bool {
        info.objectForKey(key)
            .map(|v| {
                // Check for boolean true or string "1"/"YES"
                v.downcast_ref::<NSNumber>().is_some_and(|n| n.boolValue())
                    || v.downcast_ref::<NSString>().is_some_and(|s| {
                        s.to_string() == "1" || s.to_string().eq_ignore_ascii_case("YES")
                    })
            })
            .unwrap_or(false)
    };

    // Filter out background-only apps (daemons, agents, internal system apps)
    if is_truthy(ns_string!("LSBackgroundOnly")) {
        return None;
    }

    // For apps outside trusted directories, require LSApplicationCategoryType to be set.
    // This filters out internal system apps (SCIM, ShortcutsActions, etc.) while keeping
    // user-facing apps like Finder that happen to live in /System/Library/CoreServices/.
    if !is_in_user_app_directory(&path)
        && get_string(ns_string!("LSApplicationCategoryType")).is_none()
    {
        return None;
    }

    let plist_name = get_string(ns_string!("CFBundleDisplayName"))
        .or_else(|| get_string(ns_string!("CFBundleName")));

    let file_path_name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned());

    let name = if let Some(name) = file_path_name {
        name
    } else if let Some(name) = plist_name {
        name
    } else {
        return None;
    };

    let icon = icon_of_path_ns(path.to_str().unwrap_or(&name)).unwrap_or(vec![]);
    let icons = if store_icons {
        image::ImageReader::new(Cursor::new(icon))
            .with_guessed_format()
            .unwrap()
            .decode()
            .ok()
            .map(|img| Handle::from_rgba(img.width(), img.height(), img.into_bytes()))
    } else {
        None
    };

    Some(App {
        ranking: 0,
        display_name: name.clone(),
        search_name: name.to_lowercase(),
        desc: "Application".to_string(),
        icons,
        open_command: AppCommand::Function(Function::OpenApp(path.to_string_lossy().into_owned())),
    })
}

/// Returns all installed applications discovered via Launch Services.
///
/// Attempts to use the native `LSCopyAllApplicationURLs` API for comprehensive
/// app discovery. If the API is unavailable (symbol not found or call fails),
/// falls back to the cross-platform directory scanning approach.
///
/// # Arguments
///
/// * `store_icons` - Whether to load application icons (slower but needed for display)
pub(crate) fn get_installed_apps(store_icons: bool) -> Vec<App> {
    let Some(registered_app_urls) = registered_app_urls() else {
        error!("native app discovery unavailable, falling back to directory scan");
        return cross::get_installed_apps(store_icons);
    };

    // Intermediate allocation into a vec allows us to parallelize the iteration, speeding up discovery by ~5x.
    let urls: Vec<_> = registered_app_urls.into_iter().collect();

    urls.into_par_iter()
        .filter_map(|url| query_app(url, store_icons))
        .collect()
}

fn is_nested_inside_another_app(app_path: &Path) -> bool {
    // Walk up ancestors; if we find an *.app component that is NOT the last component,
    // then this app is nested inside another app bundle.
    let comps: Vec<_> = app_path.components().collect();
    // Normalize: if path ends with ".../Foo.app", we look for any earlier "*.app".
    for component in comps.iter().take(comps.len().saturating_sub(1)) {
        if let std::path::Component::Normal(name) = component
            && name.to_string_lossy().ends_with(".app")
        {
            return true;
        }
    }
    false
}

fn is_helper_location(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/Contents/Library/LoginItems/")
        || s.contains("/Contents/XPCServices/")
        || s.contains("/Contents/Helpers/")
        || s.contains("/Contents/Frameworks/")
        || s.contains("/Library/PrivilegedHelperTools/")
}

/// <https://github.com/cardisoft/cardinal/blob/339b27c3c6abaf94405a9ab09ec39296baba4f91/fs-icon/src/lib.rs#L37>
pub fn icon_of_path_ns(path: &str) -> Option<Vec<u8>> {
    objc2::rc::autoreleasepool(|_| -> Option<Vec<u8>> {
        let path_ns = NSString::from_str(path);
        let image = NSWorkspace::sharedWorkspace().iconForFile(&path_ns);

        // Choose what you consider "high quality" output.
        // 256 is a good default; you can bump to 512 if you want.
        let target: f64 = 256.0;

        let png_data: Retained<NSData> = (|| -> Option<_> {
            unsafe {
                // Pick the best representation:
                // - Prefer the smallest rep that is >= target (avoids upscaling)
                // - Otherwise pick the largest available rep
                let mut best_rep = None::<Retained<NSImageRep>>;
                let mut best_w = 0.0;
                let mut best_h = 0.0;

                let mut largest_rep = None::<Retained<NSImageRep>>;
                let mut largest_area = 0.0;
                let mut largest_w = 0.0;
                let mut largest_h = 0.0;

                for rep in image.representations().iter() {
                    let s = rep.size();
                    let w = s.width;
                    let h = s.height;

                    // Track largest (fallback)
                    let area = w * h;
                    if area > largest_area {
                        largest_area = area;
                        largest_rep = Some(rep.retain());
                        largest_w = w;
                        largest_h = h;
                    }

                    // Track best rep for target (no upscale if possible)
                    if w >= target && h >= target {
                        let best_area = best_w * best_h;
                        if best_rep.is_none() || area < best_area {
                            best_rep = Some(rep.retain());
                            best_w = w;
                            best_h = h;
                        }
                    }
                }

                let (rep, out_w, out_h) = if let Some(rep) = best_rep {
                    (rep, target, target)
                } else if let Some(rep) = largest_rep {
                    // If nothing reaches target, use largest and render at its native size
                    (rep, largest_w, largest_h)
                } else {
                    return None;
                };

                let new_image = NSImage::imageWithSize_flipped_drawingHandler(
                    NSSize::new(out_w, out_h),
                    false,
                    &block2::RcBlock::new(move |rect| {
                        rep.drawInRect(rect);
                        true.into()
                    }),
                );

                NSBitmapImageRep::imageRepWithData(&*new_image.TIFFRepresentation()?)?
                    .representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
            }
        })()?;

        Some(png_data.to_vec())
    })
}
