import type { Track } from "../api";
import { useStrings } from "../strings";
import { describeSpec, groupReasons } from "../text";
import { LampStrip } from "./LampStrip";

type Props = {
  track: Track;
  index: number;
  selected: boolean;
  onSelect: () => void;
  onRemove: () => void;
  /** Taking a track out mid-run would change what is being converted. */
  frozen: boolean;
  /**
   * Whether the result still has to be judged.
   *
   * Under a target that promises playback it does not: the second strip would
   * be ten green lamps under every track on the screen, and two near-identical
   * rows of lamps are harder to read than one. Under any other target it is
   * the answer being looked for, so it is always there.
   */
  showAfter: boolean;
};

export function TrackRow({
  track,
  index,
  selected,
  onSelect,
  onRemove,
  frozen,
  showAfter,
}: Props) {
  const t = useStrings();

  const failing = track.now.filter((lamp) => !lamp.ok).length;
  const state = track.error !== null || failing > 0 ? "ng" : "ok";

  /*
    Why the row is marked, which is a fact about the file as it stands. What
    the conversion makes of it is on the strip below under any target that does
    not already promise an answer, and under the two that do there is nothing
    to say.
  */
  const reasons = groupReasons(t, track.now);

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

          {/*
            Ahead of the name, and only when there is something to do. A track
            that plays everywhere needs no label saying so: down a list of
            forty it is forty labels for the tracks that want nothing, and the
            few that do want something have to be found among them.
          */}
          {track.error !== null && (
            <span className="row-judge ng">{t.track.unreadable}</span>
          )}
          {track.error === null && failing > 0 && (
            <span className="row-judge ng">{t.track.convert}</span>
          )}

          <span className="row-name">{track.name}</span>
          <span className="push" />

          {/*
            The source was already short of information, and every other figure
            on this row goes up during a conversion while this one cannot. Said
            here, beside the bitrate that explains it, rather than in a dialog
            in front of the list.
          */}
          {/*
            Its own panel rather than the browser's tooltip, which waits about
            a second before it appears — long enough that a mark nobody can
            read is all most people ever see of this.
          */}
          {track.thin && (
            <span aria-label={t.track.thin} className="row-thin" role="img">
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path d="M8 2.2 15 14.2H1z" />
                <path d="M8 6.4v3.4" />
                <circle cx="8" cy="11.9" r="0.85" />
              </svg>

              <span aria-hidden="true" className="row-thin-say">
                <b>{t.track.thin}</b>
                {t.track.thinNote}
              </span>
            </span>
          )}

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
            {track.dither && <span className="row-doing">{t.track.dither}</span>}
          </div>
        )}

        <LampStrip
          when={showAfter ? t.track.lampsNow : t.track.lampsOnly}
          lamps={track.now}
          onBlue={selected}
        />
        {showAfter && track.after.length > 0 && (
          <LampStrip when={t.track.lampsAfter} lamps={track.after} onBlue={selected} />
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
