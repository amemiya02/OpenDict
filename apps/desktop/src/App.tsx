import { useEffect, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { SearchField } from "./components/SearchField";
import { useI18n } from "./i18n";
import { libraryStatus, lookUp, type Entry, type LibraryStatus } from "./lib/ipc";

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

function Lead({ children, muted = false }: { children: ReactNode; muted?: boolean }) {
  return <p className={`max-w-[860px] py-lg text-[14px] ${muted ? "text-text-secondary" : ""}`}>{children}</p>;
}
