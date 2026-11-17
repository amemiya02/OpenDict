import { useEffect, useId, useState } from "react";
import { useI18n } from "../i18n";
import {
  chooseDataFolder,
  getDataDirectory,
  moveDataDirectory,
  switchDataDirectory,
  type DataDirectory,
  type DataDirectoryError,
} from "../lib/ipc";
import { reloadInterface } from "../lib/reload";
import { OutlineButton } from "./LibraryPage";

type Said = { kind: "status" | "alert"; text: string } | null;

/**
 * Where the reader's data is kept, and moving it (for example into a synced folder). Set
 * like the sections above it: a heading, then rows divided by hairlines, no boxes. A
 * folder that already holds OpenDict data is offered for use instead, which is how a
 * second computer takes up synced data; the interface then loads afresh from it.
 */
export function DataSettings() {
  const { t } = useI18n();
  const heading = useId();
  const [directory, setDirectory] = useState<DataDirectory | null>(null);
  const [working, setWorking] = useState(false);
  /** A folder holding OpenDict data, offered for use instead of moving into it. */
  const [offered, setOffered] = useState<string | null>(null);
  const [said, setSaid] = useState<Said>(null);

  useEffect(() => {
    let current = true;
    getDataDirectory()
      .then((found) => current && setDirectory(found))
      .catch(() => {});
    return () => {
      current = false;
    };
  }, []);

  if (!directory) return null;

  async function adopt(path: string) {
    setOffered(null);
    setWorking(true);
    try {
      await switchDataDirectory(path);
      reloadInterface();
    } catch (error) {
      setSaid({ kind: "alert", text: t.dataNotUsed(path, (error as DataDirectoryError).detail) });
      setWorking(false);
    }
  }

  async function move() {
    const from = directory!.path;
    setSaid(null);
    setOffered(null);
    const path = await chooseDataFolder().catch(() => null);
    if (!path) return;
    setWorking(true);
    try {
      setDirectory(await moveDataDirectory(path));
      setSaid({ kind: "status", text: t.dataMoved(path) });
    } catch (error) {
      const failure = error as DataDirectoryError;
      if (failure.kind === "holdsData") setOffered(failure.detail);
      else setSaid({ kind: "alert", text: t.dataNotMoved(from, failure.detail) });
    } finally {
      setWorking(false);
    }
  }

  async function chooseAnother() {
    setSaid(null);
    const path = await chooseDataFolder().catch(() => null);
    if (path) await adopt(path);
  }

  return (
    <section aria-labelledby={heading} aria-busy={working} className="max-w-[860px]">
      <h2 id={heading} className="mt-5 mb-1.5 text-[13px] font-bold">
        {t.yourData}
      </h2>
      <div className="flex min-h-[34px] items-start gap-3.5 border-t border-hair py-1.5">
        <span className="w-[120px] flex-none pt-1 text-text-secondary">{t.dataDirectory}</span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-3.5 gap-y-1.5">
            <code className="min-w-0 font-sans [overflow-wrap:anywhere] select-text">{directory.path}</code>
            {directory.problem ? null : (
              <OutlineButton onClick={() => void move()} disabled={working}>
                {t.moveData}
              </OutlineButton>
            )}
          </div>
          {directory.problem ? (
            <div className="flex flex-wrap items-center gap-x-3.5 gap-y-1.5 pt-1.5">
              <p className="text-[12px] font-semibold">{t.dataMissing}</p>
              <OutlineButton onClick={() => void chooseAnother()} disabled={working}>
                {t.chooseDataFolder}
              </OutlineButton>
            </div>
          ) : (
            <p className="pt-1 text-[12px] text-text-secondary">{t.dataDirectoryNote}</p>
          )}
          {offered ? (
            <div className="flex flex-wrap items-center gap-x-3.5 gap-y-1.5 pt-1.5">
              <p className="text-[12px]">{t.dataHeldThere(offered)}</p>
              <OutlineButton onClick={() => void adopt(offered)} disabled={working}>
                {t.useThatData}
              </OutlineButton>
              <OutlineButton onClick={() => setOffered(null)} disabled={working}>
                {t.cancel}
              </OutlineButton>
            </div>
          ) : null}
          {working ? (
            <p role="status" className="pt-1.5 text-label text-text-secondary">
              {t.movingData}
            </p>
          ) : said?.kind === "status" ? (
            <p role="status" className="pt-1.5 text-label text-text-secondary">
              {said.text}
            </p>
          ) : said?.kind === "alert" ? (
            <p role="alert" className="pt-1.5 text-[12px] font-semibold">
              {said.text}
            </p>
          ) : null}
        </div>
      </div>
    </section>
  );
}
