import { useState } from "react";

import { useStrings } from "../strings";

/**
 * Chosen by what they are for rather than by format. Two of them guarantee
 * playback; `archive` is here because keeping a copy is a purpose people have,
 * and it is the one with no such promise — which is what the note under it
 * says and what the disclosure below is arranged around.
 */
const MAIN = ["cdj-safe", "lossless", "archive"];

/** Container-only changes. Rate and depth survive, so playback may not. */
const DIRECT = ["aiff", "wav", "flac"];

type Props = {
  profile: string;
  onChange: (profile: string) => void;
};

/**
 * What is being chosen is a purpose, not a format.
 *
 * In a flat list `archive` reads as the premium option when it is the only one
 * with no playback guarantee, so the bare formats sit behind a disclosure and
 * every heading names what the choice guarantees.
 */
export function TargetPicker({ profile, onChange }: Props) {
  const t = useStrings();

  const [more, setMore] = useState(DIRECT.includes(profile));

  const card = (id: string) => {
    const option = t.profiles[id];
    if (option === undefined) return null;

    return (
      <button
        className="card"
        data-on={profile === id ? "" : undefined}
        key={id}
        onClick={() => onChange(id)}
        type="button"
      >
        <span className="card-dot" />
        <span className="card-text">
          <span className="card-label">{option.label}</span>
          <span className="card-format">{option.format}</span>
          <span className="card-note">{option.note}</span>
        </span>
        <span className="card-id">{id}</span>
      </button>
    );
  };

  return (
    <section className="target">
      <div className="target-head">
        <span className="target-key">{t.toolbar.target}</span>
        <span className="push" />
        <button
          className="box-btn"
          data-on={more ? "" : undefined}
          onClick={() => setMore((was) => !was)}
          type="button"
        >
          {more ? t.toolbar.less : t.toolbar.more}
        </button>
      </div>

      <div className="cards">{MAIN.map(card)}</div>
      {more && <div className="cards">{DIRECT.map(card)}</div>}
    </section>
  );
}
