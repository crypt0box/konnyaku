use std::thread::sleep;
use std::time::{Duration, Instant};

use base64::Engine;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_app_kit::{
    NSAttributedStringAppKitDocumentFormats, NSAttributedStringDocumentFormats,
    NSBitmapImageFileType, NSBitmapImageRep, NSCharacterEncodingDocumentAttribute,
    NSDocumentTypeDocumentAttribute, NSHTMLTextDocumentType, NSPasteboard, NSPasteboardItem,
    NSPasteboardType, NSPasteboardTypeHTML, NSPasteboardTypePNG, NSPasteboardTypeRTF,
    NSPasteboardTypeString, NSPasteboardTypeTIFF, NSPasteboardWriting, NSRunningApplication,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
};
use objc2_foundation::{NSArray, NSAttributedString, NSData, NSDictionary, NSNumber, NSRange};

use super::{Capture, CaptureSource};

const COPY_TIMEOUT: Duration = Duration::from_millis(400);
/// Some apps bump changeCount on clearContents and write their flavors a moment later.
const WRITE_SETTLE: Duration = Duration::from_millis(30);
const KEY_C: u16 = 8;
const NS_UTF8_STRING_ENCODING: usize = 4;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    #[cfg(not(debug_assertions))]
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
}

/// Shows the system prompt that leads the user to grant Accessibility, when it is not granted yet.
/// Dev builds skip it so test runs raise no system dialog; grant Accessibility to them by hand.
#[cfg(not(debug_assertions))]
pub fn prompt_for_accessibility_if_needed() {
    use objc2_foundation::NSString;
    let key = NSString::from_str("AXTrustedCheckOptionPrompt");
    let value = NSNumber::numberWithBool(true);
    let options = NSDictionary::<NSString, NSNumber>::from_slices(&[&*key], &[&*value]);
    unsafe { AXIsProcessTrustedWithOptions(Retained::as_ptr(&options).cast()) };
}

pub fn own_app_is_frontmost() -> bool {
    NSRunningApplication::currentApplication().isActive()
}

pub fn capture() -> Capture {
    let pasteboard = NSPasteboard::generalPasteboard();
    // Without Accessibility the synthetic copy is silently dropped, so waiting for it is pointless.
    if !unsafe { AXIsProcessTrusted() } {
        return read(&pasteboard, CaptureSource::Clipboard);
    }

    let snapshot = Snapshot::take(&pasteboard);
    let before = pasteboard.changeCount();
    post_copy();
    let deadline = Instant::now() + COPY_TIMEOUT;
    while pasteboard.changeCount() == before && Instant::now() < deadline {
        sleep(Duration::from_millis(10));
    }
    if pasteboard.changeCount() == before {
        return read(&pasteboard, CaptureSource::Clipboard);
    }

    sleep(WRITE_SETTLE);
    let captured = read(&pasteboard, CaptureSource::Selection);
    snapshot.restore(&pasteboard);
    captured
}

fn post_copy() {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    for key_down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.as_deref(), KEY_C, key_down);
        // The user is still holding the shortcut's modifiers; only Command may reach the target app.
        CGEvent::set_flags(event.as_deref(), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
    }
}

fn read(pasteboard: &NSPasteboard, source: CaptureSource) -> Capture {
    let string = |ty: &NSPasteboardType| pasteboard.stringForType(ty).map(|s| s.to_string());
    let data = |ty: &NSPasteboardType| pasteboard.dataForType(ty);

    let html = unsafe { string(NSPasteboardTypeHTML) }
        .or_else(|| unsafe { data(NSPasteboardTypeRTF) }.and_then(|rtf| rtf_to_html(&rtf)));
    let image = unsafe { data(NSPasteboardTypePNG) }
        .or_else(|| unsafe { data(NSPasteboardTypeTIFF) }.and_then(|tiff| tiff_to_png(&tiff)));

    Capture {
        html: html.filter(|h| !h.trim().is_empty()),
        text: unsafe { string(NSPasteboardTypeString) }.filter(|t| !t.trim().is_empty()),
        image_png: image.map(|png| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png.to_vec())
            )
        }),
        source,
    }
}

fn rtf_to_html(rtf: &NSData) -> Option<String> {
    unsafe {
        let attributed = NSAttributedString::initWithRTF_documentAttributes(
            NSAttributedString::alloc(),
            rtf,
            None,
        )?;
        let keys = [
            NSDocumentTypeDocumentAttribute,
            NSCharacterEncodingDocumentAttribute,
        ];
        let encoding = NSNumber::new_usize(NS_UTF8_STRING_ENCODING);
        let values: [&AnyObject; 2] = [NSHTMLTextDocumentType.as_ref(), encoding.as_ref()];
        let attributes = NSDictionary::from_slices(&keys, &values);
        let html = attributed
            .dataFromRange_documentAttributes_error(
                NSRange::new(0, attributed.length()),
                &attributes,
            )
            .ok()?;
        String::from_utf8(html.to_vec()).ok()
    }
}

fn tiff_to_png(tiff: &NSData) -> Option<Retained<NSData>> {
    let rep = NSBitmapImageRep::imageRepWithData(tiff)?;
    unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
}

/// Every item and every type on the pasteboard, so the user's clipboard survives the synthetic copy.
struct Snapshot(Vec<Vec<(Retained<NSPasteboardType>, Retained<NSData>)>>);

impl Snapshot {
    fn take(pasteboard: &NSPasteboard) -> Self {
        let items = pasteboard
            .pasteboardItems()
            .map(|a| a.to_vec())
            .unwrap_or_default();
        Snapshot(
            items
                .iter()
                .map(|item| {
                    item.types()
                        .to_vec()
                        .into_iter()
                        .filter_map(|ty| item.dataForType(&ty).map(|data| (ty, data)))
                        .collect()
                })
                .collect(),
        )
    }

    fn restore(self, pasteboard: &NSPasteboard) {
        pasteboard.clearContents();
        let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = self
            .0
            .iter()
            .map(|entries| {
                let item = NSPasteboardItem::new();
                for (ty, data) in entries {
                    item.setData_forType(data, ty);
                }
                ProtocolObject::from_retained(item)
            })
            .collect();
        if !items.is_empty() {
            pasteboard.writeObjects(&NSArray::from_retained_slice(&items));
        }
    }
}
