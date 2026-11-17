import { useCallback, useRef, useState } from "react";
import { headwordsAfter, suggest, type Suggestion, type Suggestions } from "./ipc";

/** How many headwords the column asks for at a time. */
export const PAGE_SIZE = 100;

export interface HeadwordColumnState extends Omit<Suggestions, "items"> {
  items: Suggestion[];
  /** Index of the current suggestion, or -1 when none is current. */
  selected: number;
  /** There is nothing to load after the last item. */
  exhausted: boolean;
  /** The query the items answer. */
  query: string;
}

const initial: HeadwordColumnState = {
  items: [],
  lemma: null,
  near: false,
  miss: false,
  browse: true,
  selected: -1,
  exhausted: true,
  query: "",
};

/**
 * The headword column's state: the suggestions for what the reader typed, the current
 * one, and more headwords loaded on demand as the reader moves or scrolls past the end.
 * Answers that arrive after a newer query are dropped.
 */
export function useHeadwordColumn() {
  const [state, setState] = useState(initial);
  const latest = useRef(initial);
  const generation = useRef(0);
  const loading = useRef<Promise<void> | null>(null);

  const update = useCallback((next: HeadwordColumnState) => {
    latest.current = next;
    setState(next);
  }, []);

  /** Asks for the suggestions for `query`; resolves to the current headword, if any. */
  const refresh = useCallback(
    async (query: string): Promise<{ headword: string | null; miss: boolean; items: Suggestion[] } | null> => {
      const ticket = ++generation.current;
      loading.current = null;
      const answer = await suggest(query, PAGE_SIZE);
      if (ticket !== generation.current) return null;
      // Browsing the index or looking at a miss, nothing is current until the reader picks.
      const selected = answer.browse || answer.miss || answer.items.length === 0 ? -1 : 0;
      // Close spellings are a short list of their own; anything else may continue.
      const exhausted = answer.near || answer.items.length < PAGE_SIZE;
      update({ ...answer, selected, exhausted, query });
      return { headword: answer.items[selected]?.headword ?? null, miss: answer.miss, items: answer.items };
    },
    [update],
  );

  /** Loads the headwords that follow the last one shown. */
  const loadMore = useCallback((): Promise<void> => {
    const now = latest.current;
    const last = now.items.at(-1);
    if (now.exhausted || !last) return Promise.resolve();
    if (loading.current) return loading.current;
    const ticket = generation.current;
    const request = headwordsAfter(now.query, last.headword, PAGE_SIZE)
      .then((more) => {
        if (ticket !== generation.current) return;
        const current = latest.current;
        update({ ...current, items: [...current.items, ...more], exhausted: more.length < PAGE_SIZE });
      })
      .finally(() => {
        if (loading.current === request) loading.current = null;
      });
    loading.current = request;
    return request;
  }, [update]);

  /** Moves the current suggestion by `by` rows; resolves to the new current headword. */
  const move = useCallback(
    async (by: number): Promise<string | null> => {
      let now = latest.current;
      if (now.items.length === 0) return null;
      let next = now.selected < 0 ? (by > 0 ? 0 : now.items.length - 1) : now.selected + by;
      if (next >= now.items.length) {
        const ticket = generation.current;
        await loadMore();
        if (ticket !== generation.current) return null;
        now = latest.current;
      }
      next = Math.max(0, Math.min(next, now.items.length - 1));
      update({ ...now, selected: next });
      return now.items[next]!.headword;
    },
    [loadMore, update],
  );

  /** Makes row `index` current; resolves to its headword. */
  const select = useCallback(
    (index: number): string | null => {
      const now = latest.current;
      const item = now.items[index];
      if (!item) return null;
      update({ ...now, selected: index });
      return item.headword;
    },
    [update],
  );

  return { state, refresh, loadMore, move, select };
}
