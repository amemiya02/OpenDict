import { useEffect, useRef, useState, type ReactNode } from "react";
import { EntryFrame } from "./components/EntryFrame";
import { HeadwordColumn, optionId } from "./components/HeadwordColumn";
import { sectionId, ThumbIndex } from "./components/ThumbIndex";
import { EmptyLibrary, LibraryPage, OutlineButton, type LibraryActions } from "./components/LibraryPage";
import { ReadingSettings } from "./components/ReadingSettings";
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
  setAsAuthored,
  setDictionaryEnabled,
  type Entry,
  type Library,
  type RegisterError,
} from "./lib/ipc";
import { useAppearance, type Scheme } from "./lib/useAppearance";
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
  };

  const enabledCount = library?.dictionaries.filter((d) => d.enabled).length ?? 0;
  const asked = page.kind === "found" ? Math.max(enabledCount, page.entries.length) : enabledCount;
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

      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_auto]">
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

      <footer className="flex items-center gap-[18px] overflow-hidden border-t border-rule px-xl text-label whitespace-nowrap text-text-secondary tabular-nums select-none">
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
        />
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

/** What the page needs to know of the reader's appearance settings. */
interface Reading {
  scheme: Scheme;
  textSize: number;
}

/** The page head and one section per answering Dictionary. `asked` is how many Dictionaries the Lookup ran against. */
function EntryPage({
  headword,
  entries,
  asked,
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
}) {
  const { t } = useI18n();
  const dark = reading.scheme === "dark";
  const pronunciation = entries.find((entry) => entry.pronunciation)?.pronunciation;
  const [folded, setFolded] = useState<ReadonlySet<string>>(new Set());

  function toggleFold(dictionaryId: string) {
    setFolded((current) => {
      const next = new Set(current);
      if (!next.delete(dictionaryId)) next.add(dictionaryId);
      return next;
    });
  }

  return (
    <article>
      <div className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 className="-ml-3.5 font-serif text-display font-bold [overflow-wrap:anywhere]">【{headword}】</h1>
        {pronunciation ? <span className="font-serif text-[16px] text-text-secondary">{pronunciation}</span> : null}
        <span className="text-[12px] text-text-secondary">{t.answeredOf(entries.length, asked)}</span>
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
