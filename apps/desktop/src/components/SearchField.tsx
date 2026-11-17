import { useState, type FormEvent, type KeyboardEvent } from "react";
import { useI18n } from "../i18n";

/** The slip of India Paper set into the cover band. Submitting runs a Lookup. */
export function SearchField({ onSubmit }: { onSubmit: (query: string) => void }) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");

  function submit(event: FormEvent) {
    event.preventDefault();
    const trimmed = query.trim();
    if (trimmed) onSubmit(trimmed);
  }

  // Enter that confirms an IME composition must not submit (WebKit reports it as a keydown).
  function guardComposition(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" && (event.nativeEvent.isComposing || event.keyCode === 229)) event.preventDefault();
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
        type="search"
        aria-label={t.searchLabel}
        placeholder={t.searchPlaceholder}
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={guardComposition}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        autoFocus
        className="h-full min-w-0 flex-1 bg-transparent px-1 font-serif text-search outline-0 placeholder:font-sans placeholder:text-[14px] placeholder:text-field-text-secondary [&::-webkit-search-cancel-button]:hidden"
      />
    </form>
  );
}
