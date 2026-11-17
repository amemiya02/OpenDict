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
  removeLibraryFolder,
  rescanLibrary,
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
          <PageContent page={page} library={library} busy={busy} notice={notice} onAddFolder={actions.addFolder} />
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
}: {
  page: PageState;
  library: Library | null;
  busy: boolean;
  notice: string | null;
  onAddFolder: () => void;
}) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return (
        <article>
          <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
            <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{page.headword}】</h1>
            <span className="text-[12px] text-text-secondary">{t.answered(page.entries.length)}</span>
          </div>
          {page.entries.map((entry) => (
            <section key={entry.dictionaryId} aria-label={entry.dictionaryTitle} className="border-rule [&+&]:border-t">
              <div className="flex h-9 items-center gap-sm">
                <h2 className="-ml-[7px] truncate text-title font-bold">【{entry.dictionaryTitle}】</h2>
              </div>
              <EntryFrame dictionaryTitle={entry.dictionaryTitle} html={entry.html} />
            </section>
          ))}
        </article>
      );
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

function Lead({ children, muted = false }: { children: ReactNode; muted?: boolean }) {
  return <p className={`max-w-[860px] py-lg text-[14px] ${muted ? "text-text-secondary" : ""}`}>{children}</p>;
}
