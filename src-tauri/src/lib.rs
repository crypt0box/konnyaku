mod capture;
#[cfg(all(debug_assertions, target_os = "macos"))]
mod debug;
#[cfg(debug_assertions)]
mod dump;
mod settings;
mod translator;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::ipc::Channel;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_window_state::StateFlags;

use settings::Settings;
use translator::{Request, TranslateEvent, Translator};

const TRANSLATION_WINDOW: &str = "main";
const SETTINGS_WINDOW: &str = "settings";
const AUTOSTART_ARG: &str = "--autostarted";

struct AppState {
    settings: Mutex<Settings>,
    translator: Translator,
    capturing: AtomicBool,
}

/// キャプチャ, then show the 翻訳ウィンドウ. Capture must run before the window takes focus,
/// because the synthetic copy goes to the frontmost app.
fn capture_and_show(app: &AppHandle) {
    // With konnyaku frontmost, the copy would read its own window and replace the 訳文 with nothing.
    if capture::own_app_is_frontmost() {
        return show_translation_window(app);
    }
    // Two captures at once interleave snapshot, copy and restore on the one pasteboard, losing the user's
    // clipboard, so a press during a capture is dropped.
    if app.state::<AppState>().capturing.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        state.translator.cancel();
        let captured = capture::capture();
        let _ = app.emit_to(TRANSLATION_WINDOW, "captured", captured);
        // Off the main thread, show and set_focus only queue their work. The flag clears once they have run, so a
        // press from then on meets the frontmost check instead of copying from konnyaku's own window.
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            show_translation_window(&handle);
            handle.state::<AppState>().capturing.store(false, Ordering::SeqCst);
        });
    });
}

fn show_translation_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(TRANSLATION_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn open_settings_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        window.show()?;
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, SETTINGS_WINDOW, WebviewUrl::App("index.html".into()))
        .title("設定")
        .inner_size(460.0, 440.0)
        .resizable(false)
        // Like the 翻訳ウィンドウ in tauri.conf.json: a persistent WebKit store would keep the domains and bytes
        // of every image a 訳文 showed, and konnyaku keeps no history.
        .incognito(true)
        .build()?;
    Ok(())
}

/// Makes `shortcut` the only global shortcut. Safe to repeat: a shortcut already registered is left alone.
fn apply_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    if shortcuts.is_registered(shortcut) {
        return Ok(());
    }
    shortcuts
        .unregister_all()
        .and_then(|()| shortcuts.register(shortcut))
        .map_err(|e| format!("ショートカット「{shortcut}」を登録できません: {e}"))
}

/// Makes the login item match `enabled`. Safe to repeat.
fn apply_login_item(app: &AppHandle, enabled: bool) -> Result<(), String> {
    // Dev builds never register themselves as a login item on the developer's machine.
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().ok() == Some(enabled) {
        return Ok(());
    }
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|e| format!("ログイン時の起動を変更できません: {e}"))
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: tauri::State<AppState>,
    settings: Settings,
) -> Result<(), String> {
    let previous = state.settings.lock().unwrap().clone();
    let shortcut = apply_shortcut(&app, &settings.shortcut);
    // A shortcut that already failed before this save must not block an unrelated change such as the model.
    let shortcut = match shortcut {
        Err(error) if settings.shortcut == previous.shortcut => {
            eprintln!("konnyaku: {error}");
            Ok(())
        }
        result => result,
    };
    let applied = shortcut
        .and_then(|()| apply_login_item(&app, settings.launch_at_login))
        .and_then(|()| settings::save(&app, &settings).map_err(|e| e.to_string()));
    if let Err(error) = applied {
        let _ = apply_shortcut(&app, &previous.shortcut);
        let _ = apply_login_item(&app, previous.launch_at_login);
        return Err(error);
    }
    *state.settings.lock().unwrap() = settings.clone();
    let _ = app.emit("settings-changed", settings);
    Ok(())
}

#[tauri::command]
fn translate(
    state: tauri::State<AppState>,
    system_prompt: String,
    content: String,
    on_event: Channel<TranslateEvent>,
) {
    let settings = state.settings.lock().unwrap().clone();
    let request = Request {
        claude_path: settings.claude_path,
        model: settings.model,
        effort: settings.effort,
        system_prompt,
        content,
    };
    state.translator.start(request, move |event| {
        let _ = on_event.send(event);
    });
}

/// Esc in the 翻訳ウィンドウ: cancel and hide, then hand focus back to the app the 原文 came from.
#[tauri::command]
fn dismiss(app: AppHandle) {
    hide_translation_window(&app);
    #[cfg(target_os = "macos")]
    if !app
        .get_webview_window(SETTINGS_WINDOW)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
    {
        let _ = app.hide();
    }
}

fn hide_translation_window(app: &AppHandle) {
    app.state::<AppState>().translator.cancel();
    let _ = app.emit_to(TRANSLATION_WINDOW, "cancelled", ());
    if let Some(window) = app.get_webview_window(TRANSLATION_WINDOW) {
        let _ = window.hide();
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let settings = MenuItem::with_id(app, "settings", "設定…", true, Some("CmdOrCtrl+,"))?;
    let app_menu = Submenu::with_items(
        app,
        "konnyaku",
        true,
        &[
            &PredefinedMenuItem::about(app, None, Some(AboutMetadata::default()))?,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "編集",
        true,
        // WKWebView routes Cmd+Z/X/C/V/A through these items, so text fields need all of them.
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window_menu = Submenu::with_items(
        app,
        "ウインドウ",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();
    let settings = settings::load(handle)?;
    // On every launch, so the login item follows the setting even if an earlier run never applied it. Each is
    // applied even when the other fails.
    let applied = [
        apply_shortcut(handle, &settings.shortcut),
        apply_login_item(handle, settings.launch_at_login),
    ];
    for error in applied.into_iter().filter_map(Result::err) {
        eprintln!("konnyaku: {error}");
    }
    translator::warm_up();
    app.manage(AppState {
        settings: Mutex::new(settings),
        translator: Translator::default(),
        capturing: AtomicBool::new(false),
    });

    app.set_menu(build_menu(handle)?)?;
    app.on_menu_event(|app, event| {
        if event.id() == "settings" {
            let _ = open_settings_window(app);
        }
    });

    #[cfg(not(debug_assertions))]
    capture::prompt_for_accessibility_if_needed();

    if !std::env::args().any(|arg| arg == AUTOSTART_ARG) {
        show_translation_window(handle);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(debug_assertions)]
    let builder = builder.plugin(dump::plugin());
    let app = builder
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            #[cfg(all(debug_assertions, target_os = "macos"))]
            if debug::handle(app, &args) {
                return;
            }
            let _ = args;
            show_translation_window(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        capture_and_show(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]),
        ))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(setup)
        .on_window_event(|window, event| {
            if window.label() == TRANSLATION_WINDOW
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                hide_translation_window(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            translate,
            dismiss,
            #[cfg(debug_assertions)]
            dump::debug_dump_rendered,
            #[cfg(all(debug_assertions, target_os = "macos"))]
            debug::debug_eval_result
        ])
        .build(tauri::generate_context!())
        .expect("error while building konnyaku");

    app.run(|app, event| match event {
        // The process exits without dropping managed state, which would leave claude and its temp dir behind.
        RunEvent::Exit => app.state::<AppState>().translator.cancel(),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_translation_window(app),
        _ => {}
    });
}
