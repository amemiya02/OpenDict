import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { COLUMN_ID, HeadwordColumn, optionId, type ColumnList } from "./components/HeadwordColumn";
import { NavButtons } from "./components/NavButtons";
import { RecentLookups } from "./components/RecentLookups";
import { sectionId, ThumbIndex } from "./components/ThumbIndex";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { ReadingSettings } from "./components/ReadingSettings";
import { SearchField } from "./components/SearchField";
import { UpdateNotice } from "./components/UpdateNotice";
import { useI18n } from "./i18n";
import { soundUrl } from "./lib/entryLinks";
import { playSound } from "./lib/sound";
import {
  addLibraryFolder,
  cancelIndexing,
  clearHistory,
  getHistory,
  getLibrary,
  indexLibrary,
  lookUp,
  moveDictionary,
  recordLookup,
  registerDictionary,
  onLibraryChanged,
  removeLibraryFolder,
  rescanLibrary,
  setAllowNetwork,
  setAsAuthored,
  setDictionaryEnabled,
  windowChrome,
  type Entry,
  type Library,
  type RegisterError,
  type WindowChrome,
  type Suggestion,
} from "./lib/ipc";
import { useAppearance, type Scheme } from "./lib/useAppearance";
import { canStep, emptyTrail, step, visit } from "./lib/trail";
import { useHeadwordColumn } from "./lib/useHeadwordColumn";
import { wildcardHint } from "./lib/wildcard";

type PageState =
  | { kind: "idle" }
  | { kind: "found"; headword: string; entries: Entry[] }
  /** Nothing found for `query`; `next` are the headwords after where it would stand. */
  | { kind: "missing"; query: string; next: Suggestion[] }
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
  const [platform, setPlatform] = useState<WindowChrome["platform"] | null>(null);
  /** Narrow windows: the reader opened the headword column with the toggle in the cover band. */
  const [columnOpen, setColumnOpen] = useState(false);
  /** Narrow windows: the reader is typing, so the column shows over the page until a word is chosen. */
  const [typing, setTyping] = useState(false);
  const pageRef = useRef<HTMLElement>(null);
  const column = useHeadwordColumn();
  const appearance = useAppearance();
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
    // Indexing runs in the shell's background and reports the Library as it goes. When
    // another Dictionary's index is ready, the column has more to offer.
    let ready = -1;
    const unlisten = onLibraryChanged((changed) => {
      setLibrary(changed);
      const nowReady = changed.dictionaries.filter((d) => d.index.state === "ready").length;
      if (nowReady !== ready) {
        ready = nowReady;
        refresh(typed.current).catch(() => {});
      }
    });
    return () => {
      unlisten.then((stop) => stop(), () => {});
    };
  }, [refresh]);

  useEffect(() => {
    getHistory().then((words) => setHistory(asWords(words)), () => {});
  }, []);

  // The shell says what it built the window with: the styles leave room for the title bar
  // controls and let the system material show through (see styles.css).
  useEffect(() => {
    windowChrome().then(
      (chrome: WindowChrome | null) => {
        if (!chrome) return; // a fake shell that answers nothing
        const root = document.documentElement;
        root.dataset.platform = chrome.platform;
        if (chrome.material) root.dataset.material = chrome.material;
        else delete root.dataset.material;
        setPlatform(chrome.platform);
      },
      () => {}, // no shell (or an old one): plain solid window
    );
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
      setPage(entries.length > 0 ? { kind: "found", headword, entries } : { kind: "missing", query: headword, next: [] });
      if (entries.length > 0) onFound?.();
    } catch (error) {
      if (ticket === pageRequests.current) setPage({ kind: "failed", reason: String(error) });
    }
  }

  /** The reader's query changed: new suggestions, and the first match on the page at once. */
  async function changeQuery(text: string) {
    setList("suggestions");
    setTyping(true);
    try {
      const answer = await column.refresh(text);
      if (!answer) return;
      if (answer.headword) {
        void show(answer.headword);
      } else if (text.trim()) {
        setView("lookup");
        setPageNow({ kind: "missing", query: text.trim(), next: answer.miss ? answer.items : [] });
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
    putColumnAway();
    type(headword);
    await show(headword, () => {
      setTrail((now) => visit(now, headword));
      recordLookup(headword).then((words) => setHistory(asWords(words)), () => {});
    });
  }

  /** A narrow window's column overlay goes once a word is chosen. */
  function putColumnAway() {
    setTyping(false);
    setColumnOpen(false);
  }

  /** An `entry://` link in an Entry: the linked headword, as if the reader had typed and chosen it. */
  function followLink(headword: string) {
    void commit(headword);
    column.refresh(headword).catch(() => {});
  }

  /** The reader chose a way forward from a miss: a pattern to try or a headword. */
  function search(text: string) {
    type(text);
    void changeQuery(text);
  }

  function commitCurrent() {
    const { items, selected, query: answered } = column.state;
    // Until the shell answers the latest text, the suggestions belong to an earlier query:
    // Enter then takes what was typed, not the earlier query's current headword.
    const fresh = answered === typed.current;
    const current = list === "history" ? history[historySelected] : fresh ? items[selected]?.headword : undefined;
    const headword = current ?? typed.current.trim();
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

  /** Changes one Entry on the page, if it is still there. */
  function updateEntry(dictionaryId: string, change: Partial<Entry>) {
    setPage((current) =>
      current.kind === "found"
        ? { ...current, entries: current.entries.map((e) => (e.dictionaryId === dictionaryId ? { ...e, ...change } : e)) }
        : current,
    );
  }

  async function toggleNetwork(entry: Entry) {
    const allow = !entry.allowNetwork;
    try {
      await setAllowNetwork(entry.dictionaryId, allow);
    } catch {
      return; // the switch stays as it was: the preference was not saved
    }
    updateEntry(entry.dictionaryId, { allowNetwork: allow });
  }

  async function toggleAsAuthored(entry: Entry) {
    const asAuthored = !entry.asAuthored;
    try {
      await setAsAuthored(entry.dictionaryId, asAuthored);
    } catch {
      return;
    }
    updateEntry(entry.dictionaryId, { asAuthored });
  }

  const reading = {
    ...appearance,
    /** Every Dictionary at once, the ones on the page included. */
    setAllAsAuthored: async (asAuthored: boolean) => {
      await appearance.setAllAsAuthored(asAuthored);
      setPage((current) =>
        current.kind === "found" ? { ...current, entries: current.entries.map((e) => ({ ...e, asAuthored })) } : current,
      );
    },
  };

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
    index: () => change(indexLibrary),
    cancelIndexing: () => {
      cancelIndexing().catch((error: unknown) => setNotice(t.libraryActionFailed(String(error))));
    },
  };

  const enabledCount = library?.dictionaries.filter((d) => d.enabled).length ?? 0;
  const asked = page.kind === "found" ? Math.max(enabledCount, page.entries.length) : enabledCount;
  const { items, selected, miss, near, lemma } = column.state;
  const note = lemma ? (
    <>
      {t.lemmatized} <b>{lemma.from}</b> → <b>{lemma.to}</b>
    </>
  ) : near ? (
    t.nearNote
  ) : miss && items.length > 0 ? (
    t.missNote
  ) : null;
  const columnSelected = list === "history" ? historySelected : selected;

  return (
    <div className="grid h-full grid-cols-[264px_minmax(0,1fr)] grid-rows-[48px_minmax(0,1fr)_26px] max-[860px]:grid-cols-[200px_minmax(0,1fr)] max-[620px]:grid-cols-[minmax(0,1fr)]">
      <header
        // On macOS the cover band is the title bar (the system's controls sit at its left).
        data-tauri-drag-region={platform === "macos" ? "deep" : undefined}
        className="col-span-full flex items-center bg-cover text-on-cover select-none"
      >
        <div className="flex w-[264px] flex-none items-center gap-sm pr-3.5 pl-lg max-[860px]:w-[200px] max-[620px]:w-auto max-[620px]:pr-1.5 macos:pl-20">
          <span className="font-serif text-[17px] font-bold max-[620px]:hidden">{t.appName}</span>
          <button
            type="button"
            aria-label={t.toggleColumn}
            aria-controls={COLUMN_ID}
            aria-expanded={columnOpen || typing}
            // Keep the caret in the search field: the toggle answers what is shown now.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              setTyping(false);
              setColumnOpen(!(columnOpen || typing));
            }}
            className="hidden size-6.5 place-items-center rounded-sharp text-on-cover-muted hover:bg-cover-edge hover:text-on-cover max-[620px]:grid"
          >
            <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
              <rect x="2.5" y="3" width="11" height="10" />
              <path d="M6.5 3v10" />
            </svg>
          </button>
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
            onEscape={putColumnAway}
            onBlur={() => setTyping(false)}
          />
        </div>
      </header>

      <HeadwordColumn
        items={list === "history" ? history.map((headword) => ({ headword })) : items}
        selected={columnSelected}
        note={list === "suggestions" ? note : null}
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
        onToggleLibrary={() => {
          setColumnOpen(false);
          setView(view === "library" ? "lookup" : "library");
        }}
        open={columnOpen || typing}
      />

      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_auto] bg-paper">
        <main ref={pageRef} aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-[18px] pr-[30px] pb-12 pl-xl">
          {view === "library" && library ? (
            <>
              <LibraryPage library={library} busy={busy} notice={notice} actions={actions} />
              <ReadingSettings controls={reading} />
            </>
          ) : (
            <PageContent
              page={page}
              asked={asked}
              library={library}
              busy={busy}
              notice={notice}
              onAddFolder={actions.addFolder}
              reading={{ scheme: appearance.scheme, textSize: appearance.appearance.textSize }}
              onToggleNetwork={(entry) => void toggleNetwork(entry)}
              onToggleAsAuthored={(entry) => void toggleAsAuthored(entry)}
              onLookUp={followLink}
              onSearch={search}
              recent={<RecentLookups words={history} onPick={(word) => void commit(word)} />}
            />
          )}
        </main>
        {view === "lookup" ? (
          <div className="w-[168px] min-h-0 overflow-hidden max-[1100px]:w-[46px]">
            {page.kind === "found" ? (
              <ThumbIndex
                key={page.headword}
                notches={page.entries.map((e) => ({ dictionaryId: e.dictionaryId, title: e.dictionaryTitle }))}
                unanswered={asked - page.entries.length}
                page={pageRef}
              />
            ) : null}
          </div>
        ) : null}
      </div>

      <footer className="flex items-center bg-paper gap-[18px] overflow-hidden border-t border-rule px-xl text-label whitespace-nowrap text-text-secondary tabular-nums select-none">
        <span>{library && !library.problem ? t.libraryCount(enabledCount) : null}</span>
        <UpdateNotice />
      </footer>
    </div>
  );
}

function PageContent({
  page,
  asked,
  library,
  busy,
  notice,
  onAddFolder,
  reading,
  onToggleNetwork,
  onToggleAsAuthored,
  onLookUp,
  onSearch,
  recent,
}: {
  page: PageState;
  /** How many Dictionaries the Lookup ran against. */
  asked: number;
  library: Library | null;
  busy: boolean;
  notice: string | null;
  onAddFolder: () => void;
  reading: Reading;
  onToggleNetwork: (entry: Entry) => void;
  onToggleAsAuthored: (entry: Entry) => void;
  onLookUp: (headword: string) => void;
  onSearch: (text: string) => void;
  /** Recent lookups, offered on the empty page. */
  recent: ReactNode;
}) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return (
        <EntryPage
          key={page.headword}
          headword={page.headword}
          entries={page.entries}
          asked={asked}
          reading={reading}
          onToggleNetwork={onToggleNetwork}
          onToggleAsAuthored={onToggleAsAuthored}
          onLookUp={onLookUp}
        />
      );
    case "missing":
      return <NoMatch query={page.query} next={page.next} onSearch={onSearch} />;
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

/** What the page needs to know of the reader's appearance settings. */
interface Reading {
  scheme: Scheme;
  textSize: number;
}

/**
 * The page head and one section per answering Dictionary. `asked` is how many Dictionaries
 * the Lookup ran against. An `entry://` link in any Entry looks its headword up; a
 * `sound://` link plays from its own Dictionary.
 */
function EntryPage({
  headword,
  entries,
  asked,
  onLookUp,
  reading,
  onToggleNetwork,
  onToggleAsAuthored,
}: {
  headword: string;
  entries: Entry[];
  asked: number;
  reading: Reading;
  onToggleNetwork: (entry: Entry) => void;
  onToggleAsAuthored: (entry: Entry) => void;
  onLookUp: (headword: string) => void;
}) {
  const { t } = useI18n();
  const dark = reading.scheme === "dark";
  const pronunciation = entries.find((entry) => entry.pronunciation)?.pronunciation;
  const sound = firstSound(entries);
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());

  function toggleFold(dictionaryId: string) {
    setFolded((current) => {
      const next = new Set(current);
      if (!next.delete(dictionaryId)) next.add(dictionaryId);
      return next;
    });
  }

  // Alt+P: the Pronounce action, wherever focus is in the app (not inside an Entry frame,
  // whose keys stay its own).
  useEffect(() => {
    if (sound === null) return;
    const url = sound;
    function onKeyDown(event: KeyboardEvent) {
      if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      if (event.code !== "KeyP" && event.key.toLowerCase() !== "p") return;
      event.preventDefault();
      playSound(url);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [sound]);

  return (
    <article>
      <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{headword}】</h1>
        {pronunciation ? <span className="font-serif text-[16px] text-text-secondary">{pronunciation}</span> : null}
        <span className="text-[12px] text-text-secondary">{t.answeredOf(entries.length, asked)}</span>
        {sound !== null && (
          <div className="ml-auto flex gap-1.5 self-center">
            <PronounceButton onClick={() => playSound(sound)} />
          </div>
        )}
      </div>
      {entries.map((entry) => {
        const open = !folded.has(entry.dictionaryId);
        // In the dark theme an Entry is recoloured, or keeps its own white page as authored.
        const whitePage = dark && entry.asAuthored;
        const body = `${sectionId(entry.dictionaryId)}-body`;
        return (
          <section
            key={entry.dictionaryId}
            id={sectionId(entry.dictionaryId)}
            tabIndex={-1}
            aria-label={entry.dictionaryTitle}
            className="scroll-mt-3 border-rule outline-offset-4 [&+&]:border-t"
          >
            <div className="flex h-9 items-center gap-sm">
              <h2 className="-ml-[7px] min-w-0 flex-1 truncate text-title font-bold">【{entry.dictionaryTitle}】</h2>
              {dark ? <AsAuthoredSwitch on={entry.asAuthored} onToggle={() => onToggleAsAuthored(entry)} /> : null}
              <NetworkSwitch allowed={entry.allowNetwork} onToggle={() => onToggleNetwork(entry)} />
              <FoldButton open={open} controls={body} onToggle={() => toggleFold(entry.dictionaryId)} />
            </div>
            {/* Folded, the Entry stays loaded out of sight, so unfolding shows it as it was. */}
            <div id={body} hidden={!open} className="pb-3.5">
              <div className={whitePage ? "max-w-[860px] rounded-sharp bg-india-paper px-3 pt-2.5 pb-3" : undefined}>
                {/* A new frame when the switch changes, so the document reloads under its new policy. */}
                <EntryFrame
                  key={String(entry.allowNetwork)}
                  dictionaryTitle={entry.dictionaryTitle}
                  src={entry.src}
                  onLink={onLookUp}
                  onSound={playSound}
                  textSize={reading.textSize}
                  recolour={dark && !entry.asAuthored}
                />
              </div>
            </div>
          </section>
        );
      })}
    </article>
  );
}

/** The URL of the first audio the Entries offer, in page order, or null. */
function firstSound(entries: Entry[]): string | null {
  for (const entry of entries) {
    const url = entry.sound ? soundUrl(entry.src, entry.sound) : null;
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

/**
 * The page when nothing was found: it says so, suggests a wildcard pattern, and lists the
 * headwords that follow where the word would stand, so a miss is never a dead end.
 */
function NoMatch({ query, next, onSearch }: { query: string; next: Suggestion[]; onSearch: (text: string) => void }) {
  const { t } = useI18n();
  const pattern = wildcardHint(query);
  return (
    <article>
      <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{query}】</h1>
        <span className="text-[12px] text-text-secondary">{t.notFoundMeta}</span>
      </div>
      <Lead>{t.notFound(query)}</Lead>
      <div className="flex max-w-[860px] flex-wrap items-center gap-x-2.5 gap-y-sm pb-xs text-text-secondary">
        <span>{t.wildcardTip}</span>
        <OutlineButton onClick={() => onSearch(pattern)}>
          {t.tryPattern} <b className="font-serif text-[14px]">{pattern}</b>
        </OutlineButton>
      </div>
      {next.length > 0 ? (
        <>
          <h2 id="next-in-index" className="mt-5 mb-1.5 text-[13px] font-bold">
            {t.nextInIndex}
          </h2>
          <ul aria-labelledby="next-in-index" className="columns-[150px] gap-x-7 [column-rule:1px_solid_var(--hairline)] pt-0.5 pb-1.5">
            {next.slice(0, 40).map((item) => (
              <li key={item.headword}>
                <button
                  type="button"
                  lang={/[぀-ヿ]/.test(item.headword) ? "ja" : undefined}
                  onClick={() => onSearch(item.headword)}
                  className="-ml-1.5 block h-[26px] w-full truncate rounded-sharp px-1.5 text-left font-serif text-index hover:bg-shade"
                >
                  {item.headword}
                </button>
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </article>
  );
}

/** The dark theme's switch in the section header: pressed when the Entry keeps its own white page. */
function AsAuthoredSwitch({ on, onToggle }: { on: boolean; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-pressed={on}
      aria-label={t.asAuthored}
      title={on ? t.shownAsAuthored : t.shownRecoloured}
      onClick={onToggle}
      className="grid size-6.5 flex-none place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text aria-pressed:bg-text aria-pressed:text-paper"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.2]" viewBox="0 0 16 16" aria-hidden="true">
        <path d="M4 2.5h5.5L12 5v8.5H4z" />
        <path d="M9.5 2.5V5H12" />
      </svg>
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

/** The chevron at the right of a section header: points down while the Entry shows. */
function FoldButton({ open, controls, onToggle }: { open: boolean; controls: string; onToggle: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      aria-expanded={open}
      aria-controls={controls}
      aria-label={open ? t.collapse : t.expand}
      onClick={onToggle}
      className="grid size-6.5 flex-none place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text"
    >
      <svg
        className={`size-4 fill-none stroke-current stroke-[1.2] motion-safe:transition-[rotate] motion-safe:duration-180 motion-safe:ease-out-expo ${open ? "" : "-rotate-90"}`}
        viewBox="0 0 16 16"
        aria-hidden="true"
      >
        <path d="M4 6l4 4 4-4" />
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
