import { useEffect, useState, type ReactNode } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { useI18n } from "../i18n";

type UpdateState =
  | { kind: "none" }
  | { kind: "available"; update: Update; failure?: string }
  | { kind: "downloading"; update: Update; received: number; total: number | null }
  | { kind: "installing"; update: Update };

/**
 * Checks GitHub Releases for a newer OpenDict once, when the window opens, and offers it in the
 * status line. A failed check (offline, no release yet) shows nothing: OpenDict works without a network.
 */
export function UpdateNotice() {
  const { t } = useI18n();
  const [state, setState] = useState<UpdateState>({ kind: "none" });

  useEffect(() => {
    let current = true;
    check().then(
      (update) => {
        if (update && current) setState({ kind: "available", update });
      },
      () => {},
    );
    return () => {
      current = false;
    };
  }, []);

  async function install(update: Update) {
    let received = 0;
    let total: number | null = null;
    setState({ kind: "downloading", update, received, total });
    const onEvent = (event: DownloadEvent) => {
      if (event.event === "Started") total = event.data.contentLength ?? null;
      if (event.event === "Progress") received += event.data.chunkLength;
      setState(event.event === "Finished" ? { kind: "installing", update } : { kind: "downloading", update, received, total });
    };
    try {
      // On Windows the installer closes OpenDict and starts the new version itself; on macOS and Linux we restart.
      await update.downloadAndInstall(onEvent);
    } catch (error) {
      setState({ kind: "available", update, failure: String(error) });
      return;
    }
    await relaunch();
  }

  switch (state.kind) {
    case "none":
      return null;
    case "available":
      return (
        <Notice>
          <span role={state.failure ? "alert" : undefined}>
            {state.failure ? t.updateFailed(state.update.version, state.failure) : t.updateAvailable(state.update.version)}
          </span>
          <button
            type="button"
            onClick={() => void install(state.update)}
            className="h-5 rounded-sharp border border-rule px-1.5 text-text hover:bg-shade active:translate-y-px"
          >
            {t.updateInstall}
          </button>
        </Notice>
      );
    case "downloading": {
      const percent = state.total ? Math.min(100, Math.floor((state.received / state.total) * 100)) : null;
      return (
        <Notice>
          <span role="status">{t.updateDownloading(state.update.version, percent)}</span>
        </Notice>
      );
    }
    case "installing":
      return (
        <Notice>
          <span role="status">{t.updateInstalling(state.update.version)}</span>
        </Notice>
      );
  }
}

function Notice({ children }: { children: ReactNode }) {
  return <span className="ml-auto flex items-center gap-sm">{children}</span>;
}
