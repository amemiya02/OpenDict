/**
 * The words visited in this window, for Back and Forward. Unlike History it is not kept
 * between sessions and it keeps the order the reader went in, repeats included.
 */
export interface Trail {
  words: string[];
  /** Index of the word on the page, or -1 before the first visit. */
  at: number;
}

export const emptyTrail: Trail = { words: [], at: -1 };

/** The reader went to `word` (not by Back or Forward): the words ahead are dropped. */
export function visit(trail: Trail, word: string): Trail {
  if (trail.words[trail.at] === word) return trail;
  const words = [...trail.words.slice(0, trail.at + 1), word];
  return { words, at: words.length - 1 };
}

export function canStep(trail: Trail, by: -1 | 1): boolean {
  const next = trail.at + by;
  return trail.at >= 0 && next >= 0 && next < trail.words.length;
}

/** One step back (-1) or forward (1), or `null` when there is nowhere to go. */
export function step(trail: Trail, by: -1 | 1): { trail: Trail; word: string } | null {
  if (!canStep(trail, by)) return null;
  const at = trail.at + by;
  return { trail: { ...trail, at }, word: trail.words[at]! };
}
