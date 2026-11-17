import { IS_MAC, MOD } from "./platform";

/**
 * The keyboard map: every shortcut of the main window, in one place. Each part of the
 * interface that answers a shortcut asks `shortcutOf` what an event means, and the command
 * palette shows the same keys with `hintOf`.
 */
export type Shortcut =
  | { kind: "palette" }
  | { kind: "focusSearch" }
  | { kind: "back" }
  | { kind: "forward" }
  | { kind: "save" }
  | { kind: "pronounce" }
  /** The nth Dictionary on the page, from 0. */
  | { kind: "jump"; index: number }
  | { kind: "zoomIn" }
  | { kind: "zoomOut" }
  | { kind: "zoomReset" };

/** What a key press is made of: the fields of a keyboard event the map reads. */
export interface Keys {
  key: string;
  code: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

/** The shortcut modifier (⌘ on macOS, Ctrl elsewhere) and no other modifier. */
function modOnly(keys: Keys): boolean {
  return (IS_MAC ? keys.metaKey && !keys.ctrlKey : keys.ctrlKey && !keys.metaKey) && !keys.altKey;
}

/** Alt (Option on macOS) and no other modifier. */
function altOnly(keys: Keys): boolean {
  return keys.altKey && !keys.ctrlKey && !keys.metaKey && !keys.shiftKey;
}

/**
 * The shortcut `keys` press, if any. `inTextField`: the focus is where the reader types,
 * so a bare `/` is typed, not a shortcut.
 */
export function shortcutOf(keys: Keys, inTextField = false): Shortcut | null {
  const key = keys.key.length === 1 ? keys.key.toLowerCase() : keys.key;
  if (altOnly(keys)) {
    if (key === "ArrowLeft") return { kind: "back" };
    if (key === "ArrowRight") return { kind: "forward" };
    if (keys.code === "KeyP" || key === "p") return { kind: "pronounce" };
    // By code first: Alt (Option) with a digit types another character on some layouts.
    const digit = /^(?:Digit|Numpad)([1-9])$/.exec(keys.code)?.[1] ?? /^[1-9]$/.exec(key)?.[0];
    if (digit) return { kind: "jump", index: Number(digit) - 1 };
    return null;
  }
  // Zoom takes ⌘/Ctrl with or without Shift: "+" is Shift+= on many layouts.
  if ((keys.ctrlKey || keys.metaKey) && !keys.altKey) {
    if (key === "=" || key === "+" || keys.code === "NumpadAdd") return { kind: "zoomIn" };
    if (key === "-" || keys.code === "NumpadSubtract") return { kind: "zoomOut" };
    if (key === "0" || keys.code === "Numpad0") return { kind: "zoomReset" };
  }
  if (modOnly(keys) && !keys.shiftKey) {
    if (key === "k") return { kind: "palette" };
    if (key === "l") return { kind: "focusSearch" };
    if (key === "d") return { kind: "save" };
    return null;
  }
  if (key === "/" && !keys.altKey && !keys.ctrlKey && !keys.metaKey && !inTextField) return { kind: "focusSearch" };
  return null;
}

/** Whether `target` takes typing: a text field, a list box or editable content. */
export function isTextField(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  return target instanceof HTMLInputElement && !["button", "checkbox", "radio", "range", "reset", "submit", "color", "file", "image"].includes(target.type);
}

/** The shortcut a keyboard event presses, if any; none while an IME is composing. */
export function shortcutOfEvent(event: KeyboardEvent): Shortcut | null {
  if (event.isComposing || event.keyCode === 229) return null;
  return shortcutOf(event, isTextField(event.target));
}

/** The keys of a shortcut as the interface shows them ("Ctrl K", "Alt ←"). */
export function hintOf(shortcut: Shortcut): string {
  switch (shortcut.kind) {
    case "palette":
      return `${MOD} K`;
    case "focusSearch":
      return "/";
    case "back":
      return "Alt ←";
    case "forward":
      return "Alt →";
    case "save":
      return `${MOD} D`;
    case "pronounce":
      return "Alt P";
    case "jump":
      return `Alt ${shortcut.index + 1}`;
    case "zoomIn":
      return `${MOD} =`;
    case "zoomOut":
      return `${MOD} −`;
    case "zoomReset":
      return `${MOD} 0`;
  }
}

/**
 * Presses a shortcut's keys on `target`, for actions owned by the part of the interface
 * that answers those keys (the thumb index jumps on Alt + digit), and for keys pressed
 * inside an Entry frame, which the app's own document never hears.
 */
export function press(keys: Keys, target: EventTarget = window): void {
  target.dispatchEvent(new KeyboardEvent("keydown", { ...keys, bubbles: true, cancelable: true }));
}

/** The keys that press `shortcut` (the first, where there are several). */
export function keysOf(shortcut: Extract<Shortcut, { kind: "jump" }>): Keys {
  const digit = String(shortcut.index + 1);
  return { key: digit, code: `Digit${digit}`, altKey: true, ctrlKey: false, metaKey: false, shiftKey: false };
}

/**
 * A key press an Entry frame reported, as the app accepts it: exactly the fields of
 * `Keys` with the right types and plausible values, pressing one of the app's shortcuts.
 * Anything else is null.
 */
export function keysFromFrame(message: Record<string, unknown>): Keys | null {
  const { key, code, altKey, ctrlKey, metaKey, shiftKey } = message;
  if (typeof key !== "string" || key.length === 0 || key.length > 16) return null;
  if (typeof code !== "string" || !/^[A-Za-z0-9]{0,24}$/.test(code)) return null;
  if (typeof altKey !== "boolean" || typeof ctrlKey !== "boolean" || typeof metaKey !== "boolean" || typeof shiftKey !== "boolean") return null;
  const keys = { key, code, altKey, ctrlKey, metaKey, shiftKey };
  // The frame reports `/` only where the reader was not typing.
  return shortcutOf(keys, false) ? keys : null;
}
