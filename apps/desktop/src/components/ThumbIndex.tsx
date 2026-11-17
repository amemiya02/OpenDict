import { useEffect, useEffectEvent, useRef, useState, type CSSProperties, type RefObject } from "react";
import { useI18n } from "../i18n";
import { markOf } from "../lib/format";
import { shortcutOfEvent } from "../lib/keymap";

/** The element id of a Dictionary's section on the page, which its notch links to. */
export function sectionId(dictionaryId: string): string {
  return `dictionary-${dictionaryId}`;
}

export interface Notch {
  dictionaryId: string;
  title: string;
}

/** How far below the top of the page a section's head must rise to become the current one. */
const READING_LINE = 64;

const BY_HAND = ["wheel", "pointerdown", "keydown", "touchstart"] as const;

/**
 * The thumb index cut into the fore-edge: one notch per answering Dictionary, in page
 * order. The current notch follows the page's scroll position; a click or Alt + digit
 * jumps to that Dictionary. Notches seat in from the edge when the index mounts (one per
 * Lookup), unless the reader asked for reduced motion.
 */
export function ThumbIndex({
  notches,
  unanswered,
  page,
}: {
  notches: Notch[];
  /** How many of the Dictionaries asked had no Entry. */
  unanswered: number;
  /** The scrolling page the sections are on. */
  page: RefObject<HTMLElement | null>;
}) {
  const { t } = useI18n();
  const [current, setCurrent] = useState(0);
  // The notch last jumped to. A short last section can never rise to the reading line, so at
  // the foot of the page the jump decides which notch is current.
  const jumped = useRef<number | null>(null);

  /** Makes current the last section whose head has risen to the reading line. */
  const follow = useEffectEvent(() => {
    const scroller = page.current;
    if (!scroller) return;
    const line = scroller.getBoundingClientRect().top + READING_LINE;
    let reached = 0;
    notches.forEach((notch, index) => {
      const section = document.getElementById(sectionId(notch.dictionaryId));
      if (section && section.getBoundingClientRect().top <= line) reached = index;
    });
    const atFoot = scroller.scrollTop > 0 && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 1;
    if (atFoot) reached = jumped.current !== null && jumped.current >= reached ? jumped.current : notches.length - 1;
    setCurrent(reached);
  });

  function jump(index: number) {
    const notch = notches[index];
    const section = notch && document.getElementById(sectionId(notch.dictionaryId));
    if (!section) return;
    const calm = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    jumped.current = index;
    setCurrent(index);
    section.scrollIntoView({ behavior: calm ? "auto" : "smooth", block: "start" });
    section.focus({ preventScroll: true });
  }
  const jumpByKey = useEffectEvent(jump);

  // Follow the reader's scrolling. Scrolling by hand (wheel, scroll bar, keys) ends a jump.
  useEffect(() => {
    const scroller = page.current;
    if (!scroller) return;
    const onScroll = () => follow();
    const byHand = () => {
      jumped.current = null;
    };
    scroller.addEventListener("scroll", onScroll, { passive: true });
    for (const type of BY_HAND) scroller.addEventListener(type, byHand, { passive: true });
    return () => {
      scroller.removeEventListener("scroll", onScroll);
      for (const type of BY_HAND) scroller.removeEventListener(type, byHand);
    };
  }, [page]);

  // Alt + 1–9 jumps to the nth Dictionary, wherever the focus is in the window.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const shortcut = event.defaultPrevented ? null : shortcutOfEvent(event);
      if (shortcut?.kind !== "jump") return;
      event.preventDefault();
      jumpByKey(shortcut.index);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <nav aria-label={t.jumpToDictionary} className="pt-[26px]">
      {notches.map((notch, index) => (
        <a
          key={notch.dictionaryId}
          href={`#${sectionId(notch.dictionaryId)}`}
          title={`Alt+${index + 1}`}
          aria-current={index === current}
          onClick={(event) => {
            event.preventDefault();
            jump(index);
          }}
          style={{ "--i": index } as CSSProperties}
          className="group mb-[7px] flex h-[46px] items-center justify-end gap-2.5 pl-2.5 text-right text-label leading-[1.3] text-text-secondary no-underline hover:text-text focus-visible:-outline-offset-2 aria-[current=true]:font-bold aria-[current=true]:text-text max-[1100px]:pl-1"
        >
          <span className="line-clamp-2 max-[1100px]:hidden">{notch.title}</span>
          <b className="grid h-[46px] w-[42px] flex-none place-items-center rounded-l-[23px] bg-notch font-serif text-[17px] font-bold text-notch-text group-hover:-translate-x-[3px] group-aria-[current=true]:bg-here group-aria-[current=true]:text-here-text motion-safe:animate-seat motion-safe:[animation-delay:calc(var(--i)*28ms)] motion-safe:transition-[translate,background-color] motion-safe:duration-180 motion-safe:ease-out-expo">
            {markOf(notch.title)}
          </b>
        </a>
      ))}
      {unanswered > 0 ? (
        <p className="pt-1.5 pr-3 pl-2.5 text-right text-label text-text-secondary max-[1100px]:hidden">{t.withoutEntry(unanswered)}</p>
      ) : null}
    </nav>
  );
}
