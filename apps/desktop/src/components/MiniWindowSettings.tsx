import { useEffect, useState, type KeyboardEvent } from "react";
import { useI18n } from "../i18n";
import { getMiniSettings, setShortcut, setWatchClipboard, type MiniSettings, type ShortcutRole } from "../lib/ipc";
import { IS_MAC } from "../lib/platform";

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "OS", "AltGraph"]);

/**
 * The accelerator for a key press, modifiers first (`Control+Alt+KeyD`), by the key's
 * position (`event.code`) so it does not depend on the keyboard layout. Null for a lone
 * modifier, or a key with no modifier at all.
 */
export function acceleratorOf(event: Pick<globalThis.KeyboardEvent, "ctrlKey" | "altKey" | "shiftKey" | "metaKey" | "key" | "code">): string | null {
  if (MODIFIER_KEYS.has(event.key) || !event.code) return null;
  const parts = [event.ctrlKey && "Control", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Super"].filter(
    (part): part is string => Boolean(part),
  );
  if (parts.length === 0) return null;
  return [...parts, event.code].join("+");
}

const KEY_NAMES: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};

/** An accelerator as keyboard hints show it: `Ctrl Alt D` (`⌃ ⌥ D` on macOS). */
export function shortcutLabel(accelerator: string): string {
  return accelerator
    .split("+")
    .map((part) => {
      switch (part) {
        case "Control":
          return IS_MAC ? "⌃" : "Ctrl";
        case "Alt":
          return IS_MAC ? "⌥" : "Alt";
        case "Shift":
          return IS_MAC ? "⇧" : "Shift";
        case "Super":
          return IS_MAC ? "⌘" : "Win";
      }
      return KEY_NAMES[part] ?? part.replace(/^Key|^Digit/, "").replace(/^Numpad/, "Num ");
    })
    .join(" ");
}

/**
 * The Mini Window's settings, in the Library view: the two global shortcuts, each
 * changed by pressing the new one (or turned off), and clipboard watching (off by
 * default). Set like the rest of the page: a heading over rows divided by hairlines.
 */
export function MiniWindowSettings() {
  const { t } = useI18n();
  const [settings, setSettings] = useState<MiniSettings | null>(null);
  const [recording, setRecording] = useState<ShortcutRole | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    getMiniSettings().then(
      (loaded) => setSettings(loaded && typeof loaded === "object" ? loaded : null),
      () => {}, // a shell without the Mini Window: nothing to set
    );
  }, []);

  async function change(command: () => Promise<MiniSettings>) {
    setMessage(null);
    try {
      setSettings(await command());
    } catch (error) {
      setMessage(t.libraryActionFailed(String(error)));
    }
  }

  function keyDown(role: ShortcutRole, event: KeyboardEvent<HTMLButtonElement>) {
    if (recording !== role) return;
    if (event.key === "Tab") {
      setRecording(null);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      setRecording(null);
      setMessage(null);
      return;
    }
    if (MODIFIER_KEYS.has(event.key)) return; // the rest of the shortcut is still coming
    const accelerator = acceleratorOf(event.nativeEvent);
    if (accelerator === null) {
      setMessage(t.shortcutNeedsModifier);
      return;
    }
    setRecording(null);
    void change(() => setShortcut(role, accelerator));
  }

  if (!settings) return null;

  const rows: { role: ShortcutRole; name: string }[] = [
    { role: "lookUpSelection", name: t.lookUpSelectionShortcut },
    { role: "showMainWindow", name: t.showMainWindowShortcut },
  ];

  return (
    <section aria-labelledby="mini-window-settings" className="max-w-[860px] pt-7">
      <h2 id="mini-window-settings" className="border-b border-rule pb-1.5 text-[13px] font-bold">
        {t.miniWindow}
      </h2>
      <p className="py-sm text-text-secondary">{t.miniSettingsHow}</p>
      <ul>
        {rows.map(({ role, name }) => {
          const accelerator = settings[role];
          const taken = settings.unavailable.includes(role);
          return (
            <li key={role} className="flex min-h-9 flex-wrap items-center gap-x-md border-b border-hair py-1">
              <span className="min-w-[220px] flex-1">{name}</span>
              {taken ? <span className="text-label text-text-secondary">{t.shortcutUnavailable}</span> : null}
              <button
                type="button"
                aria-label={t.changeShortcut(name)}
                aria-pressed={recording === role}
                onClick={() => {
                  setMessage(null);
                  setRecording(recording === role ? null : role);
                }}
                onKeyDown={(event) => keyDown(role, event)}
                onBlur={() => setRecording((now) => (now === role ? null : now))}
                className="flex h-7 min-w-[120px] items-center justify-center rounded-sharp border border-rule px-2.5 font-sans text-body text-text tabular-nums hover:bg-shade aria-pressed:bg-shade"
              >
                {recording === role ? t.pressShortcut : accelerator ? shortcutLabel(accelerator) : t.shortcutOff}
              </button>
              <button
                type="button"
                aria-label={t.turnOffShortcut(name)}
                disabled={accelerator === null}
                onClick={() => void change(() => setShortcut(role, null))}
                className="h-7 rounded-sharp px-2 text-text-secondary hover:bg-shade hover:text-text disabled:opacity-40 disabled:hover:bg-transparent"
              >
                {t.shortcutOff}
              </button>
            </li>
          );
        })}
        <li className="flex min-h-9 items-center border-b border-hair py-1">
          <label className="flex items-center gap-sm">
            <input type="checkbox" checked={settings.watchClipboard} onChange={(event) => void change(() => setWatchClipboard(event.target.checked))} />
            {t.watchClipboard}
          </label>
        </li>
      </ul>
      <p role="status" className="pt-1.5 text-label text-text-secondary">
        {message ?? settings.problem}
      </p>
    </section>
  );
}
