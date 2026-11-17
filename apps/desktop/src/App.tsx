import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
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
  setAllowNetwork,
  setDictionaryEnabled,
  type Entry,
  type Library,
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
