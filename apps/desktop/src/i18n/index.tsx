import { createContext, use, type ReactNode } from "react";
import { en, type Catalogue } from "./en";
import { ja } from "./ja";
import { zhCN } from "./zh-CN";

export type Locale = "zh-CN" | "en" | "ja";

export const catalogues: Record<Locale, Catalogue> = { "zh-CN": zhCN, en, ja };

/** Picks the interface language from the system's preferred languages; English if none match. */
export function pickLocale(preferred: readonly string[]): Locale {
  for (const tag of preferred) {
    const lower = tag.toLowerCase();
    if (lower === "zh" || lower.startsWith("zh-cn") || lower.startsWith("zh-hans") || lower.startsWith("zh-sg")) return "zh-CN";
    if (lower === "ja" || lower.startsWith("ja-")) return "ja";
    if (lower === "en" || lower.startsWith("en-")) return "en";
  }
  return "en";
}

const I18nContext = createContext<{ locale: Locale; t: Catalogue }>({ locale: "en", t: en });

export function I18nProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  return <I18nContext value={{ locale, t: catalogues[locale] }}>{children}</I18nContext>;
}

export function useI18n() {
  return use(I18nContext);
}
