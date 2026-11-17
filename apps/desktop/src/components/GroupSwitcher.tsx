import { useI18n } from "../i18n";
import type { Group } from "../lib/ipc";

const chevron =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='none' stroke='%23a6a8af' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M4 6l4 4 4-4'/%3E%3C/svg%3E\")";

/**
 * The Current Group, chosen in the cover band beside the app name: the whole Library or
 * one of the reader's Groups. Hidden until the reader makes a Group.
 */
export function GroupSwitcher({
  groups,
  current,
  onSwitch,
}: {
  groups: Group[];
  /** The Current Group's key; null for the whole Library. */
  current: string | null;
  onSwitch: (group: string | null) => void;
}) {
  const { t } = useI18n();
  if (groups.length === 0) return null;
  return (
    <>
      <label htmlFor="current-group" className="sr-only">
        {t.currentGroup}
      </label>
      <select
        id="current-group"
        value={current ?? ""}
        title={t.currentGroup}
        onChange={(event) => onSwitch(event.target.value === "" ? null : event.target.value)}
        style={{ backgroundImage: chevron }}
        className="h-7 max-w-[136px] min-w-0 appearance-none truncate rounded-sharp border border-cover-edge bg-transparent bg-[right_5px_center] bg-no-repeat bg-size-[14px] pr-6 pl-2.5 text-body text-on-cover outline-offset-1 hover:bg-cover-edge focus-visible:outline focus-visible:outline-1 focus-visible:outline-on-cover max-[860px]:max-w-[96px] [&>option]:bg-india-paper [&>option]:text-ink"
      >
        <option value="">{t.wholeLibrary}</option>
        {groups.map((group) => (
          <option key={group.id} value={group.id}>
            {group.name}
          </option>
        ))}
      </select>
    </>
  );
}
