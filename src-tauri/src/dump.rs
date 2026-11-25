//! Debug builds started with KONNYAKU_DUMP=<path> write each finished 訳文's rendered HTML there, so
//! scripts/e2e can read what the 翻訳ウィンドウ shows.

use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

const PATH_VAR: &str = "KONNYAKU_DUMP";

/// Tells the frontend to send its rendered HTML, only when a dump path is set.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let builder = Builder::new("konnyaku-dump");
    if std::env::var_os(PATH_VAR).is_some() {
        builder.js_init_script("window.__KONNYAKU_DUMP__ = true;")
    } else {
        builder
    }
    .build()
}

#[tauri::command]
pub fn debug_dump_rendered(html: String) {
    let Some(path) = std::env::var_os(PATH_VAR) else {
        return;
    };
    match std::fs::write(&path, html) {
        Ok(()) => eprintln!("konnyaku: dumped rendered translation to {}", path.display()),
        Err(error) => eprintln!("konnyaku: dump failed: {error}"),
    }
}
