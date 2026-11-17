import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";

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
 * Dictionaries. The app sets no type inside it (the Hands-Off Entry Rule); it sizes the
 * frame to the height its document reports, tells the document the reader's base text
 * size when it changes, and in the dark theme recolours the frame from outside with a
 * filter, never touching the document.
 */
export function EntryFrame({
  dictionaryTitle,
  src,
  textSize,
  recolour = false,
}: {
  dictionaryTitle: string;
  src: string;
  /** The reader's base text size; the Entry scheme serves documents with it already. */
  textSize: number;
  recolour?: boolean;
}) {
  const { t } = useI18n();
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(INITIAL_HEIGHT);

  useEffect(() => {
    function onMessage(event: MessageEvent) {
      if (!frame.current || event.source !== frame.current.contentWindow) return;
      const data: unknown = event.data;
      if (typeof data !== "object" || data === null) return;
      const { opendict, height } = data as { opendict?: unknown; height?: unknown };
      if (opendict !== "size" || typeof height !== "number" || !Number.isFinite(height)) return;
      setHeight(Math.min(Math.max(Math.ceil(height), 0), MAX_HEIGHT));
    }
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
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
