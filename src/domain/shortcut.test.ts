import { describe, expect, it } from "vitest";
import { shortcutFromKeys, type KeyPress } from "./shortcut";

const press = (code: string, mods: Partial<Omit<KeyPress, "code">> = {}): KeyPress => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("shortcutFromKeys", () => {
  it.each([
    ["Control+J", press("KeyJ", { ctrlKey: true })],
    ["Control+Shift+K", press("KeyK", { ctrlKey: true, shiftKey: true })],
    ["Alt+Shift+1", press("Digit1", { altKey: true, shiftKey: true })],
    ["Command+Space", press("Space", { metaKey: true })],
  ])("records %s", (expected, keys) => {
    expect(shortcutFromKeys(keys)).toBe(expected);
  });

  it.each([
    ["a bare letter", press("KeyJ")],
    ["Shift with a letter, which is how capitals are typed", press("KeyJ", { shiftKey: true })],
    ["Shift with a digit, which is how symbols are typed", press("Digit1", { shiftKey: true })],
    ["a modifier alone", press("ControlLeft", { ctrlKey: true })],
  ])("ignores %s", (_, keys) => {
    expect(shortcutFromKeys(keys)).toBeNull();
  });
});
