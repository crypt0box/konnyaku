mod capture;
mod settings;
mod translator;

use std::sync::Mutex;

use tauri::ipc::Channel;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_window_state::StateFlags;

use settings::{Loaded, Settings};
use translator::{Request, TranslateEvent, Translator};

const TRANSLATION_WINDOW: &str = "main";
const SETTINGS_WINDOW: &str = "settings";
const AUTOSTART_ARG: &str = "--autostarted";
#[cfg(debug_assertions)]
const TRANSLATE_NOW_ARG: &str = "--translate-now";

struct AppState {
    settings: Mutex<Settings>,
    translator: Translator,
}

/// キャプチャ, then show the 翻訳ウィンドウ. Capture must run before the window takes focus,
/// because the synthetic copy goes to the frontmost app.
fn capture_and_show(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        app.state::<AppState>().translator.cancel();
        let captured = capture::capture();
        let _ = app.emit_to(TRANSLATION_WINDOW, "captured", captured);
        show_translation_window(&app);
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
        .build()?;
    Ok(())
}

fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts.unregister_all().map_err(|e| e.to_string())?;
    shortcuts.register(shortcut).map_err(|e| e.to_string())
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
    if settings.shortcut != previous.shortcut
        && let Err(error) = register_shortcut(&app, &settings.shortcut)
    {
        let _ = register_shortcut(&app, &previous.shortcut);
        return Err(format!(
            "ショートカット「{}」を登録できません: {error}",
            settings.shortcut
        ));
    }
    if settings.launch_at_login != previous.launch_at_login {
        let autolaunch = app.autolaunch();
        let result = if settings.launch_at_login {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        result.map_err(|e| e.to_string())?;
    }
    settings::save(&app, &settings).map_err(|e| e.to_string())?;
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

#[tauri::command]
fn debug_dump_rendered(html: String) {
    #[cfg(debug_assertions)]
    {
        let path = std::env::var_os("KONNYAKU_DUMP")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("konnyaku-rendered.html"));
        match std::fs::write(&path, html) {
            Ok(()) => eprintln!(
                "konnyaku: dumped rendered translation to {}",
                path.display()
            ),
            Err(error) => eprintln!("konnyaku: dump failed: {error}"),
        }
    }
    #[cfg(not(debug_assertions))]
    let _ = html;
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
        &[
            &PredefinedMenuItem::copy(app, None)?,
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
    let settings = match settings::load(handle)? {
        Loaded::Saved(settings) => settings,
        Loaded::FirstRun(settings) => {
            // A dev build must not register itself as a login item on the developer's machine.
            #[cfg(not(debug_assertions))]
            if settings.launch_at_login {
                let _ = handle.autolaunch().enable();
            }
            settings::save(handle, &settings)?;
            settings
        }
    };
    if let Err(error) = register_shortcut(handle, &settings.shortcut) {
        eprintln!(
            "konnyaku: could not register shortcut {}: {error}",
            settings.shortcut
        );
    }
    translator::warm_up();
    app.manage(AppState {
        settings: Mutex::new(settings),
        translator: Translator::default(),
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
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            #[cfg(debug_assertions)]
            if args.iter().any(|arg| arg == TRANSLATE_NOW_ARG) {
                capture_and_show(app);
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
            debug_dump_rendered
        ])
        .build(tauri::generate_context!())
        .expect("error while building konnyaku");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            show_translation_window(app);
        }
        let _ = (app, event);
    });
}
