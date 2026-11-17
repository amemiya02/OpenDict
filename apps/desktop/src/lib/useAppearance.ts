import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  getAppearance,
  setAllAsAuthored as saveAllAsAuthored,
  setTextSize as saveTextSize,
  setTheme as saveTheme,
  setZoom as saveZoom,
  type Appearance,
  type Theme,
} from "./ipc";
import { shortcutOfEvent } from "./keymap";

/** The defaults, until the shell has said otherwise (and if it cannot). */
const DEFAULTS: Appearance = { theme: "system", allAsAuthored: false, textSize: 16, zoom: 100 };

export const TEXT_SIZES = { min: 10, max: 32 } as const;
/** The zoom steps, as browsers offer them. */
export const ZOOMS = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200] as const;
const MAX_ZOOM = 200;

/** The scheme actually on screen: the reader's choice, or the operating system's. */
export type Scheme = "light" | "dark";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function subscribeToSystem(onChange: () => void) {
  const query = window.matchMedia?.(DARK_QUERY);
  query?.addEventListener("change", onChange);
  return () => query?.removeEventListener("change", onChange);
}

function systemScheme(): Scheme {
  return window.matchMedia?.(DARK_QUERY).matches ? "dark" : "light";
}

/** The next zoom step up (+1) or down (-1) from `zoom`. */
export function zoomStep(zoom: number, by: 1 | -1): number {
  if (by > 0) return ZOOMS.find((step) => step > zoom) ?? MAX_ZOOM;
  return [...ZOOMS].reverse().find((step) => step < zoom) ?? ZOOMS[0];
}

/**
 * The reader's theme, "as authored" default, text size and zoom: loaded from the shell,
 * applied to the document (`data-theme` on <html>, which the stylesheet's `dark` variant
 * reads), and saved on every change. Ctrl/Cmd with + − 0 zoom the page.
 */
export function useAppearance() {
  const [appearance, setAppearance] = useState<Appearance>(DEFAULTS);
  const system = useSyncExternalStore(subscribeToSystem, systemScheme, () => "light" as Scheme);
  const scheme: Scheme = appearance.theme === "system" ? system : appearance.theme;
  /** The latest appearance, for the keyboard handler. */
  const latest = useRef(appearance);
  useEffect(() => {
    latest.current = appearance;
  });

  /** Reads the appearance again (the Mini Window does when summoned: the main window may have changed it). */
  const reload = useCallback(() => {
    getAppearance().then((loaded) => {
      if (loaded) setAppearance(loaded);
    }, () => {});
  }, []);
  useEffect(reload, [reload]);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = scheme;
    root.style.colorScheme = scheme;
  }, [scheme]);

  const setTheme = useCallback((theme: Theme) => {
    setAppearance((current) => ({ ...current, theme }));
    saveTheme(theme).catch(() => {});
  }, []);

  const setAllAsAuthored = useCallback(async (asAuthored: boolean) => {
    await saveAllAsAuthored(asAuthored);
    setAppearance((current) => ({ ...current, allAsAuthored: asAuthored }));
  }, []);

  const setTextSize = useCallback((size: number) => {
    const wanted = Math.min(Math.max(Math.round(size), TEXT_SIZES.min), TEXT_SIZES.max);
    setAppearance((current) => ({ ...current, textSize: wanted }));
    saveTextSize(wanted).then(
      (saved) => setAppearance((current) => ({ ...current, textSize: saved })),
      () => {},
    );
  }, []);

  const setZoom = useCallback((percent: number) => {
    setAppearance((current) => ({ ...current, zoom: percent }));
    saveZoom(percent).then(
      (saved) => setAppearance((current) => ({ ...current, zoom: saved })),
      () => {},
    );
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const zoom = latest.current.zoom;
      const shortcut = shortcutOfEvent(event)?.kind;
      if (shortcut === "zoomIn") setZoom(zoomStep(zoom, 1));
      else if (shortcut === "zoomOut") setZoom(zoomStep(zoom, -1));
      else if (shortcut === "zoomReset") setZoom(100);
      else return;
      event.preventDefault();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setZoom]);

  return { appearance, scheme, setTheme, setAllAsAuthored, setTextSize, setZoom, reload };
}

export type AppearanceControls = ReturnType<typeof useAppearance>;
