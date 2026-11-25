import { useEffect, useState, type KeyboardEvent } from "react";
import { shortcutFromKeys } from "../../domain/shortcut";
import type { Effort, Model, Settings, Style } from "../../domain/types";
import { getSettings, saveSettings } from "../../ipc";

const MODELS: Model[] = ["haiku", "sonnet", "opus"];
const EFFORTS: Effort[] = ["low", "medium", "high", "xhigh", "max"];
const STYLES: Record<Style, string> = {
  followSource: "原文に合わせる",
  desuMasu: "です・ます調",
  daDearu: "だ・である調",
};

type Status = { kind: "idle" } | { kind: "saved" } | { kind: "error"; message: string };

export function SettingsWindow() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<Status>({ kind: "idle" });

  useEffect(() => {
    void getSettings().then(setSettings);
  }, []);

  if (settings === null) return null;

  const update = (patch: Partial<Settings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    saveSettings(next).then(
      () => setStatus({ kind: "saved" }),
      (error: unknown) => {
        setStatus({ kind: "error", message: String(error) });
        void getSettings().then(setSettings);
      },
    );
  };

  return (
    <main className="settings">
      <label>
        <span>ショートカット</span>
        <input
          readOnly
          value={settings.shortcut}
          onKeyDown={(event) => {
            const shortcut = recordShortcut(event);
            if (shortcut) update({ shortcut });
          }}
        />
        <small>欄を選んで、使いたいキーの組み合わせを押してください。</small>
      </label>
      <label>
        <span>モデル</span>
        <select value={settings.model} onChange={(e) => update({ model: e.target.value as Model })}>
          {MODELS.map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
      </label>
      <label>
        <span>思考の深さ (effort)</span>
        <select value={settings.effort} onChange={(e) => update({ effort: e.target.value as Effort })}>
          {EFFORTS.map((e) => (
            <option key={e}>{e}</option>
          ))}
        </select>
      </label>
      <label>
        <span>文体</span>
        <select value={settings.style} onChange={(e) => update({ style: e.target.value as Style })}>
          {Object.entries(STYLES).map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>claude のパス</span>
        <input
          placeholder="自動で探す"
          defaultValue={settings.claudePath ?? ""}
          onBlur={(e) => update({ claudePath: e.target.value.trim() || null })}
        />
        <small>見つからないときだけ入力してください（例: /Users/you/.local/bin/claude）。</small>
      </label>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={settings.launchAtLogin}
          onChange={(e) => update({ launchAtLogin: e.target.checked })}
        />
        <span>ログイン時に起動</span>
      </label>
      <p className={`status ${status.kind}`} role="status">
        {status.kind === "saved" ? "保存しました" : status.kind === "error" ? status.message : ""}
      </p>
    </main>
  );
}

function recordShortcut(event: KeyboardEvent): string | null {
  if (event.key === "Tab") return null;
  event.preventDefault();
  return shortcutFromKeys(event);
}
