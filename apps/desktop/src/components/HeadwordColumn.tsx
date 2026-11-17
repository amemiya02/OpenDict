import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import type { Suggestion } from "../lib/ipc";

/** Height of one index row (DESIGN.md: index-row). Guide words are worked out from it. */
const ROW = 26;
/** Load more headwords when the reader scrolls within this many rows of the end. */
const NEAR_END = 20;

/** Kana mark a headword as Japanese, so it is set in the Japanese serif. */
const KANA = /[぀-ヿ]/;

export const LIST_ID = "headwords";
export const COLUMN_ID = "headword-column";
export const optionId = (index: number) => `headword-${index}`;

/**
 * The headword column: guide words (the first and last headwords in view) over a double
 * rule, an optional note line, the headwords themselves with how many Dictionaries have
 * each, and the column's tabs. The search field drives it from the keyboard; rows can
 * also be clicked.
 */
export function HeadwordColumn({
  items,
  selected,
  note,
  view,
  onPick,
  onNeedMore,
  onShowSuggestions,
  onToggleLibrary,
  open = false,
}: {
  items: Suggestion[];
  /** The current row, or -1. Nothing in the column is current while the Library is open. */
  selected: number;
  note: string | null;
  view: "lookup" | "library";
  onPick: (index: number) => void;
  onNeedMore: () => void;
  onShowSuggestions: () => void;
  onToggleLibrary: () => void;
  /** In a narrow window the column is an overlay, shown over the page only while open. */
  open?: boolean;
}) {
  const { t } = useI18n();
  const listRef = useRef<HTMLUListElement>(null);
  const [inView, setInView] = useState<[number, number] | null>(null);
  const current = view === "lookup" ? selected : -1;

  const needMore = useRef(onNeedMore);
  useEffect(() => {
    needMore.current = onNeedMore;
  }, [onNeedMore]);

  // Which rows are in view. Without a layout (nothing measured yet) the guide words are
  // the first and last headwords loaded.
  const measure = useCallback(() => {
    const list = listRef.current;
    if (!list || list.clientHeight === 0) {
      setInView(null);
      return;
    }
    const first = Math.floor(list.scrollTop / ROW);
    const last = Math.floor((list.scrollTop + list.clientHeight - 1) / ROW);
    setInView((now) => (now?.[0] === first && now[1] === last ? now : [first, last]));
    if (list.scrollTop + list.clientHeight >= list.scrollHeight - NEAR_END * ROW) needMore.current();
  }, []);

  useLayoutEffect(measure, [measure, items]);

  // A taller or shorter window shows more or fewer rows.
  useEffect(() => {
    const list = listRef.current;
    if (!list || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(list);
    return () => observer.disconnect();
  }, [measure]);

  useEffect(() => {
    listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [current, items]);

  // A fresh list (a new query) starts at its top.
  const first = items[0]?.headword;
  useLayoutEffect(() => {
    listRef.current?.scrollTo?.({ top: 0 });
  }, [first]);

  const lastIndex = items.length - 1;
  const guide: [Suggestion, Suggestion] | null =
    items.length > 0
      ? [items[Math.min(inView?.[0] ?? 0, lastIndex)]!, items[Math.min(inView?.[1] ?? lastIndex, lastIndex)]!]
      : null;

  return (
    <aside
      id={COLUMN_ID}
      data-open={open || undefined}
      className="headword-column row-span-2 flex min-h-0 flex-col border-r border-rule select-none max-[620px]:hidden max-[620px]:data-open:fixed max-[620px]:data-open:top-12 max-[620px]:data-open:bottom-0 max-[620px]:data-open:left-0 max-[620px]:data-open:z-10 max-[620px]:data-open:flex max-[620px]:data-open:w-[min(280px,84vw)]"
      aria-label={t.headwordColumn}
    >
      <div
        data-testid="guide-words"
        aria-hidden="true"
        className="mx-lg flex justify-between gap-md border-b-[3px] border-double border-rule pt-md pb-1.5 font-serif text-[13px] font-bold"
      >
        {guide ? (
          <>
            <span className="truncate" lang={KANA.test(guide[0].headword) ? "ja" : undefined}>{guide[0].headword}</span>
            <span className="truncate" lang={KANA.test(guide[1].headword) ? "ja" : undefined}>{guide[1].headword}</span>
          </>
        ) : (
          <span>{t.suggestions}</span>
        )}
      </div>
      {note ? <p className="px-lg pt-sm pb-1.5 text-[12px] text-text-secondary">{note}</p> : null}
      <ul
        ref={listRef}
        id={LIST_ID}
        role="listbox"
        aria-label={t.headwords}
        onScroll={measure}
        // Keep the caret in the search field when a row is clicked.
        onMouseDown={(event) => event.preventDefault()}
        className={`min-h-0 flex-1 overflow-y-auto outline-0 ${items.length === 0 ? "hidden" : ""}`}
      >
        {items.map((item, index) => (
          <li
            key={item.headword}
            id={optionId(index)}
            role="option"
            aria-selected={index === current}
            aria-label={`${item.headword}, ${t.answered(item.count)}`}
            data-headword={item.headword}
            lang={KANA.test(item.headword) ? "ja" : undefined}
            onClick={() => onPick(index)}
            className="flex h-[26px] cursor-default items-center gap-sm pr-lg pl-[30px] font-serif text-index hover:bg-[color-mix(in_srgb,var(--paper-shade)_55%,transparent)] aria-selected:bg-shade aria-selected:pl-lg aria-selected:font-bold"
          >
            {index === current ? <i className="size-1.5 flex-none bg-here" aria-hidden="true" /> : null}
            <span className="truncate">{item.headword}</span>
            <small className="ml-auto font-sans text-[11px] font-normal text-text-secondary tabular-nums">{item.count}</small>
          </li>
        ))}
      </ul>
      {items.length === 0 ? <p className="flex-1 px-lg py-3.5 text-text-secondary">{t.noMatchingHeadwords}</p> : null}
      <div className="flex border-t border-rule">
        <ColumnTab pressed={view === "lookup"} onClick={onShowSuggestions}>
          {t.suggestions}
        </ColumnTab>
        <ColumnTab pressed={view === "library"} onClick={onToggleLibrary}>
          {t.library}
        </ColumnTab>
      </div>
    </aside>
  );
}

function ColumnTab({ pressed, onClick, children }: { pressed: boolean; onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      className="h-9 min-w-0 flex-auto truncate px-sm text-[12px] text-text-secondary hover:bg-shade hover:text-text aria-pressed:text-text aria-pressed:shadow-[inset_0_2px_0_var(--rule)]"
    >
      {children}
    </button>
  );
}
