/** Whether the app runs on macOS, where ⌘ takes the place of Ctrl in shortcuts. */
export const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** The name of the shortcut modifier, as keyboard hints show it. */
export const MOD = IS_MAC ? "⌘" : "Ctrl";

/** Whether the event holds the shortcut modifier (⌘ on macOS, Ctrl elsewhere) and no other. */
export function withMod(event: KeyboardEvent): boolean {
  return (IS_MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey) && !event.altKey && !event.shiftKey;
}
