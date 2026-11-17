import { useEffect, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { SearchField } from "./components/SearchField";
import { useI18n } from "./i18n";
import { libraryStatus, lookUp, setAllowNetwork, type Entry, type LibraryStatus } from "./lib/ipc";

type PageState =
  | { kind: "idle" }
  | { kind: "found"; headword: string; entries: Entry[] }
  | { kind: "missing"; query: string }
  | { kind: "failed"; reason: string };

/** The main window: cover band, page and status line. */
export function App() {
  const { t } = useI18n();
  const [page, setPage] = useState<PageState>({ kind: "idle" });
  const [status, setStatus] = useState<LibraryStatus | null>(null);

  useEffect(() => {
    libraryStatus().then(setStatus, (error: unknown) => setPage({ kind: "failed", reason: String(error) }));
  }, []);

  async function runLookup(query: string) {
    try {
      const entries = await lookUp(query);
      setPage(entries.length > 0 ? { kind: "found", headword: query, entries } : { kind: "missing", query });
    } catch (error) {
      setPage({ kind: "failed", reason: String(error) });
    }
  }

  return (
    <div className="grid h-full grid-rows-[48px_minmax(0,1fr)_26px]">
      <header className="flex items-center bg-cover text-on-cover select-none">
        <div className="flex w-[264px] flex-none items-center pr-3.5 pl-lg max-[860px]:w-[200px] max-[620px]:w-auto">
          <span className="font-serif text-[17px] font-bold">{t.appName}</span>
        </div>
        <div className="flex min-w-0 flex-1 pr-lg pl-xl max-[860px]:pl-xs">
          <SearchField onSubmit={runLookup} />
        </div>
      </header>

      <main aria-label={t.pageLabel} className="min-h-0 overflow-y-auto pt-[18px] pr-[30px] pb-12 pl-xl">
        <PageContent page={page} status={status} />
      </main>

      <footer className="flex items-center gap-[18px] overflow-hidden border-t border-rule px-xl text-label whitespace-nowrap text-text-secondary tabular-nums select-none">
        <span>{status?.kind === "ready" ? t.libraryCount(status.dictionaryCount) : null}</span>
      </footer>
    </div>
  );
}

function PageContent({ page, status }: { page: PageState; status: LibraryStatus | null }) {
  const { t } = useI18n();
  switch (page.kind) {
    case "found":
      return <EntryPage key={page.headword} headword={page.headword} entries={page.entries} />;
    case "missing":
      return <Lead>{t.notFound(page.query)}</Lead>;
    case "failed":
      return <Lead>{t.lookupFailed(page.reason)}</Lead>;
    case "idle":
      if (status?.kind === "unconfigured") {
        return (
          <Lead>
            {t.noLibraryFolder}
            <span className="block pt-2.5 text-[12px] text-text-secondary">{t.noLibraryFolderHow(status.configFile)}</span>
          </Lead>
        );
      }
      if (status?.kind === "unavailable") return <Lead>{t.libraryUnavailable(status.reason)}</Lead>;
      if (status?.kind === "ready" && status.dictionaryCount === 0) return <Lead>{t.libraryEmpty(status.folder)}</Lead>;
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
