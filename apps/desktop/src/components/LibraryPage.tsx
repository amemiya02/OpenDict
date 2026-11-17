import type { ReactNode } from "react";
import { useI18n } from "../i18n";
import { formatCount, formatSize, markOf } from "../lib/format";
import type { Dictionary, FailureReason, Indexing, Library } from "../lib/ipc";
import type { Catalogue } from "../i18n/en";

export interface LibraryActions {
  addFolder: () => void;
  removeFolder: (path: string) => void;
  rescan: () => void;
  setEnabled: (id: string, enabled: boolean) => void;
  move: (id: string, direction: "up" | "down") => void;
  /** Indexes the Dictionaries still waiting for their index. */
  index: () => void;
  cancelIndexing: () => void;
}

/**
 * The Library view: Library Folders, then every Dictionary in Library order with its
 * enable switch, then the Dictionaries that could not be opened. Set like the page:
 * a head over a double rule, hairlines between rows, no boxes.
 */
export function LibraryPage({
  library,
  busy,
  notice,
  actions,
}: {
  library: Library;
  busy: boolean;
  notice: string | null;
  actions: LibraryActions;
}) {
  const { t } = useI18n();
  const empty = library.dictionaries.length === 0 && library.failures.length === 0;
  const waiting = library.dictionaries.filter((d) => d.index.state === "pending").length;

  return (
    <article aria-busy={busy}>
      <header className="flex flex-wrap items-baseline gap-x-md gap-y-xs border-b-[3px] border-double border-rule pb-2.5">
        <h1 tabIndex={-1} className="-ml-3.5 font-serif text-display font-bold outline-0">
          【{t.library}】
        </h1>
        <span className="text-[12px] text-text-secondary">{t.libraryMeta(library.dictionaries.length)}</span>
      </header>

      <div className="flex flex-wrap items-center gap-1.5 pt-3.5">
        <OutlineButton onClick={actions.addFolder} disabled={busy}>
          {t.addFolder}
        </OutlineButton>
        <OutlineButton onClick={actions.rescan} disabled={busy || library.folders.length === 0}>
          {t.rescan}
        </OutlineButton>
        <span role="status" className="text-label text-text-secondary">
          {busy ? t.working : notice}
        </span>
      </div>

      {library.indexing ? (
        <IndexingProgress indexing={library.indexing} onCancel={actions.cancelIndexing} />
      ) : waiting > 0 ? (
        <div className="flex flex-wrap items-center gap-x-3.5 gap-y-1.5 pt-3 text-label text-text-secondary">
          <span>{t.notIndexedCount(waiting)}</span>
          <OutlineButton onClick={actions.index} disabled={busy}>
            {t.indexNow}
          </OutlineButton>
        </div>
      ) : null}

      {empty ? <EmptyLibrary /> : null}

      {library.folders.length > 0 ? (
        <>
          <SubHeading>{t.libraryFolders}</SubHeading>
          <ul className="max-w-[860px]">
            {library.folders.map((folder) => (
              <li key={folder.path} className="flex items-center gap-3.5 border-t border-hair py-1">
                <code className="min-w-0 truncate font-sans">{folder.path}</code>
                <span className="text-[12px] text-text-secondary">
                  {folder.problem ? t.folderUnreadable(folder.problem) : t.dictionariesFound(folder.dictionaryCount)}
                </span>
                <button
                  type="button"
                  aria-label={t.removeFolder(folder.path)}
                  disabled={busy}
                  onClick={() => actions.removeFolder(folder.path)}
                  className="ml-auto h-6.5 flex-none rounded-sharp px-2 text-[12px] text-text-secondary hover:bg-shade hover:text-text disabled:opacity-50"
                >
                  {t.remove}
                </button>
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {library.dictionaries.length > 0 ? (
        <>
          <SubHeading>{t.dictionaries}</SubHeading>
          <DictionaryTable dictionaries={library.dictionaries} busy={busy} actions={actions} />
          <p className="max-w-[860px] pt-2.5 text-[12px] text-text-secondary">{t.libraryNote}</p>
        </>
      ) : null}

      {library.failures.length > 0 ? (
        <>
          <SubHeading>{t.failedToOpen}</SubHeading>
          <ul className="max-w-[860px]">
            {library.failures.map((failure) => (
              <li key={failure.path} className="flex flex-wrap items-baseline gap-x-3.5 border-t border-hair py-1">
                <code className="min-w-0 font-sans [overflow-wrap:anywhere]">{failure.path}</code>
                <span className="text-[12px] text-text-secondary">{describeFailure(t, failure.reason)}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </article>
  );
}

function DictionaryTable({
  dictionaries,
  busy,
  actions,
}: {
  dictionaries: Dictionary[];
  busy: boolean;
  actions: LibraryActions;
}) {
  const { t, locale } = useI18n();
  const cell = "h-[34px] border-t border-hair pr-2.5 whitespace-nowrap";
  const head = "h-7 border-y border-rule pr-2.5 text-left text-label font-normal whitespace-nowrap text-text-secondary";
  return (
    <table className="w-full max-w-[860px] border-collapse tabular-nums">
      <thead>
        <tr>
          <th scope="col" className={head}>
            {t.columnOrder}
          </th>
          <th scope="col" className={head}>
            {t.columnOn}
          </th>
          <th scope="col" className={head}>
            {t.columnName}
          </th>
          <th scope="col" className={`${head} pr-[18px] text-right`}>
            {t.columnEntries}
          </th>
          <th scope="col" className={`${head} pr-[18px] text-right`}>
            {t.columnSize}
          </th>
        </tr>
      </thead>
      <tbody>
        {dictionaries.map((dictionary, i) => (
          <tr key={dictionary.id} className={dictionary.enabled ? "" : "text-text-secondary"}>
            <td className={cell}>
              <span className="flex items-center">
                <IconButton
                  label={t.moveUp(dictionary.title)}
                  disabled={busy || i === 0}
                  onClick={() => actions.move(dictionary.id, "up")}
                >
                  <path d="M4 10l4-4 4 4" />
                </IconButton>
                <IconButton
                  label={t.moveDown(dictionary.title)}
                  disabled={busy || i === dictionaries.length - 1}
                  onClick={() => actions.move(dictionary.id, "down")}
                >
                  <path d="M4 6l4 4 4-4" />
                </IconButton>
              </span>
            </td>
            <td className={cell}>
              <input
                type="checkbox"
                role="switch"
                aria-label={t.enable(dictionary.title)}
                checked={dictionary.enabled}
                disabled={busy}
                onChange={(event) => actions.setEnabled(dictionary.id, event.target.checked)}
                className="m-0 size-[15px] align-middle accent-[var(--ink)]"
              />
            </td>
            <th scope="row" title={dictionary.path} className={`${cell} w-full text-left font-semibold whitespace-normal`}>
              <b
                aria-hidden="true"
                className="mr-2.5 inline-grid size-[22px] place-items-center rounded-l-[11px] bg-notch align-middle font-serif text-[12px] font-bold text-notch-text"
              >
                {markOf(dictionary.title)}
              </b>
              {dictionary.title}
              <IndexNote dictionary={dictionary} />
            </th>
            <td className={`${cell} pr-[18px] text-right`}>{formatCount(dictionary.entryCount, locale)}</td>
            <td className={`${cell} pr-[18px] text-right`}>{formatSize(dictionary.size, locale)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Indexing in progress: which Dictionary, how far, and a way to stop. */
function IndexingProgress({ indexing, onCancel }: { indexing: Indexing; onCancel: () => void }) {
  const { t, locale } = useI18n();
  const share = indexing.total > 0 ? Math.min(indexing.done / indexing.total, 1) : 0;
  const label = t.indexingLabel(indexing.title);
  return (
    <div className="flex max-w-[860px] flex-wrap items-center gap-x-3.5 gap-y-1.5 pt-3 text-label text-text-secondary">
      <span className="min-w-0 truncate">
        {label} · {new Intl.NumberFormat(locale, { style: "percent" }).format(share)}
        {indexing.waiting > 0 ? ` · ${t.indexingWaiting(indexing.waiting)}` : null}
      </span>
      <progress
        aria-label={label}
        value={indexing.done}
        max={Math.max(indexing.total, 1)}
        className="h-1 w-40 flex-none accent-[var(--ink)]"
      />
      <OutlineButton onClick={onCancel}>{t.stopIndexing}</OutlineButton>
    </div>
  );
}

/** A Dictionary that does not answer yet because its index is not built, and why. */
function IndexNote({ dictionary }: { dictionary: Dictionary }) {
  const { t } = useI18n();
  const index = dictionary.index;
  if (index.state === "ready") return null;
  return (
    <span className="ml-2.5 text-[12px] font-normal text-text-secondary">
      {index.state === "pending" ? t.notIndexed : t.indexFailed(index.detail)}
    </span>
  );
}

/** What an empty Library says: how to add a Library Folder. */
export function EmptyLibrary({ action }: { action?: ReactNode }) {
  const { t } = useI18n();
  return (
    <div className="max-w-[860px] py-lg text-[14px]">
      <p>{t.emptyLibrary}</p>
      <p className="pt-1.5 text-[13px] text-text-secondary">{t.emptyLibraryHow}</p>
      {action ? <div className="pt-3">{action}</div> : null}
    </div>
  );
}

export function describeFailure(t: Catalogue, reason: FailureReason): string {
  switch (reason.kind) {
    case "unreadable":
      return t.reasonUnreadable(reason.detail);
    case "notADictionary":
      return t.reasonNotADictionary;
    case "unsupportedVersion":
      return t.reasonUnsupportedVersion(reason.detail);
    case "damaged":
      return t.reasonDamaged;
    case "needsRegistrationCode":
      return t.reasonNeedsRegistrationCode;
    case "unsupported":
      return t.reasonUnsupported(reason.detail);
  }
}

function SubHeading({ children }: { children: ReactNode }) {
  return <h2 className="mt-5 mb-1.5 text-[13px] font-bold">{children}</h2>;
}

/** The outline button from DESIGN.md: 28px, 1px ink border, square-cut. */
export function OutlineButton({
  children,
  onClick,
  disabled = false,
}: {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="h-7 rounded-sharp border border-rule px-2.5 text-body text-text hover:bg-shade active:translate-y-px disabled:opacity-50 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className="grid size-6.5 place-items-center rounded-sharp text-text-secondary hover:bg-shade hover:text-text disabled:opacity-30 disabled:hover:bg-transparent"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.5]" viewBox="0 0 16 16" aria-hidden="true" strokeLinecap="round" strokeLinejoin="round">
        {children}
      </svg>
    </button>
  );
}
