import { createContext, use, useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { getAppearance, onLanguageChanged, setLanguage as saveLanguage, type Language } from "../lib/ipc";
import { en, type Catalogue } from "./en";
import { ja } from "./ja";
import { zhCN } from "./zh-CN";

export type Locale = Language;

export const catalogues: Record<Locale, Catalogue> = { "zh-CN": zhCN, en, ja };

/** Each interface language by its own name, in the order the app offers them. */
export const LANGUAGES: Record<Locale, string> = { "zh-CN": "简体中文", en: "English", ja: "日本語" };

function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && Object.hasOwn(catalogues, value);
}

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

interface I18n {
  /** The interface language on screen. */
  locale: Locale;
  t: Catalogue;
  /** Switches the interface language at once, and keeps the choice. */
  chooseLanguage: (locale: Locale) => void;
}

const I18nContext = createContext<I18n>({ locale: "en", t: en, chooseLanguage: () => {} });

/**
 * The interface language: `locale` (the system's) until the shell says the reader chose
 * another, and whatever the reader chooses from then on, in this window or another one.
 * It writes the language to `<html lang>`, so CJK text picks the right fonts.
 */
export function I18nProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  const [chosen, setChosen] = useState<Locale | null>(null);
  /** The reader chose here before the stored choice arrived: theirs wins. */
  const touched = useRef(false);
  const current = chosen ?? locale;

  useEffect(() => {
    getAppearance().then(
      (stored) => {
        if (!touched.current && isLocale(stored?.language)) setChosen(stored.language);
      },
      () => {}, // no shell, or no stored choice: the system's language
    );
    const unlisten = onLanguageChanged((language) => setChosen(isLocale(language) ? language : null));
    return () => {
      unlisten.then((stop) => stop(), () => {});
    };
  }, []);

  useEffect(() => {
    document.documentElement.lang = current;
  }, [current]);

  const chooseLanguage = useCallback((language: Locale) => {
    touched.current = true;
    setChosen(language);
    saveLanguage(language).catch(() => {}); // still switched, for this session
  }, []);

  return <I18nContext value={{ locale: current, t: catalogues[current], chooseLanguage }}>{children}</I18nContext>;
}

export function useI18n() {
  return use(I18nContext);
}
