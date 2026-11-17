import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { HeadwordColumn, optionId } from "./components/HeadwordColumn";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { SearchField } from "./components/SearchField";
import { UpdateNotice } from "./components/UpdateNotice";
import { useI18n } from "./i18n";
import {
  addLibraryFolder,
  getLibrary,
  lookUp,
  moveDictionary,
  registerDictionary,
  removeLibraryFolder,
  rescanLibrary,
  setAllowNetwork,
  setDictionaryEnabled,
  type Entry,
  type Library,
  type RegisterError,
  type Suggestion,
} from "./lib/ipc";
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
      setPage(entries.length > 0 ? { kind: "found", headword, entries } : { kind: "missing", query: headword, next: [] });
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
        setPageNow({ kind: "missing", query: text.trim(), next: answer.miss ? answer.items : [] });
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

  /** The reader chose a way forward from a miss: a pattern to try or a headword. */
  function search(text: string) {
    type(text);
    void changeQuery(text);
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
  const { items, selected, miss, near } = column.state;

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
        note={near ? t.nearNote : miss && items.length > 0 ? t.missNote : null}
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
          <PageContent
            page={page}
            library={library}
            busy={busy}
            notice={notice}
            onAddFolder={actions.addFolder}
            onSearch={search}
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
  onSearch,
}: {
  page: PageState;
  library: Library | null;
  busy: boolean;
  notice: string | null;
  onAddFolder: () => void;
  onSearch: (text: string) => void;
}) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return <EntryPage key={page.headword} headword={page.headword} entries={page.entries} />;
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
      return <Lead muted>{t.startHint}</Lead>;
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
