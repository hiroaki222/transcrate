import type { Choice } from "../strings";
import { CHOICES, CHOICE_NAMES, useStrings } from "../strings";

type Props = {
  choice: Choice;
  onChange: (choice: Choice) => void;
};

/** Everything that is not about a track. Language is all it holds so far. */
export function UtilityPanel({ choice, onChange }: Props) {
  const t = useStrings();

  return (
    <div className="pane">
      <section className="setting-group">
        <h2 className="setting-title">{t.settings.language}</h2>

        <div className="choices">
          {CHOICES.map((option) => (
            <button
              className="choice"
              data-on={choice === option ? "" : undefined}
              key={option}
              onClick={() => onChange(option)}
              type="button"
            >
              <span className="dot" />
              <span className="choice-label">
                {option === "auto" ? t.settings.auto : CHOICE_NAMES[option]}
              </span>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
