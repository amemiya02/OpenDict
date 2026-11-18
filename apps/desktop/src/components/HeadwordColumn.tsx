import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useI18n } from "../i18n";
/** One row: a headword and, for suggestions, how many Dictionaries have it. */
export interface ColumnItem {
  headword: string;
  count?: number;
}

/** What the column lists: suggestions for what the reader typed, History, or the chosen Word Book. */
export type ColumnList = "suggestions" | "history" | "wordBook";

/** Height of one index row (DESIGN.md: index-row). */
const ROW = 26;
/** Load more headwords when the reader scrolls within this many rows of the end. */
const NEAR_END = 20;

/** Kana mark a headword as Japanese, so it is set in the Japanese serif. */
const KANA = /[぀-ヿ]/;

export const LIST_ID = "headwords";
export const COLUMN_ID = "headword-column";
export const optionId = (index: number) => `headword-${index}`;

/**
 * The headword column: a title, an optional note line, and the list of headwords
 * with a count for those found in multiple Dictionaries. The search field drives it from the keyboard; rows can
 * also be clicked. The History tab lists committed Lookups instead, newest first, with a
 * way to clear them; the Word Book tab lists the chosen Word Book, newest first.
 */
export function HeadwordColumn({
  items,
  selected,
  note,
  hasQuery,
  view,
  list = "suggestions",
  onPick,
  onNeedMore,
  onShowSuggestions,
  onShowHistory,
  onClearHistory,
  onShowWordBook,
  onToggleLibrary,
  open = false,
}: {
  items: ColumnItem[];
  /** The current row, or -1. Nothing in the column is current while the Library is open. */
  selected: number;
  note: ReactNode;
  hasQuery: boolean;
  view: "lookup" | "library" | "wordBook";
  list?: ColumnList;
  onPick: (index: number) => void;
  onNeedMore: () => void;
  onShowSuggestions: () => void;
  onShowHistory: () => void;
  onClearHistory: () => void;
  onShowWordBook: () => void;
  onToggleLibrary: () => void;
  /** In a narrow window the column is an overlay, shown over the page only while open. */
  open?: boolean;
}) {
  const { t } = useI18n();
  const listRef = useRef<HTMLUListElement>(null);
  const current = view === "lookup" ? selected : -1;
  const title = {
    suggestions: hasQuery ? t.lookupSuggestions : t.headwordIndex,
    history: t.history,
    wordBook: t.wordBook,
  }[list];

  const needMore = useRef(onNeedMore);
  useEffect(() => {
    needMore.current = onNeedMore;
  }, [onNeedMore]);

  const checkNearEnd = useCallback(() => {
    const list = listRef.current;
    if (!list || list.clientHeight === 0) return;
    if (list.scrollTop + list.clientHeight >= list.scrollHeight - NEAR_END * ROW) needMore.current();
  }, []);

  useLayoutEffect(checkNearEnd, [checkNearEnd, items]);

  // A taller or shorter window shows more or fewer rows.
  useEffect(() => {
    const list = listRef.current;
    if (!list || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(checkNearEnd);
    observer.observe(list);
    return () => observer.disconnect();
  }, [checkNearEnd]);

  useEffect(() => {
    listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [current, items]);

  // A fresh list (a new query) starts at its top.
  const first = items[0]?.headword;
  useLayoutEffect(() => {
    listRef.current?.scrollTo?.({ top: 0 });
  }, [first]);

  return (
    <aside
      id={COLUMN_ID}
      data-open={open || undefined}
      className="headword-column row-span-2 flex min-h-0 flex-col border-r border-rule select-none max-[620px]:hidden max-[620px]:data-open:fixed max-[620px]:data-open:top-12 max-[620px]:data-open:bottom-0 max-[620px]:data-open:left-0 max-[620px]:data-open:z-10 max-[620px]:data-open:flex max-[620px]:data-open:w-[min(280px,84vw)]"
      aria-label={t.headwordColumn}
    >
      <h2 className="mx-lg border-b-[3px] border-double border-rule pt-md pb-1.5 font-sans text-[13px] font-bold">
        {title}
      </h2>
      {note ? <p className="px-lg pt-sm pb-1.5 text-[12px] text-text-secondary">{note}</p> : null}
      <ul
        ref={listRef}
        id={LIST_ID}
        role="listbox"
        aria-label={t.headwords}
        onScroll={checkNearEnd}
        // Keep the caret in the search field when a row is clicked.
        onMouseDown={(event) => event.preventDefault()}
        className={`min-h-0 flex-1 overflow-y-auto outline-0 ${items.length === 0 ? "hidden" : ""}`}
      >
        {items.map((item, index) => {
          const count = item.count ?? 0;
          const showCount = count >= 2;
          return (
            <li
              key={item.headword}
              id={optionId(index)}
              role="option"
              aria-selected={index === current}
              aria-label={showCount ? `${item.headword}, ${t.answered(count)}` : item.headword}
              data-headword={item.headword}
              lang={KANA.test(item.headword) ? "ja" : undefined}
              onClick={() => onPick(index)}
              className="flex h-[26px] cursor-default items-center gap-sm pr-lg pl-[30px] font-serif text-index hover:bg-[color-mix(in_srgb,var(--paper-shade)_55%,transparent)] aria-selected:bg-shade aria-selected:pl-lg aria-selected:font-bold"
            >
              {index === current ? <i className="size-1.5 flex-none bg-here" aria-hidden="true" /> : null}
              <span className="truncate">{item.headword}</span>
              {showCount ? (
                <small className="ml-auto shrink-0 whitespace-nowrap font-sans text-[11px] font-normal text-text-secondary tabular-nums">
                  {t.suggestionCount(count)}
                </small>
              ) : null}
            </li>
          );
        })}
      </ul>
      {items.length === 0 ? (
        <p className="flex-1 px-lg py-3.5 text-text-secondary">
          {list === "history" ? t.noLookupsYet : list === "wordBook" ? t.wordCount(0) : t.noMatchingHeadwords}
        </p>
      ) : null}
      {list === "history" && view === "lookup" && items.length > 0 ? <ClearHistory onClear={onClearHistory} /> : null}
      <div className="flex border-t border-rule">
        <ColumnTab pressed={view === "lookup" && list === "suggestions"} onClick={onShowSuggestions}>
          {t.suggestions}
        </ColumnTab>
        <ColumnTab pressed={view === "lookup" && list === "history"} onClick={onShowHistory}>
          {t.history}
        </ColumnTab>
        <ColumnTab pressed={view !== "library" && list === "wordBook"} onClick={onShowWordBook}>
          {t.wordBook}
        </ColumnTab>
        <ColumnTab pressed={view === "library"} onClick={onToggleLibrary}>
          {t.libraryAndSettings}
        </ColumnTab>
      </div>
    </aside>
  );
}

/** Clearing History asks once more in place, so one stray click cannot lose it. */
function ClearHistory({ onClear }: { onClear: () => void }) {
  const { t } = useI18n();
  const [asking, setAsking] = useState(false);
  const button =
    "h-6.5 flex-none rounded-sharp px-sm text-[12px] text-text-secondary hover:bg-shade hover:text-text";
  return (
    <div className="flex min-h-9 items-center justify-end gap-xs border-t border-hair px-sm text-[12px]">
      {asking ? (
        <>
          <span className="mr-auto min-w-0 pl-sm text-text-secondary">{t.clearHistoryQuestion}</span>
          <button type="button" className={button} onClick={() => setAsking(false)}>
            {t.keep}
          </button>
          <button
            type="button"
            className={`${button} border border-rule text-text`}
            onClick={() => {
              setAsking(false);
              onClear();
            }}
          >
            {t.clear}
          </button>
        </>
      ) : (
        <button type="button" className={button} onClick={() => setAsking(true)}>
          {t.clearHistory}
        </button>
      )}
    </div>
  );
}

function ColumnTab({ pressed, onClick, children }: { pressed: boolean; onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      className="h-9 min-w-min flex-auto px-1 text-[12px] leading-tight text-text-secondary hover:bg-shade hover:text-text aria-pressed:text-text aria-pressed:shadow-[inset_0_2px_0_var(--rule)] max-[860px]:px-0.5"
    >
      {children}
    </button>
  );
}
