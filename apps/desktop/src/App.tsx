import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { SearchField } from "./components/SearchField";
import { useI18n } from "./i18n";
import {
  addLibraryFolder,
  getLibrary,
  lookUp,
  moveDictionary,
  registerDictionary,
  removeLibraryFolder,
  rescanLibrary,
  setDictionaryEnabled,
  type Entry,
  type Library,
  type RegisterError,
} from "./lib/ipc";

type PageState =
  | { kind: "idle" }
  | { kind: "found"; headword: string; entries: Entry[] }
  | { kind: "missing"; query: string }
  | { kind: "failed"; reason: string };

type View = "lookup" | "library";

/** The main window: cover band, page and status line. */
export function App() {
  const { t } = useI18n();
  const [page, setPage] = useState<PageState>({ kind: "idle" });
  const [view, setView] = useState<View>("lookup");
  const [library, setLibrary] = useState<Library | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const pageRef = useRef<HTMLElement>(null);

  useEffect(() => {
    getLibrary().then(setLibrary, (error: unknown) => setPage({ kind: "failed", reason: String(error) }));
  }, []);

  // Moving between the page and the Library puts the reader at the top of the new view.
  useEffect(() => {
    pageRef.current?.scrollTo?.({ top: 0 });
    if (view === "library") pageRef.current?.querySelector<HTMLElement>("h1")?.focus();
  }, [view]);

  async function runLookup(query: string) {
    setView("lookup");
    try {
      const entries = await lookUp(query);
      setPage(entries.length > 0 ? { kind: "found", headword: query, entries } : { kind: "missing", query });
    } catch (error) {
      setPage({ kind: "failed", reason: String(error) });
    }
  }

  /** Runs one Library command; the shell answers with the Library as it now is. */
  async function change(command: () => Promise<Library | null>, after?: (library: Library) => void) {
    setBusy(true);
    setNotice(null);
    try {
      const updated = await command();
      if (updated) {
        setLibrary(updated);
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

  return (
    <div className="grid h-full grid-rows-[48px_minmax(0,1fr)_26px]">
      <header className="flex items-center bg-cover text-on-cover select-none">
        <div className="flex w-[264px] flex-none items-center gap-sm pr-3.5 pl-lg max-[860px]:w-[200px] max-[620px]:w-auto">
          <span className="font-serif text-[17px] font-bold">{t.appName}</span>
          <button
            type="button"
            aria-pressed={view === "library"}
            onClick={() => setView(view === "library" ? "lookup" : "library")}
            className="ml-auto h-7 rounded-sharp px-2.5 text-[12px] text-on-cover-muted hover:bg-cover-edge hover:text-on-cover aria-pressed:bg-cover-edge aria-pressed:text-on-cover"
          >
            {t.library}
          </button>
        </div>
        <div className="flex min-w-0 flex-1 pr-lg pl-xl max-[860px]:pl-xs">
          <SearchField onSubmit={runLookup} />
        </div>
      </header>

      <main ref={pageRef} aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-[18px] pr-[30px] pb-12 pl-xl">
        {view === "library" && library ? (
          <LibraryPage library={library} busy={busy} notice={notice} actions={actions} />
        ) : (
          <PageContent page={page} library={library} busy={busy} notice={notice} onAddFolder={actions.addFolder} />
        )}
      </main>

      <footer className="flex items-center gap-[18px] overflow-hidden border-t border-rule px-xl text-label whitespace-nowrap text-text-secondary tabular-nums select-none">
        <span>{library && !library.problem ? t.libraryCount(enabledCount) : null}</span>
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
