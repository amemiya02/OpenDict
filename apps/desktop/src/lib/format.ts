import type { Locale } from "../i18n";

const UNITS = ["byte", "kilobyte", "megabyte", "gigabyte", "terabyte"] as const;

/** A file size for people: 38 MB, 1.2 GB (binary multiples, as file managers show them). */
export function formatSize(bytes: number, locale: Locale): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: UNITS[unit],
    unitDisplay: "short",
    maximumFractionDigits: value < 10 && unit > 0 ? 1 : 0,
  }).format(value);
}

export function formatCount(n: number, locale: Locale): string {
  return n.toLocaleString(locale);
}

/** The one-character mark a Dictionary carries in the Library and the thumb index. */
export function markOf(title: string): string {
  const [first = "?"] = Array.from(title.trim());
  return first.toLocaleUpperCase();
}
