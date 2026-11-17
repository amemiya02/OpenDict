import { useEffect, useId, useRef, useState, type KeyboardEvent, type SyntheticEvent } from "react";
import { useI18n } from "../i18n";
import { shortcutOf } from "../lib/keymap";

/** One action the palette offers. */
export interface Command {
  label: string;
  /** Its keyboard shortcut, as the interface shows it. */
  hint?: string;
  /** For a choice among several (Group, theme, language): whether it is the current one. */
  checked?: boolean;
  run: () => void;
}

/** An IME is composing: its keys and its text are not the reader's yet. */
function composing(event: SyntheticEvent) {
  const native = event.nativeEvent as Partial<globalThis.KeyboardEvent>;
  return Boolean(native.isComposing) || native.keyCode === 229;
}

/**
 * The command palette (DESIGN.md): a 560px sheet of India Paper over a dimmed window, the
 * input over a double rule and the commands as 30px rows, each with its shortcut at the
 * right. A vermilion check marks the current Group, theme and interface language.
 * Typing narrows the list, ↑ and ↓ move, Enter runs; Esc or a click outside closes. An IME
 * composition never runs a command. `commands` gives the actions as things stand at each
 * render. Mounted while open; `onClose` once it has closed (the focus is back where it
 * was), then the chosen command runs.
 */
export function CommandPalette({ commands, onClose }: { commands: () => Command[]; onClose: () => void }) {
  const { t } = useI18n();
  const dialog = useRef<HTMLDialogElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const ids = useId();
  const [needle, setNeedle] = useState("");
  const [selected, setSelected] = useState(0);
  /** The command to run once the palette has closed. */
  const chosen = useRef<Command | null>(null);

  const wanted = needle.trim().toLowerCase();
  const shown = commands().filter((command) => command.label.toLowerCase().includes(wanted));
  const current = Math.min(selected, Math.max(0, shown.length - 1));
  const optionId = (index: number) => `${ids}-${index}`;

  useEffect(() => {
    const element = dialog.current!;
    // Click outside closes where <dialog closedby> is not supported yet (WebKit).
    element.setAttribute("closedby", "any");
    let pressedOutside = false;
    const outside = (event: PointerEvent) => {
      const r = element.getBoundingClientRect();
      return event.clientX < r.left || event.clientX > r.right || event.clientY < r.top || event.clientY > r.bottom;
    };
    const down = (event: PointerEvent) => {
      pressedOutside = outside(event);
    };
    const up = (event: PointerEvent) => {
      if (pressedOutside && outside(event)) element.close();
    };
    if (!("closedBy" in HTMLDialogElement.prototype)) {
      element.addEventListener("pointerdown", down);
      element.addEventListener("pointerup", up);
    }
    const closed = () => {
      onClose();
      const command = chosen.current;
      chosen.current = null;
      command?.run();
    };
    element.addEventListener("close", closed);
    element.showModal();
    return () => {
      element.removeEventListener("pointerdown", down);
      element.removeEventListener("pointerup", up);
      element.removeEventListener("close", closed);
    };
    // Opened once per mount; `onClose` belongs to the mount that opened it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [current, needle]);

  function run(index: number) {
    const command = shown[index];
    if (!command) return;
    chosen.current = command;
    dialog.current?.close();
  }

  function keyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (composing(event)) {
      if (event.key === "Enter") event.preventDefault();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (shown.length === 0) return;
      const by = event.key === "ArrowDown" ? 1 : -1;
      setSelected((current + by + shown.length) % shown.length);
    } else if (event.key === "Enter") {
      event.preventDefault();
      run(current);
    }
  }

  return (
    <dialog
      ref={dialog}
      aria-label={t.commandPalette}
      // The window's shortcuts wait while the palette is open; zoom still works.
      onKeyDown={(event) => {
        const shortcut = shortcutOf(event.nativeEvent);
        if (!shortcut?.kind.startsWith("zoom")) event.stopPropagation();
      }}
      className="mx-auto mt-[11vh] mb-auto max-h-[min(520px,76vh)] w-[min(560px,calc(100vw-32px))] flex-col overflow-hidden rounded-sharp border border-rule bg-paper p-0 text-body text-text open:flex backdrop:bg-[rgb(8_10_14/.42)] motion-safe:open:animate-drop"
    >
      <div className="flex items-center gap-sm border-b-[3px] border-double border-rule pr-2.5 pl-lg">
        <input
          type="text"
          role="combobox"
          aria-label={t.typeCommand}
          aria-controls={`${ids}-list`}
          aria-expanded="true"
          aria-autocomplete="list"
          aria-activedescendant={shown.length > 0 ? optionId(current) : undefined}
          placeholder={t.typeCommand}
          value={needle}
          onChange={(event) => {
            setNeedle(event.target.value);
            setSelected(0);
          }}
          onKeyDown={keyDown}
          autoComplete="off"
          spellCheck={false}
          autoFocus
          className="h-[46px] min-w-0 flex-1 bg-transparent text-[15px] outline-0 placeholder:text-text-secondary"
        />
        <button
          type="button"
          aria-label={t.close}
          onClick={() => dialog.current?.close()}
          className="flex h-6 items-center rounded-sharp px-1 text-[12px] text-text-secondary hover:bg-shade hover:text-text"
        >
          <kbd className="rounded-sharp border border-hair px-[5px] font-sans text-[11px] leading-4">Esc</kbd>
        </button>
      </div>
      <ul
        ref={list}
        id={`${ids}-list`}
        role="listbox"
        aria-label={t.commandPalette}
        // Keep the focus in the input while a row is clicked.
        onMouseDown={(event) => event.preventDefault()}
        className="overflow-y-auto py-xs [scrollbar-color:var(--hairline)_transparent]"
      >
        {shown.length === 0 ? (
          <li className="flex min-h-[30px] items-center pr-3.5 pl-[34px] text-text-secondary">{t.noMatchingCommand}</li>
        ) : (
          shown.map((command, index) => (
            <li
              key={command.label}
              id={optionId(index)}
              role="option"
              aria-selected={index === current}
              aria-checked={command.checked}
              onClick={() => run(index)}
              className="flex min-h-[30px] items-center gap-sm py-[3px] pr-3.5 pl-2.5 hover:bg-[color-mix(in_srgb,var(--paper-shade)_55%,transparent)] aria-selected:bg-shade aria-selected:font-bold"
            >
              <span className="grid size-4 flex-none place-items-center text-here" aria-hidden="true">
                {command.checked ? (
                  <svg className="size-4 fill-none stroke-current stroke-[1.6]" viewBox="0 0 16 16">
                    <path d="M3.5 8.5l3 3 6-7" />
                  </svg>
                ) : null}
              </span>
              <span data-label="">{command.label}</span>
              {command.hint ? (
                <kbd className="ml-auto rounded-sharp border border-hair px-[5px] font-sans text-[11px] leading-4 font-normal whitespace-nowrap text-text-secondary">
                  {command.hint}
                </kbd>
              ) : null}
            </li>
          ))
        )}
      </ul>
    </dialog>
  );
}
