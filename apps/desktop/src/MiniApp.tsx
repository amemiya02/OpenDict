import { useEffect, useEffectEvent, useRef, useState } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { SearchField } from "./components/SearchField";
import { sectionId, ThumbIndex } from "./components/ThumbIndex";
import { useI18n } from "./i18n";
import {
  closeMini,
  getWordBooks,
  lookUp,
  miniSummons,
  onSummoned,
  onWordBooksChanged,
  openInMain,
  suggest,
  toggleWord,
  type Entry,
  type Summons,
  type WordBooks,
} from "./lib/ipc";
import { MOD, withMod } from "./lib/platform";
import { playSound } from "./lib/sound";

type MiniPage =
  | { kind: "idle" }
  | { kind: "found"; headword: string; entries: Entry[]; lemma: { from: string; to: string } | null; near: boolean }
  | { kind: "missing"; query: string }
  | { kind: "failed"; reason: string };

/**
 * The Mini Window: its own cover band with the search field, the page in miniature, a
 * compact thumb index, and a footer with Add to Word Book, Open in main window and the
 * Esc hint. The shell summons it with the text the reader selected in another
 * application; it looks that up as the main window does (Lemmatization included). Esc
 * closes it and the shell gives focus back to where the reader was.
 */
export function MiniApp() {
  const { t } = useI18n();
  const [text, setText] = useState("");
  const [page, setPage] = useState<MiniPage>({ kind: "idle" });
  const [wordBooks, setWordBooks] = useState<WordBooks | null>(null);
  const requests = useRef(0);
  const field = useRef<HTMLInputElement>(null);
  const pageRef = useRef<HTMLElement>(null);
  const lastSerial = useRef(0);

  const currentBook = wordBooks?.books.find((book) => book.id === wordBooks.current) ?? null;
  const headword = page.kind === "found" ? page.headword : null;
  const saved = headword !== null && (currentBook?.words.some((word) => word.headword === headword) ?? false);

  /** Looks `query` up as the main window does: its first suggestion, with how it was found. */
  async function lookUpText(query: string) {
    const ticket = ++requests.current;
    const trimmed = query.trim();
    if (!trimmed) {
      setPage({ kind: "idle" });
      return;
    }
    try {
      const answer = await suggest(trimmed, 1);
      const first = answer.browse || answer.miss ? null : (answer.items[0]?.headword ?? null);
      const entries = first === null ? [] : await lookUp(first);
      if (ticket !== requests.current) return;
      pageRef.current?.scrollTo?.({ top: 0 });
      setPage(
        first !== null && entries.length > 0
          ? { kind: "found", headword: first, entries, lemma: answer.lemma, near: answer.near }
          : { kind: "missing", query: trimmed },
      );
    } catch (error) {
      if (ticket === requests.current) setPage({ kind: "failed", reason: String(error) });
    }
  }

  const summoned = useEffectEvent((summons: Summons) => {
    if (summons.serial <= lastSerial.current) return;
    lastSerial.current = summons.serial;
    setText(summons.text);
    void lookUpText(summons.text);
    // Ready to correct the word, or to type another over it.
    field.current?.focus({ preventScroll: true });
    field.current?.select();
  });

  useEffect(() => {
    const stops = [onSummoned((summons) => summoned(summons)), onWordBooksChanged((books) => setWordBooks(books))];
    // A summons sent before this window had loaded.
    miniSummons().then(
      (summons) => {
        if (summons) summoned(summons);
      },
      () => {},
    );
    getWordBooks().then((books) => setWordBooks(Array.isArray(books?.books) ? books : null), () => {});
    return () => {
      for (const stop of stops) stop.then((unlisten) => unlisten(), () => {});
    };
  }, []);

  function close() {
    closeMini().catch(() => {});
  }

  async function toggleSave() {
    if (page.kind !== "found") return;
    const from = page.entries[0]!;
    try {
      setWordBooks(await toggleWord(page.headword, from.dictionaryId, from.dictionaryTitle, t.newWordBookName));
    } catch {
      // The button stays as it was: nothing was saved.
    }
  }

  function openHere(word: string) {
    setText(word);
    void lookUpText(word);
  }

  const keys = useEffectEvent((event: KeyboardEvent) => {
    if (event.isComposing || event.keyCode === 229 || event.defaultPrevented) return;
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (withMod(event) && event.key.toLowerCase() === "d") {
      event.preventDefault();
      void toggleSave();
    }
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => keys(event);
    document.addEventListener("keydown", listener);
    return () => document.removeEventListener("keydown", listener);
  }, []);

  return (
    <div className="grid h-full grid-rows-[40px_minmax(0,1fr)_40px] bg-paper">
      <header className="flex items-center gap-xs bg-cover pr-1.5 pl-sm text-on-cover select-none" data-tauri-drag-region>
        <SearchField
          compact
          inputRef={field}
          value={text}
          activeOption={undefined}
          onChange={setText}
          onQuery={(query) => void lookUpText(query)}
          onMove={() => {}}
          onCommit={() => void lookUpText(text)}
        />
        <button
          type="button"
          aria-label={t.close}
          onClick={close}
          className="grid size-6.5 flex-none place-items-center rounded-sharp text-on-cover-muted hover:bg-cover-edge hover:text-on-cover"
        >
          <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
      </header>

      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_auto]">
        <main ref={pageRef} aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-2.5 pr-3.5 pb-lg pl-[18px]">
          <MiniPageContent page={page} onLookUp={openHere} onEscape={close} />
        </main>
        <div className="w-[30px] min-h-0 overflow-hidden">
          {page.kind === "found" ? (
            <ThumbIndex
              key={page.headword}
              compact
              notches={page.entries.map((e) => ({ dictionaryId: e.dictionaryId, title: e.dictionaryTitle }))}
              unanswered={0}
              page={pageRef}
            />
          ) : null}
        </div>
      </div>

      <footer className="flex items-center gap-1.5 border-t border-rule px-sm select-none">
        <button
          type="button"
          aria-pressed={saved}
          aria-keyshortcuts={MOD === "⌘" ? "Meta+D" : "Control+D"}
          disabled={page.kind !== "found"}
          onClick={() => void toggleSave()}
          className="flex h-6.5 flex-none items-center gap-1.5 rounded-sharp border border-rule px-sm text-[12px] text-text hover:bg-shade active:translate-y-px disabled:opacity-40 disabled:hover:bg-transparent aria-pressed:bg-text aria-pressed:text-paper"
        >
          <svg className="size-3.5 fill-none stroke-current stroke-[1.3]" viewBox="0 0 14 14" aria-hidden="true">
            <path d="M3.5 1.5h7v11L7 9.8l-3.5 2.7z" fill={saved ? "currentColor" : "none"} />
          </svg>
          <span>{saved ? t.inWordBook : t.addToWordBook}</span>
        </button>
        <button
          type="button"
          disabled={page.kind === "idle" && !text.trim()}
          onClick={() => openInMain(headword ?? text.trim()).catch(() => {})}
          className="flex h-6.5 flex-none items-center rounded-sharp border border-rule px-sm text-[12px] text-text hover:bg-shade active:translate-y-px disabled:opacity-40 disabled:hover:bg-transparent"
        >
          {t.openInMain}
        </button>
        <span className="ml-auto flex items-center gap-1 text-label text-text-secondary">
          <kbd className="rounded-sharp border border-hair px-1 font-sans text-[11px]">Esc</kbd>
          <span>{t.close}</span>
        </span>
      </footer>
    </div>
  );
}

function MiniPageContent({ page, onLookUp, onEscape }: { page: MiniPage; onLookUp: (headword: string) => void; onEscape: () => void }) {
  const { t } = useI18n();
  switch (page.kind) {
    case "idle":
      return null;
    case "failed":
      return <p className="py-md text-[13px]">{t.lookupFailed(page.reason)}</p>;
    case "missing":
      return (
        <article>
          <MiniHead headword={page.query} />
          <p className="py-md text-[13px]">{t.miniNone(page.query)}</p>
        </article>
      );
    case "found": {
      const pronunciation = page.entries.find((entry) => entry.pronunciation)?.pronunciation;
      return (
        <article>
          <MiniHead headword={page.headword} pronunciation={pronunciation} />
          {page.lemma ? (
            <p className="pt-1.5 text-[12px] text-text-secondary">
              {t.lemmatized} <b className="text-text">{page.lemma.from}</b> → <b className="text-text">{page.lemma.to}</b>
            </p>
          ) : page.near ? (
            <p className="pt-1.5 text-[12px] text-text-secondary">{t.nearNote}</p>
          ) : null}
          {page.entries.map((entry) => (
            <section
              key={entry.dictionaryId}
              id={sectionId(entry.dictionaryId)}
              tabIndex={-1}
              aria-label={entry.dictionaryTitle}
              className="scroll-mt-1.5 border-rule outline-offset-2 [&+&]:border-t"
            >
              <h2 className="-ml-[7px] flex h-[30px] items-center truncate text-title font-bold">【{entry.dictionaryTitle}】</h2>
              <div className="pb-2.5">
                <EntryFrame dictionaryTitle={entry.dictionaryTitle} src={entry.src} onLink={onLookUp} onSound={playSound} onEscape={onEscape} />
              </div>
            </section>
          ))}
        </article>
      );
    }
  }
}

/** The headword over a double rule, 24px (DESIGN.md: Headline). */
function MiniHead({ headword, pronunciation }: { headword: string; pronunciation?: string | null }) {
  return (
    <div className="flex flex-wrap items-baseline gap-x-2.5 border-b-[3px] border-double border-rule pb-1.5">
      <h1 className="-ml-[9px] font-serif text-headline font-bold [overflow-wrap:anywhere]" lang={/[぀-ヿ]/.test(headword) ? "ja" : undefined}>
        【{headword}】
      </h1>
      {pronunciation ? <span className="font-serif text-[14px] text-text-secondary">{pronunciation}</span> : null}
    </div>
  );
}
