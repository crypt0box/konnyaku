use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Model {
    Haiku,
    Sonnet,
    Opus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Style {
    FollowSource,
    DesuMasu,
    DaDearu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub shortcut: String,
    pub model: Model,
    pub effort: Effort,
    pub style: Style,
    pub claude_path: Option<String>,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shortcut: "Control+J".into(),
            model: Model::Sonnet,
            effort: Effort::Low,
            style: Style::FollowSource,
            claude_path: None,
            launch_at_login: true,
        }
    }
}

impl Model {
    pub fn as_arg(self) -> &'static str {
        match self {
            Model::Haiku => "haiku",
            Model::Sonnet => "sonnet",
            Model::Opus => "opus",
        }
    }
}

impl Effort {
    pub fn as_arg(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

pub enum Loaded {
    FirstRun(Settings),
    Saved(Settings),
}

pub fn load(app: &AppHandle) -> tauri::Result<Loaded> {
    let store = app.store(STORE_FILE).map_err(into_tauri)?;
    Ok(match store.get(STORE_KEY) {
        None => Loaded::FirstRun(Settings::default()),
        Some(value) => Loaded::Saved(serde_json::from_value(value).unwrap_or_default()),
    })
}

pub fn save(app: &AppHandle, settings: &Settings) -> tauri::Result<()> {
    let store = app.store(STORE_FILE).map_err(into_tauri)?;
    store.set(STORE_KEY, serde_json::to_value(settings)?);
    store.save().map_err(into_tauri)
}

fn into_tauri(error: tauri_plugin_store::Error) -> tauri::Error {
    tauri::Error::Anyhow(error.into())
}
