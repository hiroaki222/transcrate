import { useEffect, useRef, useState } from "react";

import type { DeviceRow } from "../api";
import { useStrings } from "../strings";

type Props = {
  rows: DeviceRow[];
  /** Chosen device ids. Never empty. */
  chosen: string[];
  onChange: (chosen: string[]) => void;
};

/**
 * Checkboxes rather than one choice: the real unit is "the few players in the
 * venue I am playing", not a single model. The last one cannot be unticked —
 * with nothing selected there is nothing to judge against, and no obvious way
 * back from an empty screen.
 */
export function DevicePicker({ rows, chosen, onChange }: Props) {
  const t = useStrings();

  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;

    const close = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) setOpen(false);
    };

    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const all = rows.map((row) => row.id);

  function toggle(id: string) {
    if (chosen.includes(id)) {
      if (chosen.length === 1) return;
      onChange(chosen.filter((kept) => kept !== id));
      return;
    }

    // Back into table order: picking order would break the lamp columns.
    onChange(all.filter((each) => chosen.includes(each) || each === id));
  }

  const label =
    chosen.length === all.length
      ? t.toolbar.allPlayers(all.length)
      : t.toolbar.somePlayers(chosen.length);

  return (
    <div className="picker" ref={box}>
      <button
        className="ctl"
        data-on={open ? "" : undefined}
        onClick={() => setOpen((was) => !was)}
        type="button"
      >
        <span className="ctl-key">{t.toolbar.players}</span>
        <span className="ctl-val">{label}</span>
        <span className="ctl-caret" />
      </button>

      {open && (
        <div className="picker-panel">
          <div className="picker-tools">
            <button
              className="box-btn"
              disabled={chosen.length === all.length}
              onClick={() => onChange(all)}
              type="button"
            >
              {t.toolbar.selectAll}
            </button>
          </div>

          <div className="picker-list">
            {rows.map((row) => {
              const checked = chosen.includes(row.id);
              const last = checked && chosen.length === 1;

              return (
                <button
                  className="picker-row"
                  data-on={checked ? "" : undefined}
                  disabled={last}
                  key={row.id}
                  onClick={() => toggle(row.id)}
                  type="button"
                >
                  <span className="tick" data-on={checked ? "" : undefined} />
                  <span className="picker-short">{row.short}</span>
                  <span className="picker-name">{row.name}</span>
                  <span className="picker-year">{row.year}</span>
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
