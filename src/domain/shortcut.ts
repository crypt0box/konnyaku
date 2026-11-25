export type KeyPress = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

const MODIFIER_CODES = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/;

/** The global-shortcut accelerator for a key press, or null while it is not a usable shortcut yet. */
export function shortcutFromKeys(keys: KeyPress): string | null {
  // Shift alone is how capitals and symbols are typed, so a Shift-only shortcut would hijack typing.
  if (MODIFIER_CODES.test(keys.code) || !(keys.ctrlKey || keys.altKey || keys.metaKey)) return null;
  const modifiers = [
    keys.ctrlKey && "Control",
    keys.altKey && "Alt",
    keys.shiftKey && "Shift",
    keys.metaKey && "Command",
  ].filter(Boolean);
  const key = keys.code.replace(/^Key/, "").replace(/^Digit/, "");
  return [...modifiers, key].join("+");
}
