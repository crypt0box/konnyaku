use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
#[serde(rename_all = "camelCase")]
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

pub fn load(app: &AppHandle) -> tauri::Result<Settings> {
    let store = app.store(STORE_FILE).map_err(into_tauri)?;
    Ok(store
        .get(STORE_KEY)
        .map(|value| from_stored(&value))
        .unwrap_or_default())
}

/// Each field falls back to its default on its own, so one unreadable field does not reset the rest.
fn from_stored(value: &Value) -> Settings {
    let defaults = Settings::default();
    Settings {
        shortcut: field(value, "shortcut", defaults.shortcut),
        model: field(value, "model", defaults.model),
        effort: field(value, "effort", defaults.effort),
        style: field(value, "style", defaults.style),
        claude_path: field(value, "claudePath", defaults.claude_path),
        launch_at_login: field(value, "launchAtLogin", defaults.launch_at_login),
    }
}

fn field<T: DeserializeOwned>(value: &Value, name: &str, default: T) -> T {
    let Some(stored) = value.get(name) else {
        return default;
    };
    serde_json::from_value(stored.clone()).unwrap_or_else(|error| {
        eprintln!("konnyaku: ignoring stored setting {name} = {stored}: {error}");
        default
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn one_unreadable_field_keeps_the_others() {
        let stored = json!({
            "shortcut": "Alt+K",
            "model": "gpt-5",
            "effort": "high",
            "style": 3,
            "claudePath": "/opt/claude",
            "launchAtLogin": false
        });
        assert_eq!(
            from_stored(&stored),
            Settings {
                shortcut: "Alt+K".into(),
                model: Model::Sonnet,
                effort: Effort::High,
                style: Style::FollowSource,
                claude_path: Some("/opt/claude".into()),
                launch_at_login: false,
            }
        );
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        assert_eq!(
            from_stored(&json!({ "model": "opus", "style": "daDearu" })),
            Settings {
                model: Model::Opus,
                style: Style::DaDearu,
                ..Settings::default()
            }
        );
        assert_eq!(from_stored(&json!("garbage")), Settings::default());
    }
}
