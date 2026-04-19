//! This handles all of the platform specific stuff.
use iced::wgpu::rwh::WindowHandle;

pub use self::cross::default_app_paths;
use crate::app::apps::App;

pub mod cross;
#[cfg(target_os = "macos")]
pub mod macos;

pub fn set_activation_policy_accessory() {
    #[cfg(target_os = "macos")]
    self::macos::set_activation_policy_accessory();
}

pub fn window_config(handle: &WindowHandle) {
    #[cfg(target_os = "macos")]
    self::macos::macos_window_config(handle);
}

pub fn focus_this_app() {
    #[cfg(target_os = "macos")]
    self::macos::focus_this_app();
}

pub fn transform_process_to_ui_element() {
    #[cfg(target_os = "macos")]
    self::macos::transform_process_to_ui_element();
}

/// The kinds of haptic patterns that can be performed
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum HapticPattern {
    Generic,
    Alignment,
    LevelChange,
}

#[cfg(target_os = "macos")]
pub fn perform_haptic(pattern: HapticPattern) -> bool {
    self::macos::perform_haptic(pattern)
}

#[cfg(not(target_os = "macos"))]
pub fn perform_haptic(_: HapticPattern) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn get_installed_apps(store_icons: bool) -> Vec<App> {
    self::macos::get_installed_apps(store_icons)
}

#[cfg(not(target_os = "macos"))]
pub fn get_installed_apps(store_icons: bool) -> Vec<App> {
    self::cross::get_installed_apps(store_icons)
}

#[cfg(target_os = "macos")]
pub fn get_copied_files() -> Option<Vec<String>> {
    self::macos::get_copied_files()
}

#[cfg(not(target_os = "macos"))]
pub fn get_copied_files() -> Option<Vec<String>> {
    None
}

#[cfg(target_os = "macos")]
pub fn put_copied_files(paths: &[String]) {
    self::macos::put_copied_files(paths);
}

#[cfg(not(target_os = "macos"))]
pub fn put_copied_files(_: &[String]) {}

#[cfg(target_os = "macos")]
pub fn icon_of_path_ns(path: &str) -> Option<Vec<u8>> {
    self::macos::discovery::icon_of_path_ns(path)
}

#[cfg(not(target_os = "macos"))]
pub fn icon_of_path_ns(_: &str) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "macos")]
pub fn get_copied_text() -> Option<String> {
    self::macos::clipboard::get_copied_text()
}

#[cfg(not(target_os = "macos"))]
pub fn get_copied_text() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
pub fn get_copied_image() -> Option<crate::clipboard::ImageData<'static>> {
    self::macos::clipboard::get_copied_image()
}

#[cfg(not(target_os = "macos"))]
pub fn get_copied_image() -> Option<crate::clipboard::ImageData<'static>> {
    None
}

#[cfg(target_os = "macos")]
pub fn put_copied_text(text: &str) {
    self::macos::clipboard::put_copied_text(text);
}

#[cfg(not(target_os = "macos"))]
pub fn put_copied_text(_: &str) {}

#[cfg(target_os = "macos")]
pub fn put_copied_image(img: &crate::clipboard::ImageData) {
    self::macos::clipboard::put_copied_image(img);
}

#[cfg(not(target_os = "macos"))]
pub fn put_copied_image(_: &crate::clipboard::ImageData) {}
