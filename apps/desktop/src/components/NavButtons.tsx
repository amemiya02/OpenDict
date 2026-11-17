import { useI18n } from "../i18n";

/** Back and Forward in the cover band, through the words visited in this window. */
export function NavButtons({
  canGoBack,
  canGoForward,
  onGo,
}: {
  canGoBack: boolean;
  canGoForward: boolean;
  onGo: (by: -1 | 1) => void;
}) {
  const { t } = useI18n();
  return (
    <nav className="flex flex-none gap-0.5 pr-md pl-xl max-[860px]:pl-xs">
      <CoverIcon label={t.back} hint="Alt+←" disabled={!canGoBack} onClick={() => onGo(-1)} path="M10 3 5 8l5 5" />
      <CoverIcon label={t.forward} hint="Alt+→" disabled={!canGoForward} onClick={() => onGo(1)} path="M6 3l5 5-5 5" />
    </nav>
  );
}

function CoverIcon({
  label,
  hint,
  disabled,
  onClick,
  path,
}: {
  label: string;
  hint: string;
  disabled: boolean;
  onClick: () => void;
  path: string;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-keyshortcuts={hint.replace("←", "ArrowLeft").replace("→", "ArrowRight")}
      title={`${label} (${hint})`}
      disabled={disabled}
      onClick={onClick}
      className="grid size-6.5 place-items-center rounded-sharp text-on-cover-muted enabled:hover:bg-cover-edge enabled:hover:text-on-cover disabled:opacity-40"
    >
      <svg className="size-4 fill-none stroke-current stroke-[1.5]" viewBox="0 0 16 16" aria-hidden="true" strokeLinecap="round" strokeLinejoin="round">
        <path d={path} />
      </svg>
    </button>
  );
}
