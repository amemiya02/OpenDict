/**
 * A wildcard pattern to suggest when nothing was found for `typed`: its first two
 * characters and `*`, or all of it and `*` when it is that short.
 */
export function wildcardHint(typed: string): string {
  const chars = Array.from(typed.trim());
  return `${chars.slice(0, 2).join("")}*`;
}
