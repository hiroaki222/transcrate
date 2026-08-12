import type { Track } from "../api";
import { useStrings } from "../strings";
import { actionName, describeSpec, groupReasons, verdict } from "../text";
import { LampStrip } from "./LampStrip";

type Props = {
  track: Track;
  index: number;
  selected: boolean;
  onSelect: () => void;
};

/** A second strip is only worth reading when the verdict actually moves. */
const changesAnything = (track: Track) =>
  track.after.some(
    (after, at) => after.ok !== (track.now[at]?.ok ?? after.ok),
  );

export function TrackRow({ track, index, selected, onSelect }: Props) {
  const t = useStrings();

  const failing = track.now.filter((lamp) => !lamp.ok).length;
  const state = track.error !== null || failing > 0 ? "ng" : "ok";
  const reasons = groupReasons(t, track.now);

  return (
    <div
      className="row"
      data-state={state}
      data-sel={selected ? "" : undefined}
      onClick={onSelect}
      onKeyDown={(event) => {
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

        {selected && reasons.length > 0 && (
          <dl className="why">
            {reasons.map(({ reason, devices }) => (
              <div className="why-line" key={reason}>
                <dt>{t.track.reasonCount(devices.length)}</dt>
                <dd>
                  {reason}。{devices.join("、")}
                </dd>
              </div>
            ))}
          </dl>
        )}
      </div>
    </div>
  );
}
