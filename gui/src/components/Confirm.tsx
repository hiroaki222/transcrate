import { useEffect, useRef } from "react";

import { useStrings } from "../strings";

type Props = {
  title: string;
  note: string;
  /** Wording on the button that goes through with it, not a bare "OK". */
  confirm: string;
  onConfirm: () => void;
  onCancel: () => void;
};

/**
 * A question with two answers, asked before something is thrown away.
 *
 * Escape and the ground behind it both cancel, and the cancel is what holds
 * focus when this opens: a dialog that arrives with the destructive button
 * under the return key is one that gets dismissed by reflex into the thing it
 * was asking about.
 */
export function Confirm({ title, note, confirm, onConfirm, onCancel }: Props) {
  const t = useStrings();
  const back = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    back.current?.focus();

    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onCancel();
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="veil" onClick={onCancel} role="presentation">
      <div
        aria-modal="true"
        className="ask"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <p className="ask-title">{title}</p>
        <p className="ask-note">{note}</p>

        <div className="ask-answers">
          <button
            className="box-btn"
            onClick={onCancel}
            ref={back}
            type="button"
          >
            {t.confirm.cancel}
          </button>
          <button className="danger-btn" onClick={onConfirm} type="button">
            {confirm}
          </button>
        </div>
      </div>
    </div>
  );
}
