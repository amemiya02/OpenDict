import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";

/** Height before the Entry document has reported its own. */
const INITIAL_HEIGHT = 120;
/** A frame never grows past this, whatever its document claims. */
const MAX_HEIGHT = 200_000;

/**
 * The only element that ever shows Entry content. The document comes from the Entry
 * scheme, which sends it with a content security policy that sandboxes it again and keeps
 * it off the network unless the reader allowed it. The frame may run the Dictionary's
 * scripts but has an opaque origin: no access to the app, Tauri IPC, storage or other
 * Dictionaries. The app sets no type inside it (the Hands-Off Entry Rule); it only sizes
 * the frame to the height its document reports.
 */
export function EntryFrame({ dictionaryTitle, src }: { dictionaryTitle: string; src: string }) {
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

  return (
    <iframe
      ref={frame}
      sandbox="allow-scripts"
      title={t.entryFrameTitle(dictionaryTitle)}
      src={src}
      style={{ height: `${height}px` }}
      className="block w-full max-w-[860px] border-0 bg-transparent"
    />
  );
}
