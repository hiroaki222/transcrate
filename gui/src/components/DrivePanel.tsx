import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import type { DeviceRow, Drive, ConvertOptions } from "../api";
import { checkDrive } from "../api";
import { useStrings } from "../strings";
import { DevicePicker } from "./DevicePicker";
import { LampStrip } from "./LampStrip";

type Props = {
  settings: ConvertOptions;
  rows: DeviceRow[];
  chosen: string[];
  onChooseDevices: (chosen: string[]) => void;
};

export function DrivePanel({ settings, rows, chosen, onChooseDevices }: Props) {
  const t = useStrings();

  const [at, setAt] = useState<string | null>(null);
  const [drive, setDrive] = useState<Drive | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function choose() {
    const picked = await open({ directory: true, title: t.dialog.pickDrive });
    if (typeof picked === "string") setAt(picked);
  }

  // Re-judge the same drive when the gear behind the question changes.
  useEffect(() => {
    if (at === null) return;

    void checkDrive(at, settings).then((found) => {
      setDrive(found);
      setMessage(found === null ? t.drive.nothingMounted(at) : null);
    });
  }, [at, settings]);

  const unreadable = drive?.lamps.filter((lamp) => !lamp.ok) ?? [];
  const filesystem = drive?.filesystem ?? drive?.reportedAs ?? "";

  return (
    <div className="pane">
      <div className="bar">
        <button className="box-btn" type="button" onClick={choose}>
          {t.drive.pick}
        </button>
        <DevicePicker chosen={chosen} onChange={onChooseDevices} rows={rows} />
        <span className="push" />
        <span className="modetag">{t.drive.readOnly}</span>
      </div>

      {drive === null ? (
        <div className="empty">
          <div className="empty-title">{t.drive.emptyTitle}</div>
          <div className="empty-note">{t.drive.emptyNote}</div>
          {message !== null && <div className="empty-note">{message}</div>}
        </div>
      ) : (
        <div className="drive">
          <div className="drive-head">
            <div className="drive-line">
              <span className="drive-name">{drive.mountPoint}</span>
              <span className="tag">{filesystem}</span>
            </div>
            <div className="drive-answer">
              {unreadable.length === 0 ? (
                t.drive.allRead(drive.lamps.length)
              ) : (
                <span className="ng">{t.drive.someFail(unreadable.length)}</span>
              )}
            </div>
          </div>

          <div className="drive-body">
            <LampStrip when={t.drive.lamps} lamps={drive.lamps} />

            {unreadable.length > 0 && (
              <dl className="why">
                <div className="why-line">
                  <dt>{t.track.reasonCount(unreadable.length)}</dt>
                  <dd>
                    {t.drive.failReason(
                      filesystem,
                      unreadable.map((lamp) => lamp.name).join("、"),
                    )}
                  </dd>
                </div>
                <div className="why-line">
                  <dt className="fix">{t.drive.fix}</dt>
                  <dd>{t.drive.fixNote(drive.lamps.length)}</dd>
                </div>
              </dl>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
