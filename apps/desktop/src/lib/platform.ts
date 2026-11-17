/** Whether the app runs on macOS, where ⌘ takes the place of Ctrl in shortcuts. */
export const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** The name of the shortcut modifier, as keyboard hints show it. */
export const MOD = IS_MAC ? "⌘" : "Ctrl";

