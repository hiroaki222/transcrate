import { useStrings } from "../strings";

type Props = {
  /** True only while files are held over the window. */
  hovering: boolean;
  onPick: () => void;
};

/**
 * No border: a drop is accepted anywhere in the window, so drawing an edge
 * would be a lie about where it works. The whole face is the button.
 */
export function DropZone({ hovering, onPick }: Props) {
  const t = useStrings();


  return (
    <div className="drop">
      <button
        className="drop-bay"
        data-hot={hovering ? "" : undefined}
        onClick={onPick}
        type="button"
      >
        <span className="drop-title">{t.empty.title}</span>
        <span className="drop-note">{t.empty.note}</span>
      </button>
    </div>
  );
}
