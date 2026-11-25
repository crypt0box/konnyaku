import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Capture, Settings, TranslateEvent } from "./domain/types";

export const getSettings = () => invoke<Settings>("get_settings");

export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });

export function translate(systemPrompt: string, content: string, onEvent: (event: TranslateEvent) => void) {
  const channel = new Channel<TranslateEvent>();
  channel.onmessage = onEvent;
  return invoke<void>("translate", { systemPrompt, content, onEvent: channel });
}

export const dismiss = () => invoke<void>("dismiss");

export const debugDumpRendered = (html: string) => invoke<void>("debug_dump_rendered", { html });

export const onCaptured = (handler: (capture: Capture) => void): Promise<UnlistenFn> =>
  listen<Capture>("captured", (event) => handler(event.payload));

export const onCancelled = (handler: () => void): Promise<UnlistenFn> => listen("cancelled", handler);

export const onSettingsChanged = (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>("settings-changed", (event) => handler(event.payload));
