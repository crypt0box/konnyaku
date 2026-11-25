use super::{Capture, CaptureSource};

pub fn own_app_is_frontmost() -> bool {
    false
}

pub fn capture() -> Capture {
    Capture {
        html: None,
        text: None,
        image_png: None,
        source: CaptureSource::Clipboard,
    }
}

#[cfg(not(debug_assertions))]
pub fn prompt_for_accessibility_if_needed() {}
