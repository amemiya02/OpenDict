import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { HeadwordColumn, optionId, type ColumnList } from "./components/HeadwordColumn";
import { NavButtons } from "./components/NavButtons";
import { RecentLookups } from "./components/RecentLookups";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { SearchField } from "./components/SearchField";
import { UpdateNotice } from "./components/UpdateNotice";
import { useI18n } from "./i18n";
import {
  addLibraryFolder,
  clearHistory,
  getHistory,
  getLibrary,
  lookUp,
  moveDictionary,
  recordLookup,
  registerDictionary,
  removeLibraryFolder,
  rescanLibrary,
  setAllowNetwork,
  setDictionaryEnabled,
  type Entry,
  type Library,
  type RegisterError,
} from "./lib/ipc";
import { canStep, emptyTrail, step, visit } from "./lib/trail";
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
  /** Committed Lookups, newest first. */
  const [history, setHistory] = useState<string[]>([]);
  const [list, setList] = useState<ColumnList>("suggestions");
  /** The words visited in this window, for Back and Forward. */
  const [trail, setTrail] = useState(emptyTrail);
  /** Where the word on the page stands in History, or -1. */
  const historySelected = page.kind === "found" ? history.indexOf(page.headword) : -1;
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

  useEffect(() => {
    getHistory().then((words) => setHistory(asWords(words)), () => {});
  }, []);

  // Moving between the page and the Library puts the reader at the top of the new view.
  useEffect(() => {
    pageRef.current?.scrollTo?.({ top: 0 });
    if (view === "library") pageRef.current?.querySelector<HTMLElement>("h1")?.focus();
  }, [view]);

  function setPageNow(next: PageState) {
    pageRequests.current++;
    setPage(next);
  }

  /**
   * Puts `headword`'s Entries on the page. `onFound` runs in the same update as the page
   * when a Dictionary had the word, so nothing sees the page without it.
   */
  async function show(headword: string, onFound?: () => void) {
    const ticket = ++pageRequests.current;
    setView("lookup");
    try {
      const entries = await lookUp(headword);
      if (ticket !== pageRequests.current) return;
      setPage(entries.length > 0 ? { kind: "found", headword, entries } : { kind: "missing", query: headword });
      if (entries.length > 0) onFound?.();
    } catch (error) {
      if (ticket === pageRequests.current) setPage({ kind: "failed", reason: String(error) });
    }
  }

  /** The reader's query changed: new suggestions, and the first match on the page at once. */
  async function changeQuery(text: string) {
    setList("suggestions");
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
    if (list === "history") {
      if (history.length === 0) return;
      const next = historySelected < 0 ? (by > 0 ? 0 : history.length - 1) : historySelected + by;
      const headword = history[Math.max(0, Math.min(next, history.length - 1))]!;
      type(headword);
      void show(headword);
      return;
    }
    const headword = await column.move(by).catch(() => null);
    if (headword) void show(headword);
  }

  function type(text: string) {
    typed.current = text;
    setQuery(text);
  }

  /**
   * The reader chose a word (Enter, a click, a followed link): it goes into the search
   * field and onto the page. A word that was found is recorded in History and becomes
   * the latest step for Back and Forward.
   */
  async function commit(headword: string) {
    type(headword);
    await show(headword, () => {
      setTrail((now) => visit(now, headword));
      recordLookup(headword).then((words) => setHistory(asWords(words)), () => {});
    });
  }

  function commitCurrent() {
    const { items, selected } = column.state;
    const current = list === "history" ? history[historySelected] : items[selected]?.headword;
    const headword = current ?? query.trim();
    if (headword) void commit(headword);
  }

  function pick(index: number) {
    const headword = list === "history" ? history[index] : column.select(index);
    if (headword) void commit(headword);
  }

  /** Back (-1) or Forward (1) through the words visited. Retracing is not a new Lookup. */
  function go(by: -1 | 1) {
    const next = step(trail, by);
    if (!next) return;
    setTrail(next.trail);
    type(next.word);
    column.refresh(next.word).catch(() => {});
    void show(next.word);
  }

  // Alt+← and Alt+→ work wherever the focus is in the window (but not inside an Entry
  // frame, which is another document).
  const goRef = useRef(go);
  useEffect(() => {
    goRef.current = go;
  });
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault();
      goRef.current(event.key === "ArrowLeft" ? -1 : 1);
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  function forgetHistory() {
    clearHistory().then(
      () => setHistory([]),
      () => {}, // History stays as it was: nothing was cleared
    );
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
    register: async (path, email, code) => {
      setBusy(true);
      setNotice(null);
      try {
        const updated = await registerDictionary(path, email, code);
        setLibrary(updated);
        const unlocked = updated.dictionaries.find((dictionary) => dictionary.path === path);
        if (unlocked) setNotice(t.unlocked(unlocked.title));
        return null;
      } catch (error) {
        // The form words the error beside the fields it is about.
        return error as RegisterError;
      } finally {
        setBusy(false);
      }
    },
  };

  const enabledCount = library?.dictionaries.filter((d) => d.enabled).length ?? 0;
  const { items, selected, miss } = column.state;
  const columnSelected = list === "history" ? historySelected : selected;

  return (
    <div className="grid h-full grid-cols-[264px_minmax(0,1fr)] grid-rows-[48px_minmax(0,1fr)_26px] max-[860px]:grid-cols-[200px_minmax(0,1fr)]">
      <header className="col-span-2 flex items-center bg-cover text-on-cover select-none">
        <div className="flex w-[264px] flex-none items-center gap-sm pr-3.5 pl-lg max-[860px]:w-[200px]">
          <span className="font-serif text-[17px] font-bold">{t.appName}</span>
        </div>
        <NavButtons canGoBack={canStep(trail, -1)} canGoForward={canStep(trail, 1)} onGo={go} />
        <div className="flex min-w-0 flex-1 pr-lg">
          <SearchField
            value={query}
            activeOption={view === "lookup" && columnSelected >= 0 ? optionId(columnSelected) : undefined}
            onChange={type}
            onQuery={(text) => void changeQuery(text)}
            onMove={(by) => void move(by)}
            onCommit={commitCurrent}
          />
        </div>
      </header>

      <HeadwordColumn
        items={list === "history" ? history.map((headword) => ({ headword })) : items}
        selected={columnSelected}
        note={list === "suggestions" && miss && items.length > 0 ? t.missNote : null}
        view={view}
        list={list}
        onPick={pick}
        onNeedMore={() => {
          if (list === "suggestions") void column.loadMore().catch(() => {});
        }}
        onShowSuggestions={() => {
          setList("suggestions");
          setView("lookup");
        }}
        onShowHistory={() => {
          setList("history");
          setView("lookup");
        }}
        onClearHistory={forgetHistory}
        onToggleLibrary={() => setView(view === "library" ? "lookup" : "library")}
      />

      <main ref={pageRef} aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-[18px] pr-[30px] pb-12 pl-xl">
        {view === "library" && library ? (
          <LibraryPage library={library} busy={busy} notice={notice} actions={actions} />
        ) : (
          <PageContent
            page={page}
            library={library}
            busy={busy}
            notice={notice}
            onAddFolder={actions.addFolder}
            recent={<RecentLookups words={history} onPick={(word) => void commit(word)} />}
          />
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
  recent,
}: {
  page: PageState;
  library: Library | null;
  busy: boolean;
  notice: string | null;
  onAddFolder: () => void;
  /** Recent lookups, offered on the empty page. */
  recent: ReactNode;
}) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return <EntryPage key={page.headword} headword={page.headword} entries={page.entries} />;
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
      return (
        <>
          <Lead muted>{t.startHint}</Lead>
          {recent}
        </>
      );
  }
}

/** The page head and one section per answering Dictionary. */
function EntryPage({ headword, entries: initial }: { headword: string; entries: Entry[] }) {
  const { t } = useI18n();
  const [entries, setEntries] = useState(initial);

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
      </div>
      {entries.map((entry) => (
        <section key={entry.dictionaryId} aria-label={entry.dictionaryTitle} className="border-rule [&+&]:border-t">
          <div className="flex h-9 items-center gap-sm">
            <h2 className="-ml-[7px] min-w-0 flex-1 truncate text-title font-bold">【{entry.dictionaryTitle}】</h2>
            <NetworkSwitch allowed={entry.allowNetwork} onToggle={() => void toggleNetwork(entry)} />
          </div>
          {/* A new frame when the switch changes, so the document reloads under its new policy. */}
          <EntryFrame key={String(entry.allowNetwork)} dictionaryTitle={entry.dictionaryTitle} src={entry.src} />
        </section>
      ))}
    </article>
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

/** History as the shell sent it; anything but a list of words counts as none. */
function asWords(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((word): word is string => typeof word === "string") : [];
}
