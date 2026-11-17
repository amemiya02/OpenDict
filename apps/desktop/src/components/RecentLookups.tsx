import { useI18n } from "../i18n";

/** How many recent lookups the empty page offers. */
const SHOWN = 24;

/** Kana mark a headword as Japanese, so it is set in the Japanese serif. */
const KANA = /[぀-ヿ]/;

/** The words looked up most recently, newest first, offered on the empty page. */
export function RecentLookups({ words, onPick }: { words: string[]; onPick: (word: string) => void }) {
  const { t } = useI18n();
  if (words.length === 0) return null;
  return (
    <>
      <h2 id="recent-lookups" className="mt-5 mb-1.5 text-[13px] font-bold">
        {t.recentLookups}
      </h2>
      <ul aria-labelledby="recent-lookups" className="max-w-[860px] [column-gap:28px] [column-rule:1px_solid_var(--hairline)] [columns:150px] pt-0.5 pb-1.5">
        {words.slice(0, SHOWN).map((word) => (
          <li key={word}>
            <button
              type="button"
              lang={KANA.test(word) ? "ja" : undefined}
              onClick={() => onPick(word)}
              className="-ml-1.5 block h-[26px] w-full truncate rounded-sharp px-1.5 text-left font-serif text-index hover:bg-shade"
            >
              {word}
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}
