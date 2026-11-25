//! Debug-build levers for scripts/e2e/gui-verify.sh. The app holds the Accessibility grant, so it
//! posts the synthetic input that a test shell cannot. Commands arrive as args forwarded by
//! tauri-plugin-single-instance.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::Duration;

use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventType, CGMouseButton,
};
use objc2_foundation::NSPoint;
use tauri::{AppHandle, Manager};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

enum Command {
    Keys(Combo, usize),
    Click(NSPoint),
    Drag(NSPoint, NSPoint),
    Eval {
        label: String,
        out: PathBuf,
        body: String,
    },
    Ax {
        out: PathBuf,
    },
}

struct Combo {
    flags: CGEventFlags,
    key: u16,
}

/// Runs a `--debug-*` command. Returns false when `args` carry none.
pub fn handle(app: &AppHandle, args: &[String]) -> bool {
    let Some(position) = args.iter().position(|a| a.starts_with("--debug-")) else {
        return false;
    };
    match parse(&args[position..]) {
        Ok(Command::Keys(combo, times)) => post_keys(&combo, times),
        Ok(Command::Click(at)) => post_click(at),
        Ok(Command::Drag(from, to)) => post_drag(from, to),
        Ok(Command::Eval { label, out, body }) => eval(app, &label, out, &body),
        Ok(Command::Ax { out }) => write(
            &out,
            &format!(r#"{{"trusted":{}}}"#, unsafe { AXIsProcessTrusted() }),
        ),
        Err(error) => eprintln!("konnyaku debug: {error}: {args:?}"),
    }
    true
}

fn parse(args: &[String]) -> Result<Command, String> {
    let arg = |i: usize| args.get(i).cloned().ok_or("missing argument");
    let number = |i: usize| -> Result<f64, String> {
        arg(i)?.parse().map_err(|e| format!("not a number: {e}"))
    };
    match args[0].as_str() {
        "--debug-keys" => {
            let times = match args.get(2) {
                Some(times) => times.parse().map_err(|e| format!("not a count: {e}"))?,
                None => 1,
            };
            Ok(Command::Keys(parse_combo(&arg(1)?)?, times))
        }
        "--debug-click" => Ok(Command::Click(NSPoint::new(number(1)?, number(2)?))),
        "--debug-drag" => Ok(Command::Drag(
            NSPoint::new(number(1)?, number(2)?),
            NSPoint::new(number(3)?, number(4)?),
        )),
        "--debug-eval" => Ok(Command::Eval {
            label: arg(1)?,
            out: arg(2)?.into(),
            body: arg(3)?,
        }),
        "--debug-ax" => Ok(Command::Ax {
            out: arg(1)?.into(),
        }),
        other => Err(format!("unknown command {other}")),
    }
}

/// "ctrl+shift+k", "cmd+comma", "escape".
fn parse_combo(spec: &str) -> Result<Combo, String> {
    let mut flags = CGEventFlags::empty();
    let mut key = None;
    for part in spec.split('+') {
        match part {
            "ctrl" => flags |= CGEventFlags::MaskControl,
            "shift" => flags |= CGEventFlags::MaskShift,
            "alt" => flags |= CGEventFlags::MaskAlternate,
            "cmd" => flags |= CGEventFlags::MaskCommand,
            name => key = Some(key_code(name).ok_or(format!("unknown key {name}"))?),
        }
    }
    Ok(Combo {
        flags,
        key: key.ok_or("no key")?,
    })
}

/// ANSI virtual key codes (HIToolbox Events.h); positional, so they hold on JIS keyboards too.
fn key_code(name: &str) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7,
        16, 6,
    ];
    Some(match name {
        "escape" => 53,
        "comma" => 43,
        "return" => 36,
        "tab" => 48,
        letter if letter.len() == 1 && letter.as_bytes()[0].is_ascii_lowercase() => {
            LETTERS[(letter.as_bytes()[0] - b'a') as usize]
        }
        _ => return None,
    })
}

/// Repeated presses go out back to back, so the next press lands while the previous one is still being handled.
fn post_keys(combo: &Combo, times: usize) {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let gap = Duration::from_millis(if times == 1 { 30 } else { 1 });
    for key_down in std::iter::repeat_n([true, false], times).flatten() {
        let event = CGEvent::new_keyboard_event(source.as_deref(), combo.key, key_down);
        CGEvent::set_flags(event.as_deref(), combo.flags);
        CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
        sleep(gap);
    }
}

fn post_mouse(kind: CGEventType, at: NSPoint) {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let event = CGEvent::new_mouse_event(source.as_deref(), kind, at, CGMouseButton::Left);
    CGEvent::set_integer_value_field(event.as_deref(), CGEventField::MouseEventClickState, 1);
    CGEvent::post(CGEventTapLocation::HIDEventTap, event.as_deref());
    sleep(Duration::from_millis(30));
}

/// Puts the pointer back where the user left it.
fn with_pointer_restored(post: impl FnOnce()) {
    let home = CGEvent::location(CGEvent::new(None).as_deref());
    post();
    post_mouse(CGEventType::MouseMoved, home);
}

fn post_click(at: NSPoint) {
    with_pointer_restored(|| {
        post_mouse(CGEventType::MouseMoved, at);
        post_mouse(CGEventType::LeftMouseDown, at);
        post_mouse(CGEventType::LeftMouseUp, at);
    });
}

fn post_drag(from: NSPoint, to: NSPoint) {
    const STEPS: u32 = 12;
    with_pointer_restored(|| {
        post_mouse(CGEventType::MouseMoved, from);
        post_mouse(CGEventType::LeftMouseDown, from);
        for step in 1..=STEPS {
            let t = f64::from(step) / f64::from(STEPS);
            let at = NSPoint::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
            post_mouse(CGEventType::LeftMouseDragged, at);
        }
        post_mouse(CGEventType::LeftMouseUp, to);
    });
}

static PENDING_EVALS: Mutex<Option<HashMap<u64, PathBuf>>> = Mutex::new(None);
static NEXT_EVAL: AtomicU64 = AtomicU64::new(0);

/// Runs `body` as an async function in the webview and writes `{"ok":true,"value":…}` or
/// `{"ok":false,"error":…}` to `out` once it settles.
fn eval(app: &AppHandle, label: &str, out: PathBuf, body: &str) {
    let Some(webview) = app.get_webview_window(label) else {
        return write(&out, r#"{"ok":false,"error":"no such webview"}"#);
    };
    let id = NEXT_EVAL.fetch_add(1, Ordering::Relaxed);
    PENDING_EVALS
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(id, out.clone());
    let script = format!(
        "(async () => {{ let r; try {{ r = {{ ok: true, value: await (async () => {{ {body}\n }})() }}; }} \
         catch (e) {{ r = {{ ok: false, error: String(e) }}; }} \
         await window.__TAURI_INTERNALS__.invoke('debug_eval_result', {{ id: {id}, json: JSON.stringify(r) }}); }})()"
    );
    if let Err(error) = webview.eval(script) {
        write(&out, &format!(r#"{{"ok":false,"error":"{error}"}}"#));
    }
}

#[tauri::command]
pub fn debug_eval_result(id: u64, json: String) {
    let out = PENDING_EVALS
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|pending| pending.remove(&id));
    if let Some(out) = out {
        write(&out, &json);
    }
}

fn write(out: &PathBuf, contents: &str) {
    if let Err(error) = std::fs::write(out, contents) {
        eprintln!("konnyaku debug: cannot write {}: {error}", out.display());
    }
}
