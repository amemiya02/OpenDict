import type { FormEvent, KeyboardEvent, SyntheticEvent } from "react";
import { useI18n } from "../i18n";
import { LIST_ID } from "./HeadwordColumn";

/** An IME is composing: its keys and its text are not the reader's yet. */
function composing(event: SyntheticEvent) {
  const native = event.nativeEvent as Partial<globalThis.KeyboardEvent & InputEvent>;
  return Boolean(native.isComposing) || native.keyCode === 229;
}

/**
 * The slip of India Paper set into the cover band. It drives the headword column: what
 * the reader types asks for suggestions, ↑ and ↓ move through them, and Enter commits.
 * Nothing happens while an IME is composing; the composed text counts once it is done.
 */
export function SearchField({
  value,
  activeOption,
  onChange,
  onQuery,
  onMove,
  onCommit,
}: {
  value: string;
  /** The id of the current suggestion, if any. */
  activeOption: string | undefined;
  /** The text changed (also while composing). */
  onChange: (value: string) => void;
  /** The reader's query changed (never mid-composition). */
  onQuery: (query: string) => void;
  onMove: (by: number) => void;
  onCommit: () => void;
}) {
  const { t } = useI18n();

  function submit(event: FormEvent) {
    event.preventDefault();
    onCommit();
  }

  function keyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (composing(event)) {
      // Enter that confirms an IME composition must not submit (WebKit reports it as a keydown).
      if (event.key === "Enter") event.preventDefault();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      onMove(event.key === "ArrowDown" ? 1 : -1);
    }
  }

  return (
    <form
      role="search"
      onSubmit={submit}
      className="flex h-8 w-[620px] min-w-0 shrink items-center gap-xs rounded-sharp bg-field pr-1.5 pl-0.75 text-field-text focus-within:outline focus-within:outline-offset-1 focus-within:outline-on-cover"
    >
      <button
        type="submit"
        aria-label={t.lookUp}
        className="grid size-6.5 place-items-center rounded-sharp text-field-text-secondary hover:bg-field-hover hover:text-field-text"
      >
        <svg className="size-4 fill-none stroke-current stroke-[1.5]" viewBox="0 0 16 16" aria-hidden="true" strokeLinecap="round">
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5 14 14" />
        </svg>
      </button>
      <input
        type="text"
        role="combobox"
        aria-label={t.searchLabel}
        aria-controls={LIST_ID}
        aria-expanded="true"
        aria-autocomplete="list"
        aria-activedescendant={activeOption}
        placeholder={t.searchPlaceholder}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
          if (!composing(event)) onQuery(event.target.value);
        }}
        onCompositionEnd={(event) => onQuery(event.currentTarget.value)}
        onKeyDown={keyDown}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        autoFocus
        className="h-full min-w-0 flex-1 bg-transparent px-1 font-serif text-search outline-0 placeholder:font-sans placeholder:text-[14px] placeholder:text-field-text-secondary"
      />
    </form>
  );
}
