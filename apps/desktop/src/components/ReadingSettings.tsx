import { useId, type ReactNode } from "react";
import { LANGUAGES, useI18n, type Locale } from "../i18n";
import type { Theme } from "../lib/ipc";
import { TEXT_SIZES, ZOOMS, zoomStep, type AppearanceControls } from "../lib/useAppearance";

/**
 * How the page reads: interface language, theme, "as authored" for every Dictionary, text
 * size and zoom. Set
 * like the Library above it: a heading, then rows divided by hairlines, no boxes.
 */
export function ReadingSettings({ controls }: { controls: AppearanceControls }) {
  const { t, locale, followsSystem, chooseLanguage } = useI18n();
  const heading = useId();
  const { appearance, setTheme, setAllAsAuthored, setTextSize, setZoom } = controls;
  const themes: [Theme, string][] = [
    ["system", t.themeSystem],
    ["light", t.themeLight],
    ["dark", t.themeDark],
  ];

  return (
    <section aria-labelledby={heading} className="max-w-[860px]">
      <h2 id={heading} className="mt-5 mb-1.5 text-[13px] font-bold">
        {t.reading}
      </h2>
      <Row label={t.interfaceLanguage}>
        <div role="radiogroup" aria-label={t.interfaceLanguage} className="flex flex-wrap gap-x-3.5 gap-y-1">
          <label className="flex items-center gap-1.5">
            <input type="radio" name="language" checked={followsSystem} onChange={() => chooseLanguage(null)} />
            {t.languageSystem}
          </label>
          {(Object.entries(LANGUAGES) as [Locale, string][]).map(([language, name]) => (
            <label key={language} lang={language} className="flex items-center gap-1.5">
              <input type="radio" name="language" checked={!followsSystem && locale === language} onChange={() => chooseLanguage(language)} />
              {name}
            </label>
          ))}
        </div>
      </Row>
      <Row label={t.theme}>
        <div role="radiogroup" aria-label={t.theme} className="flex flex-wrap gap-x-3.5 gap-y-1">
          {themes.map(([theme, label]) => (
            <label key={theme} className="flex items-center gap-1.5">
              <input
                type="radio"
                name="theme"
                checked={appearance.theme === theme}
                onChange={() => setTheme(theme)}
              />
              {label}
            </label>
          ))}
        </div>
      </Row>
      <Row label={t.darkEntries}>
        <div>
          <label className="flex items-center gap-1.5">
            <input
              type="checkbox"
              checked={appearance.allAsAuthored}
              onChange={(event) => void setAllAsAuthored(event.target.checked).catch(() => {})}
            />
            {t.allAsAuthored}
          </label>
          <p className="text-[12px] text-text-secondary">{t.allAsAuthoredNote}</p>
        </div>
      </Row>
      <Row label={t.textSize}>
        <Stepper
          value={t.pixels(appearance.textSize)}
          down={{ label: t.smallerText, disabled: appearance.textSize <= TEXT_SIZES.min, run: () => setTextSize(appearance.textSize - 1) }}
          up={{ label: t.largerText, disabled: appearance.textSize >= TEXT_SIZES.max, run: () => setTextSize(appearance.textSize + 1) }}
        />
        <span className="text-[12px] text-text-secondary">{t.textSizeNote}</span>
      </Row>
      <Row label={t.zoom}>
        <Stepper
          value={t.percent(appearance.zoom)}
          down={{ label: t.zoomOut, hint: "Ctrl −", disabled: appearance.zoom <= ZOOMS[0], run: () => setZoom(zoomStep(appearance.zoom, -1)) }}
          up={{ label: t.zoomIn, hint: "Ctrl +", disabled: appearance.zoom >= 200, run: () => setZoom(zoomStep(appearance.zoom, 1)) }}
        />
        <button
          type="button"
          onClick={() => setZoom(100)}
          disabled={appearance.zoom === 100}
          className="h-6.5 rounded-sharp px-2 text-[12px] text-text-secondary hover:bg-shade hover:text-text disabled:opacity-50 disabled:hover:bg-transparent"
        >
          {t.zoomReset} <kbd className="font-sans text-label">Ctrl 0</kbd>
        </button>
      </Row>
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-[34px] items-center gap-3.5 border-t border-hair py-1">
      <span className="w-[120px] flex-none text-text-secondary">{label}</span>
      {children}
    </div>
  );
}

interface Step {
  label: string;
  hint?: string;
  disabled: boolean;
  run: () => void;
}

/** − value +, as two square icon buttons around the value in tabular figures. */
function Stepper({ value, down, up }: { value: string; down: Step; up: Step }) {
  return (
    <span className="flex flex-none items-center gap-1">
      <StepButton step={down} glyph="M4 8h8" />
      <span className="min-w-12 text-center tabular-nums">{value}</span>
      <StepButton step={up} glyph="M4 8h8M8 4v8" />
    </span>
  );
}

function StepButton({ step, glyph }: { step: Step; glyph: string }) {
  return (
    <button
      type="button"
      aria-label={step.label}
      title={step.hint ? `${step.label} (${step.hint})` : step.label}
      disabled={step.disabled}
      onClick={step.run}
      className="grid size-6.5 place-items-center rounded-sharp border border-rule text-text hover:bg-shade active:translate-y-px disabled:opacity-40 disabled:hover:bg-transparent"
    >
      <svg className="size-3.5 fill-none stroke-current stroke-[1.4]" viewBox="0 0 16 16" aria-hidden="true">
        <path d={glyph} />
      </svg>
    </button>
  );
}
