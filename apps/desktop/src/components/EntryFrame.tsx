import { useI18n } from "../i18n";

/**
 * The only element that ever receives Entry HTML. The frame is sandboxed with no
 * allowances: no scripts, no forms, no top navigation, and an opaque origin, so the
 * Entry cannot reach the app, Tauri IPC or user data. Its own CSP blocks every
 * network fetch. The app sets no type inside it (the Hands-Off Entry Rule).
 */
export function EntryFrame({ dictionaryTitle, html }: { dictionaryTitle: string; html: string }) {
  const { t } = useI18n();
  return (
    <iframe
      sandbox=""
      title={t.entryFrameTitle(dictionaryTitle)}
      srcDoc={entryDocument(html)}
      className="block h-[min(60vh,480px)] w-full max-w-[860px] border-0 bg-transparent"
    />
  );
}

const ENTRY_CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:";

/** Wraps untrusted Entry HTML in a document of its own, under a CSP that allows no network access. */
export function entryDocument(html: string): string {
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${ENTRY_CSP}"></head><body>${html}</body></html>`;
}
