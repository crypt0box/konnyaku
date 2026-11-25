use serde::Serialize;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{capture, own_app_is_frontmost};
#[cfg(all(target_os = "macos", not(debug_assertions)))]
pub use macos::prompt_for_accessibility_if_needed;

#[cfg(not(target_os = "macos"))]
mod windows;
#[cfg(not(target_os = "macos"))]
pub use windows::{capture, own_app_is_frontmost};
#[cfg(all(not(target_os = "macos"), not(debug_assertions)))]
pub use windows::prompt_for_accessibility_if_needed;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureSource {
    Selection,
    Clipboard,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// data: URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_png: Option<String>,
    pub source: CaptureSource,
}

impl Capture {
    pub fn nothing(source: CaptureSource) -> Capture {
        Capture {
            html: None,
            text: None,
            image_png: None,
            source,
        }
    }
}
