import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import type { Contents, DeviceRow, Drive, ConvertOptions } from "../api";
import { checkDrive, scanDrive } from "../api";
import { useStrings } from "../strings";
import { DevicePicker } from "./DevicePicker";
import { LampStrip } from "./LampStrip";

type Props = {
  settings: ConvertOptions;
  rows: DeviceRow[];
  chosen: string[];
  onChooseDevices: (chosen: string[]) => void;
  onScanning: (running: boolean) => void;
};

/**
 * How many offending paths to name before summarising the rest.
 *
 * A drive that is wrong is usually wrong in one place repeated many times, so
 * the first few name the problem and the rest would only bury it.
 */
const NAMED_AT_MOST = 20;

export function DrivePanel({
  settings,
  rows,
  chosen,
  onChooseDevices,
  onScanning,
}: Props) {
  const t = useStrings();

  const [at, setAt] = useState<string | null>(null);
  const [drive, setDrive] = useState<Drive | null>(null);
  const [contents, setContents] = useState<Contents | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function choose() {
    const picked = await open({ directory: true, title: t.dialog.pickDrive });
    if (typeof picked === "string") setAt(picked);
  }

  /*
    Keyed on the players rather than on the whole of settings: reading a drive
    is one ffprobe per track, and changing the output format has no bearing on
    what is already written to it.
  */
  const players = settings.devices.join(",");

  useEffect(() => {
    if (at === null) return;

    // A slow scan of the drive left behind must not land on top of a new one.
    let current = true;

    setContents(null);
    onScanning(true);

    void checkDrive(at, settings).then((found) => {
      if (!current) return;
      setDrive(found);
      setMessage(found === null ? t.drive.nothingMounted(at) : null);
    });

    void scanDrive(at, settings)
      .then((found) => {
        if (current) setContents(found);
      })
      .finally(() => {
        if (current) onScanning(false);
      });

    return () => {
      current = false;
      onScanning(false);
    };
  }, [at, players]);

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

            <ScanReport contents={contents} />
          </div>
        </div>
      )}
    </div>
  );
}

/** What is on the drive, once every track has been read. */
function ScanReport({ contents }: { contents: Contents | null }) {
  const t = useStrings();

  if (contents === null) return null;

  const plays = contents.tracks - contents.failing.length;

  return (
    <section className="scan">
      <div className="scan-head">{t.scan.title}</div>

      <div className="scan-counts">
        <span className="cell">
          <span className="cell-key">TRACKS</span>
          <span className="cell-val">{contents.tracks.toLocaleString()}</span>
        </span>
        {/*
          Shown next to the rejected count rather than only when nothing is
          wrong. A drive with one bad track is mostly good news, and reporting
          only the failure leaves somebody to work that out by subtraction.
        */}
        <span className="cell">
          <span className="cell-key">PLAYS</span>
          <span className={plays > 0 ? "cell-val ok" : "cell-val"}>
            {plays.toLocaleString()}
          </span>
        </span>
        <span className="cell">
          <span className="cell-key">REJECTED</span>
          <span
            className={contents.failing.length > 0 ? "cell-val ng" : "cell-val"}
          >
            {contents.failing.length.toLocaleString()}
          </span>
        </span>
        <span className="cell">
          <span className="cell-key">FOLDERS</span>
          <span className="cell-val">{contents.folders.toLocaleString()}</span>
        </span>
        <span className="cell">
          <span className="cell-key">DEPTH</span>
          <span
            className={
              contents.deepest > contents.depthLimit ? "cell-val ng" : "cell-val"
            }
          >
            {contents.deepest}
            <small> / {contents.depthLimit}</small>
          </span>
        </span>
      </div>

      {contents.tracks === 0 ? (
        <p className="note">{t.scan.noTracks}</p>
      ) : (
        /*
          Keyed on the tracks alone, not on `clean`: a folder nested too deep
          says nothing about whether the tracks themselves play, and "2 of 2
          tracks play" is a strange way to say all of them do.
        */
        <p className={plays > 0 ? "scan-clear" : "note"}>
          {contents.failing.length === 0
            ? t.scan.allPlay(contents.tracks)
            : t.scan.someFail(plays, contents.tracks)}
        </p>
      )}

      {contents.otherFiles > 0 && (
        <p className="note">{t.scan.otherFiles(contents.otherFiles)}</p>
      )}

      {contents.unreachable.length > 0 && (
        <Finding
          title={t.scan.deepTitle(contents.unreachable.length)}
          note={t.scan.deepNote(contents.depthLimit)}
        >
          {contents.unreachable.slice(0, NAMED_AT_MOST).map((folder) => (
            <li key={folder}>
              <span className="scan-path">{folder}</span>
            </li>
          ))}
          <More total={contents.unreachable.length} />
        </Finding>
      )}

      {contents.crowded.length > 0 && contents.entryLimit !== null && (
        <Finding
          title={t.scan.crowdedTitle(contents.crowded.length)}
          note={t.scan.crowdedNote(contents.entryLimit)}
        >
          {contents.crowded.slice(0, NAMED_AT_MOST).map((folder) => (
            <li key={folder.folder}>
              <span className="scan-path">
                {folder.folder === "" ? t.scan.root : folder.folder}
              </span>
              <span className="scan-count">
                {t.scan.crowdedEntries(folder.entries)}
              </span>
            </li>
          ))}
          <More total={contents.crowded.length} />
        </Finding>
      )}

      {contents.failing.length > 0 && (
        <Finding
          title={t.scan.failingTitle(contents.failing.length)}
          note={t.scan.failingNote}
        >
          {contents.failing.slice(0, NAMED_AT_MOST).map((track) => (
            <li key={track.path}>
              <span className="scan-track">
                <span className="scan-name">{track.name}</span>
                <span className="scan-where">
                  {track.folder === "" ? t.scan.root : track.folder}
                </span>
              </span>
              {track.error === null ? (
                <LampStrip lamps={track.lamps} />
              ) : (
                <span className="row-error">{track.error}</span>
              )}
            </li>
          ))}
          <More total={contents.failing.length} />
        </Finding>
      )}
    </section>
  );
}

type FindingProps = {
  title: string;
  note: string;
  children: React.ReactNode;
};

function Finding({ title, note, children }: FindingProps) {
  return (
    <div className="finding">
      <div className="finding-title">{title}</div>
      <div className="finding-note">{note}</div>
      <ul className="finding-list">{children}</ul>
    </div>
  );
}

/**
 * Stopping silently would read as "that is all of them", which is the one thing
 * a report of what is wrong must never imply.
 */
function More({ total }: { total: number }) {
  const t = useStrings();
  const rest = total - NAMED_AT_MOST;

  if (rest <= 0) return null;

  return <li className="finding-more">{t.scan.andMore(rest)}</li>;
}
