import type { Lamp } from "../api";
import { useStrings } from "../strings";

type Props = {
  /** Omitted where the surrounding row already says what is being judged. */
  when?: string;
  lamps: Lamp[];
  /** A selected row is filled blue, so the tally has to shift with it. */
  onBlue?: boolean;
};

/**
 * Ten players, always in the same order.
 *
 * Fixed positions turn the list into columns, so "this model fails everything"
 * reads at a glance. Failing lamps are hatched as well as red, so the reading
 * does not depend on colour.
 */
export function LampStrip({ when, lamps, onBlue = false }: Props) {
  const t = useStrings();

  if (lamps.length === 0) return null;

  const ok = lamps.filter((lamp) => lamp.ok).length;

  return (
    <div className="lamps">
      {when !== undefined && <span className="lamps-when">{when}</span>}
      <span className="lamps-row">
        {lamps.map((lamp) => (
          <span
            key={lamp.id}
            className={lamp.ok ? "lamp go" : "lamp stop"}
            title={
              lamp.ok ? t.track.playsOn(lamp.name) : t.track.failsOn(lamp.name)
            }
          >
            {lamp.short}
          </span>
        ))}
      </span>
      <span className={ok === lamps.length ? "tally ok" : "tally ng"}>
        {ok}
        <span className={onBlue ? "tally-of on-blue" : "tally-of"}>
          /{lamps.length}
        </span>
      </span>
    </div>
  );
}
