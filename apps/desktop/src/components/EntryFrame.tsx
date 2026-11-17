import { useCallback, useEffect, useEffectEvent, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { linkedHeadword, soundUrl } from "../lib/entryLinks";
import { keysFromFrame, press } from "../lib/keymap";
import { readerJustActed } from "../lib/sound";

/** Height before the Entry document has reported its own. */
const INITIAL_HEIGHT = 120;
/** A frame never grows past this, whatever its document claims. */
const MAX_HEIGHT = 200_000;
/**
 * The dark theme's recolouring: the frame is inverted as a picture, so dark type on the
 * Entry's (transparent) page turns paper-coloured on the navy page, and hues stay roughly
 * where they were. 0.92 rather than 1 lands black type on Night Paper Ink, not pure white.
 */
const RECOLOUR = "invert(0.92) hue-rotate(180deg)";

/**
 * The only element that ever shows Entry content. The document comes from the Entry
 * scheme, which sends it with a content security policy that sandboxes it again and keeps
 * it off the network unless the reader allowed it. The frame may run the Dictionary's
 * scripts but has an opaque origin: no access to the app, Tauri IPC, storage or other
 * Dictionaries. The app sets no type inside it (the Hands-Off Entry Rule).
 *
 * The frame's document can say five things, by message, and nothing else: its height,
 * an `entry://` link the reader clicked (`onLink` gets the headword), a `sound://` link
 * the reader clicked (`onSound` gets a URL beneath this frame's own Dictionary), Esc
 * pressed while the focus is in it (`onEscape`, which the Mini Window closes on) and a key
 * the reader pressed in it that is one of the window's shortcuts (lib/keymap.ts), which
 * the frame presses again on itself, so the window answers it as if pressed there.
 * Messages from any other window, of any other shape, or (for links and keys) with no
 * reader action in this frame behind them are dropped.
 *
 * It also tells the document the reader's base text size when it changes, and in the
 * dark theme recolours the frame from outside with a filter, never touching the document.
 */
export function EntryFrame({
  dictionaryTitle,
  src,
  onLink,
  onSound,
  textSize,
  recolour = false,
  onEscape,
}: {
  dictionaryTitle: string;
  src: string;
  onLink?: (headword: string) => void;
  onSound?: (url: string) => void;
  /** The reader's base text size; the Entry scheme serves documents with it already. */
  textSize: number;
  recolour?: boolean;
  onEscape?: () => void;
}) {
  const { t } = useI18n();
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(INITIAL_HEIGHT);

  /**
   * Whether the reader has just clicked (or pressed a key) inside this frame: focus is in
   * it and the reader acted a moment ago. A script cannot fake either, so a Dictionary
   * cannot start Lookups or sounds on its own, say as soon as its Entry loads.
   */
  function readerActedHere(): boolean {
    return frame.current !== null && document.activeElement === frame.current && readerJustActed();
  }

  const onMessage = useEffectEvent((event: MessageEvent) => {
    if (!frame.current || event.source !== frame.current.contentWindow) return;
    const data: unknown = event.data;
    if (typeof data !== "object" || data === null) return;
    const message = data as { opendict?: unknown; height?: unknown; href?: unknown };
    switch (message.opendict) {
      case "size":
        if (typeof message.height !== "number" || !Number.isFinite(message.height)) return;
        setHeight(Math.min(Math.max(Math.ceil(message.height), 0), MAX_HEIGHT));
        return;
      case "link": {
        if (typeof message.href !== "string" || !readerActedHere()) return;
        const headword = linkedHeadword(message.href);
        if (headword !== null) onLink?.(headword);
        return;
      }
      case "key": {
        const keys = keysFromFrame(message as Record<string, unknown>);
        if (keys && readerActedHere()) press(keys, frame.current);
        return;
      }
      case "escape":
        // Esc is no user activation; that the reader's focus is in this frame is enough
        // for something as harmless as closing the Mini Window.
        if (document.activeElement === frame.current) onEscape?.();
        return;
      case "sound": {
        if (typeof message.href !== "string" || !/^sound:\/\//i.test(message.href) || !readerActedHere()) return;
        const url = soundUrl(src, message.href);
        if (url !== null) onSound?.(url);
        return;
      }
    }
  });

  useEffect(() => {
    const listener = (event: MessageEvent) => onMessage(event);
    window.addEventListener("message", listener);
    return () => window.removeEventListener("message", listener);
  }, []);

  // The document was served with the size of the moment; later changes reach it as a
  // message, so the Entry keeps its place and state. The origin is opaque, hence "*":
  // the message holds nothing but the number.
  const postTextSize = useCallback(() => {
    frame.current?.contentWindow?.postMessage({ opendict: "textSize", size: textSize }, "*");
  }, [textSize]);
  const served = useRef(textSize);
  useEffect(() => {
    if (served.current === textSize) return;
    served.current = textSize;
    postTextSize();
  }, [textSize, postTextSize]);

  return (
    <iframe
      ref={frame}
      sandbox="allow-scripts"
      title={t.entryFrameTitle(dictionaryTitle)}
      src={src}
      onLoad={postTextSize}
      style={{ height: `${height}px`, filter: recolour ? RECOLOUR : undefined }}
      // Light, like the document inside: with matching schemes the frame stays transparent.
      className="block w-full max-w-[860px] border-0 bg-transparent [color-scheme:light]"
    />
  );
}
