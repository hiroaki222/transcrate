import { useStrings } from "../strings";

type Props = {
  open: boolean;
  onOpen: () => void;
};

/** Mixer faders rather than a cog: on a black panel a cog reads as a sun. */
export function SettingsButton({ open, onOpen }: Props) {
  const t = useStrings();

  return (
    <button
      aria-label={t.settings.open}
      className="gear"
      data-on={open ? "" : undefined}
      onClick={onOpen}
      title={t.settings.open}
      type="button"
    >
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <path d="M3 5.5h14M3 10h14M3 14.5h14" />
        <circle cx="7" cy="5.5" r="2.1" />
        <circle cx="13.5" cy="10" r="2.1" />
        <circle cx="6" cy="14.5" r="2.1" />
      </svg>
    </button>
  );
}
