import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { HeadwordColumn, optionId } from "./components/HeadwordColumn";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { SearchField } from "./components/SearchField";
import { UpdateNotice } from "./components/UpdateNotice";
import { useI18n } from "./i18n";
import { soundUrl } from "./lib/entryLinks";
import { playSound } from "./lib/sound";
import {
  addLibraryFolder,
  getLibrary,
  lookUp,
  moveDictionary,
  removeLibraryFolder,
  rescanLibrary,
  setAllowNetwork,
  setDictionaryEnabled,
  type Entry,
  type Library,
} from "./lib/ipc";
import { useHeadwordColumn } from "./lib/useHeadwordColumn";

type PageState =
  | { kind: "idle" }
  | { kind: "found"; headword: string; entries: Entry[] }
  | { kind: "missing"; query: string }
  | { kind: "failed"; reason: string };

type View = "lookup" | "library";

/** The main window: cover band, headword column, page and status line. */
export function App() {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [page, setPage] = useState<PageState>({ kind: "idle" });
  const [view, setView] = useState<View>("lookup");
  const [library, setLibrary] = useState<Library | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const pageRef = useRef<HTMLElement>(null);
  const column = useHeadwordColumn();
  const { refresh } = column;
  /** Counts page requests, so only the latest one lands. */
  const pageRequests = useRef(0);
  /** The text in the search field, for refreshing the column after a Library change. */
  const typed = useRef("");

  // The column opens on the headword index, to browse.
  useEffect(() => {
    getLibrary().then(
      (loaded) => {
        setLibrary(loaded);
        refresh(typed.current).catch(() => {});
      },
      (error: unknown) => setPage({ kind: "failed", reason: String(error) }),
    );
  }, [refresh]);

  // Moving between the page and the Library puts the reader at the top of the new view.
  useEffect(() => {
    pageRef.current?.scrollTo?.({ top: 0 });
    if (view === "library") pageRef.current?.querySelector<HTMLElement>("h1")?.focus();
  }, [view]);

  function setPageNow(next: PageState) {
    pageRequests.current++;
    setPage(next);
  }

  /** Puts `headword`'s Entries on the page. */
  async function show(headword: string) {
    const ticket = ++pageRequests.current;
    setView("lookup");
    try {
      const entries = await lookUp(headword);
      if (ticket !== pageRequests.current) return;
      setPage(entries.length > 0 ? { kind: "found", headword, entries } : { kind: "missing", query: headword });
    } catch (error) {
      if (ticket === pageRequests.current) setPage({ kind: "failed", reason: String(error) });
    }
  }

  /** The reader's query changed: new suggestions, and the first match on the page at once. */
  async function changeQuery(text: string) {
    try {
      const answer = await column.refresh(text);
      if (!answer) return;
      if (answer.headword) {
        void show(answer.headword);
      } else if (text.trim()) {
        setView("lookup");
        setPageNow({ kind: "missing", query: text.trim() });
      } else {
        setPageNow({ kind: "idle" });
      }
    } catch (error) {
      setPageNow({ kind: "failed", reason: String(error) });
    }
  }

  async function move(by: number) {
    const headword = await column.move(by).catch(() => null);
    if (headword) void show(headword);
  }

  function type(text: string) {
    typed.current = text;
    setQuery(text);
  }

  /** The reader chose a word: it goes into the search field and onto the page. */
  function commit(headword: string) {
    type(headword);
    void show(headword);
  }

  /** An `entry://` link in an Entry: the linked headword, as if the reader had typed and chosen it. */
  function followLink(headword: string) {
    commit(headword);
    column.refresh(headword).catch(() => {});
  }

  function commitCurrent() {
    const { items, selected } = column.state;
    const headword = items[selected]?.headword ?? query.trim();
    if (headword) commit(headword);
  }

  function pick(index: number) {
    const headword = column.select(index);
    if (headword) commit(headword);
  }

  /** Runs one Library command; the shell answers with the Library as it now is. */
  async function change(command: () => Promise<Library | null>, after?: (library: Library) => void) {
    setBusy(true);
    setNotice(null);
    try {
      const updated = await command();
      if (updated) {
        setLibrary(updated);
        // Which headwords there are, and how many Dictionaries have each, may have changed.
        column.refresh(typed.current).catch(() => {});
        after?.(updated);
      }
    } catch (error) {
      setNotice(t.libraryActionFailed(String(error)));
    } finally {
      setBusy(false);
    }
  }

  const actions: LibraryActions = {
    addFolder: () => change(addLibraryFolder, () => setView("library")),
    removeFolder: (path) => change(() => removeLibraryFolder(path)),
    rescan: () => change(rescanLibrary),
    setEnabled: (id, enabled) => change(() => setDictionaryEnabled(id, enabled)),
    move: (id, direction) => change(() => moveDictionary(id, direction)),
  };

  const enabledCount = library?.dictionaries.filter((d) => d.enabled).length ?? 0;
  const { items, selected, miss } = column.state;

  return (
    <div className="grid h-full grid-cols-[264px_minmax(0,1fr)] grid-rows-[48px_minmax(0,1fr)_26px] max-[860px]:grid-cols-[200px_minmax(0,1fr)]">
      <header className="col-span-2 flex items-center bg-cover text-on-cover select-none">
        <div className="flex w-[264px] flex-none items-center gap-sm pr-3.5 pl-lg max-[860px]:w-[200px]">
          <span className="font-serif text-[17px] font-bold">{t.appName}</span>
        </div>
        <div className="flex min-w-0 flex-1 pr-lg pl-xl max-[860px]:pl-xs">
          <SearchField
            value={query}
            activeOption={view === "lookup" && selected >= 0 ? optionId(selected) : undefined}
            onChange={type}
            onQuery={(text) => void changeQuery(text)}
            onMove={(by) => void move(by)}
            onCommit={commitCurrent}
          />
        </div>
      </header>

      <HeadwordColumn
        items={items}
        selected={selected}
        note={miss && items.length > 0 ? t.missNote : null}
        view={view}
        onPick={pick}
        onNeedMore={() => void column.loadMore().catch(() => {})}
        onShowSuggestions={() => setView("lookup")}
        onToggleLibrary={() => setView(view === "library" ? "lookup" : "library")}
      />

      <main ref={pageRef} aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-[18px] pr-[30px] pb-12 pl-xl">
        {view === "library" && library ? (
          <LibraryPage library={library} busy={busy} notice={notice} actions={actions} />
        ) : (
          <PageContent page={page} library={library} busy={busy} notice={notice} onAddFolder={actions.addFolder} onLookUp={followLink} />
        )}
      </main>

      <footer className="flex items-center gap-[18px] overflow-hidden border-t border-rule px-xl text-label whitespace-nowrap text-text-secondary tabular-nums select-none">
        <span>{library && !library.problem ? t.libraryCount(enabledCount) : null}</span>
        <UpdateNotice />
      </footer>
    </div>
  );
}

function PageContent({
  page,
  library,
  busy,
  notice,
  onAddFolder,
  onLookUp,
}: {
  page: PageState;
  library: Library | null;
  busy: boolean;
  notice: string | null;
  onAddFolder: () => void;
  onLookUp: (headword: string) => void;
}) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return <EntryPage key={page.headword} headword={page.headword} entries={page.entries} onLookUp={onLookUp} />;
    case "missing":
      return <Lead>{t.notFound(page.query)}</Lead>;
    case "failed":
      return <Lead>{t.lookupFailed(page.reason)}</Lead>;
    case "idle":
      if (library?.problem) return <Lead>{t.libraryUnavailable(library.problem)}</Lead>;
      if (library && library.dictionaries.length === 0) {
        return (
          <EmptyLibrary
            action={
              <span className="flex items-center gap-sm">
                <OutlineButton onClick={onAddFolder} disabled={busy}>
                  {t.addFolder}
                </OutlineButton>
                <span role="status" className="text-label text-text-secondary">
                  {busy ? t.working : notice}
                </span>
              </span>
            }
          />
        );
      }
      return <Lead muted>{t.startHint}</Lead>;
  }
}

/**
 * The page head and one section per answering Dictionary. An `entry://` link in any Entry
 * looks its headword up; a `sound://` link plays from its own Dictionary.
 */
function EntryPage({
  headword,
  entries: initial,
  onLookUp,
}: {
  headword: string;
  entries: Entry[];
  onLookUp: (headword: string) => void;
}) {
  const { t } = useI18n();
  const [entries, setEntries] = useState(initial);
  const pronunciation = firstPronunciation(entries);

  // Alt+P: the Pronounce action, wherever focus is in the app (not inside an Entry frame,
  // whose keys stay its own).
  useEffect(() => {
    if (pronunciation === null) return;
    const url = pronunciation;
    function onKeyDown(event: KeyboardEvent) {
      if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      if (event.code !== "KeyP" && event.key.toLowerCase() !== "p") return;
      event.preventDefault();
      playSound(url);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [pronunciation]);

  async function toggleNetwork(entry: Entry) {
    const allow = !entry.allowNetwork;
    try {
      await setAllowNetwork(entry.dictionaryId, allow);
    } catch {
      return; // the switch stays as it was: the preference was not saved
    }
    setEntries((current) => current.map((e) => (e.dictionaryId === entry.dictionaryId ? { ...e, allowNetwork: allow } : e)));
  }

  return (
    <article>
      <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{headword}】</h1>
        <span className="text-[12px] text-text-secondary">{t.answered(entries.length)}</span>
        {pronunciation !== null && (
          <div className="ml-auto flex gap-1.5 self-center">
            <PronounceButton onClick={() => playSound(pronunciation)} />
          </div>
        )}
      </div>
      {entries.map((entry) => (
        <section key={entry.dictionaryId} aria-label={entry.dictionaryTitle} className="border-rule [&+&]:border-t">
          <div className="flex h-9 items-center gap-sm">
            <h2 className="-ml-[7px] min-w-0 flex-1 truncate text-title font-bold">【{entry.dictionaryTitle}】</h2>
            <NetworkSwitch allowed={entry.allowNetwork} onToggle={() => void toggleNetwork(entry)} />
          </div>
          {/* A new frame when the switch changes, so the document reloads under its new policy. */}
          <EntryFrame
            key={String(entry.allowNetwork)}
            dictionaryTitle={entry.dictionaryTitle}
            src={entry.src}
            onLink={onLookUp}
            onSound={playSound}
          />
        </section>
      ))}
    </article>
  );
}

/** The URL of the first audio the Entries offer, in page order, or null. */
function firstPronunciation(entries: Entry[]): string | null {
  for (const entry of entries) {
    const url = entry.pronunciation ? soundUrl(entry.src, entry.pronunciation) : null;
    if (url !== null) return url;
  }
  return null;
}

function PronounceButton({ onClick }: { onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onClick}
      aria-keyshortcuts="Alt+P"
      className="flex h-7 items-center gap-1.5 rounded-sharp border border-rule px-2.5 text-body text-text hover:bg-shade active:translate-y-px"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
        <path d="M2.5 6v4H5l3.5 3V3L5 6z" />
        <path d="M11 5.5a3.5 3.5 0 0 1 0 5" />
      </svg>
      <span>{t.pronounce}</span>
      <kbd aria-hidden="true" className="font-sans text-[11px] text-text-secondary max-[620px]:hidden">
        Alt P
      </kbd>
    </button>
  );
}

/** An icon button in the section header: pressed when the Dictionary may use the network. */
function NetworkSwitch({ allowed, onToggle }: { allowed: boolean; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-pressed={allowed}
      aria-label={t.allowNetwork}
      title={allowed ? t.networkAllowed : t.networkBlocked}
      onClick={onToggle}
      className="grid size-6.5 flex-none place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text aria-pressed:bg-text aria-pressed:text-paper"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="5.5" />
        <path d="M2.5 8h11M8 2.5c-1.8 1.6-2.6 3.4-2.6 5.5s.8 3.9 2.6 5.5c1.8-1.6 2.6-3.4 2.6-5.5S9.8 4.1 8 2.5Z" />
      </svg>
    </button>
  );
}

function Lead({ children, muted = false }: { children: ReactNode; muted?: boolean }) {
  return <p className={`max-w-[860px] py-lg text-[14px] ${muted ? "text-text-secondary" : ""}`}>{children}</p>;
}
