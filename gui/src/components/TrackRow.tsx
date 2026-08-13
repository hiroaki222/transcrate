import type { Track } from "../api";
import { useStrings } from "../strings";
import { actionName, describeSpec, groupReasons, verdict } from "../text";
import { LampStrip } from "./LampStrip";

type Props = {
  track: Track;
  index: number;
  selected: boolean;
  onSelect: () => void;
  onRemove: () => void;
  /** Taking a track out mid-run would change what is being converted. */
  frozen: boolean;
};

/** A second strip is only worth reading when the verdict actually moves. */
const changesAnything = (track: Track) =>
  track.after.some(
    (after, at) => after.ok !== (track.now[at]?.ok ?? after.ok),
  );

export function TrackRow({
  track,
  index,
  selected,
  onSelect,
  onRemove,
  frozen,
}: Props) {
  const t = useStrings();

  const failing = track.now.filter((lamp) => !lamp.ok).length;
  const state = track.error !== null || failing > 0 ? "ng" : "ok";

  /*
    Reasons for what will still be wrong, not for what is wrong now. Opened,
    an ALAC track bound for AIFF used to explain that three players do not read
    ALAC — directly under a second strip showing all ten of them lit, because
    the conversion on screen deals with it. The reader is left to work out that
    the complaint is about the file they are replacing.
  */
  const settled = track.after.length > 0 ? track.after : track.now;
  const reasons = groupReasons(t, settled);
  const mended = failing > 0 && reasons.length === 0;

  return (
    <div
      className="row"
      data-state={state}
      data-sel={selected ? "" : undefined}
      onClick={onSelect}
      onKeyDown={(event) => {
        // Only the row itself. Enter on the remove button inside it reaches
        // here too, and would take the track out and open it in one press.
        if (event.target !== event.currentTarget) return;
        if (event.key === "Enter" || event.key === " ") onSelect();
      }}
      role="button"
      tabIndex={0}
    >
      <span className="row-edge" />
      <div className="row-main">
        <div className="row-head">
          <span className="row-no">{String(index + 1).padStart(3, "0")}</span>
          <span className="row-name">{track.name}</span>
          {track.error === null ? (
            <span className={failing > 0 ? "row-judge ng" : "row-judge ok"}>
              {verdict(t, track.now)}
            </span>
          ) : (
            <span className="row-judge ng">{t.track.unreadable}</span>
          )}
          <span className="push" />

          {/*
            The source was already short of information, and every other figure
            on this row goes up during a conversion while this one cannot. Said
            here, beside the bitrate that explains it, rather than in a dialog
            in front of the list.
          */}
          {track.thin && <span className="row-thin">{t.track.thin}</span>}

          {/*
            Its own button rather than a second meaning for the row, and the
            click stops here: the row opens on click, and taking a track out
            while opening its detail would be two answers to one press.
          */}
          <button
            aria-label={t.track.remove}
            className="row-drop"
            disabled={frozen}
            onClick={(event) => {
              event.stopPropagation();
              onRemove();
            }}
            title={t.track.remove}
            type="button"
          >
            ×
          </button>
        </div>

        {track.error !== null && <div className="row-error">{track.error}</div>}

        {track.source !== null && track.output !== null && (
          <div className="row-spec">
            {describeSpec(track.source)}
            <span className="row-arrow">→</span>
            <b>{describeSpec(track.output)}</b>
            <span className="row-doing">{actionName(t, track.action)}</span>
            {track.dither && <span className="row-doing">{t.track.dither}</span>}
          </div>
        )}

        <LampStrip when={t.track.lampsNow} lamps={track.now} onBlue={selected} />
        {changesAnything(track) && (
          <LampStrip when={t.track.lampsAfter} lamps={track.after} onBlue={selected} />
        )}

        {selected && mended && (
          <p className="why mended">{t.track.mended(track.after.length)}</p>
        )}

        {selected && reasons.length > 0 && (
          <dl className="why">
            {reasons.map(({ reason, devices }) => (
              <div className="why-line" key={reason}>
                <dt>{t.track.reasonCount(devices.length)}</dt>
                <dd>{t.track.reasonDetail(reason, devices)}</dd>
              </div>
            ))}
          </dl>
        )}
      </div>
    </div>
  );
}
