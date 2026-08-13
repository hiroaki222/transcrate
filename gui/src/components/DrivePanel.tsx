import { useEffect, useState } from "react";

import type { Contents, DeviceRow, Drive, Mounted, ConvertOptions } from "../api";
import { checkDrive, drives as listDrives, scanDrive } from "../api";
import { useStrings } from "../strings";
import { DevicePicker } from "./DevicePicker";
import { LampStrip } from "./LampStrip";

type Props = {
  settings: ConvertOptions;
  rows: DeviceRow[];
  chosen: string[];
  onChooseDevices: (chosen: string[]) => void;
  onScanning: (running: boolean) => void;
  /** So the status bar can carry the drive counts while this screen is open. */
  onDrives: (found: Mounted[]) => void;
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
  onDrives,
}: Props) {
  const t = useStrings();

  const [at, setAt] = useState<string | null>(null);
  const [drive, setDrive] = useState<Drive | null>(null);
  const [contents, setContents] = useState<Contents | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const [mounted, setMounted] = useState<Mounted[] | null>(null);
  const [looking, setLooking] = useState(false);

  /*
    Asked again on every look rather than once at startup. Plugging a stick in
    after opening the app is the ordinary case, and a list that cannot answer
    that is a list nobody trusts.
  */
  function look() {
    setLooking(true);
    void listDrives(settings)
      .then((found) => {
        setMounted(found);
        onDrives(found);
        // One drive is not a choice. Somebody with a single stick plugged in
        // has already said which one they mean by plugging it in.
        const only = found.length === 1 ? found[0] : undefined;
        if (only !== undefined) setAt(only.mountPoint);
      })
      .finally(() => setLooking(false));
  }

  /*
    Keyed on the players rather than on the whole of settings: reading a drive
    is one ffprobe per track, and changing the output format has no bearing on
    what is already written to it.
  */
  const players = settings.devices.join(",");

  useEffect(look, [players]);

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

  const chosenDrive = mounted?.find((found) => found.mountPoint === at);
  const unreadable = drive?.lamps.filter((lamp) => !lamp.ok) ?? [];
  const filesystem = drive?.filesystem ?? drive?.reportedAs ?? "";

  return (
    <div className="pane">
      <div className="bar">
        <DevicePicker chosen={chosen} onChange={onChooseDevices} rows={rows} />
        <span className="push" />
        <span className="modetag">{t.drive.readOnly}</span>
      </div>

      {/*
        Two panes, as the players lay this out: what is plugged in on the left,
        and everything known about the one chosen on the right. The list stays
        visible while the right side is read, so moving between two sticks is
        one click rather than a trip back to a picker.
      */}
      <div className="usb">
        <aside className="usb-list">
          <div className="usb-list-head">
            <span className="usb-list-key">USB</span>
            <span className="usb-list-count">
              {t.drive.count(mounted?.length ?? 0)}
            </span>
          </div>

          <div className="usb-list-body">
            {mounted?.map((found) => (
              <button
                aria-pressed={found.mountPoint === at}
                className="stick"
                data-on={found.mountPoint === at ? "" : undefined}
                key={found.mountPoint}
                onClick={() => setAt(found.mountPoint)}
                type="button"
              >
                {/*
                  A bar down the leading edge rather than a colour over the whole
                  row: the verdict has to survive the row being selected, and a
                  selected row is already carrying a colour of its own.
                */}
                <span
                  className={found.readable === found.players ? "stick-edge" : "stick-edge ng"}
                />
                <span className="stick-name">{found.name}</span>
                <span className="tag">{found.filesystem ?? found.reportedAs}</span>
                <span className="stick-free">{t.drive.free(gb(found.freeBytes))}</span>
                <Tally of={found.players} some={found.readable} />
              </button>
            ))}

            {mounted?.length === 0 && (
              <p className="usb-list-none">{t.drive.none}</p>
            )}
          </div>

          <button
            className="box-btn usb-look"
            disabled={looking}
            onClick={look}
            type="button"
          >
            {looking ? t.drive.picking : t.drive.refresh}
          </button>
        </aside>

        {drive === null ? (
          <div className="empty">
            <div className="empty-title">
              {looking ? t.drive.picking : t.drive.emptyTitle}
            </div>
            <div className="empty-note">{t.drive.emptyNote}</div>
            {message !== null && <div className="empty-note">{message}</div>}
          </div>
        ) : (
          <div className="usb-detail">
            <div className="usb-detail-head">
              <span className="usb-detail-name">{drive.name}</span>
              <span className="tag">{filesystem}</span>
              <span className="usb-detail-where">{drive.mountPoint}</span>
            </div>

            <div className="usb-detail-body">
              <p
                className={
                  unreadable.length === 0 ? "verdict" : "verdict ng"
                }
              >
                {unreadable.length === 0
                  ? t.drive.allRead(drive.lamps.length)
                  : t.drive.someFail(unreadable.length)}
              </p>

              <LampStrip when={t.drive.lamps} lamps={drive.lamps} />

              <dl className="facts">
                <div className="fact">
                  <dt>{t.drive.capacity}</dt>
                  <dd>
                    {t.drive.gb(gb(chosenDrive?.freeBytes ?? 0))}
                    <span className="fact-of">
                      {" / "}
                      {t.drive.gb(gb(chosenDrive?.totalBytes ?? 0))}
                    </span>
                  </dd>
                </div>
                <div className="fact">
                  <dt>{t.drive.format}</dt>
                  <dd>{filesystem}</dd>
                </div>
                <div className="fact">
                  <dt>{t.drive.refused}</dt>
                  <dd className={unreadable.length > 0 ? "ng" : undefined}>
                    {unreadable.length === 0
                      ? t.drive.refusedNone
                      : t.drive.refusedNames(unreadable.map((lamp) => lamp.name))}
                  </dd>
                </div>
              </dl>

              <ScanReport contents={contents} />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Bytes as the one figure anybody reads a stick's capacity in.
 *
 * Decimal GB rather than binary: it is what the label on the stick says, and
 * being off by seven per cent from Finder would look like a bug.
 */
function gb(bytes: number) {
  return bytes / 1_000_000_000;
}

/** How many players read this drive, out of how many were asked about. */
function Tally({ of, some }: { of: number; some: number }) {
  return (
    <span className={some === of ? "tally ok" : "tally ng"}>
      {some}
      <span className="tally-of">/{of}</span>
    </span>
  );
}

/** What is on the drive, once every track has been read. */
function ScanReport({ contents }: { contents: Contents | null }) {
  const t = useStrings();

  if (contents === null) return null;

  const plays = contents.tracks - contents.failing.length;
  // Three ways a count above can be short of the drive: a folder the browser
  // never descends into, one it stops listing part way, and one this program
  // could not read at all. Tracks behind any of them were never counted and
  // never judged.
  const hasGaps =
    contents.unreachable.length > 0 ||
    contents.crowded.length > 0 ||
    contents.unreadable.length > 0;

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
          Green says the drive is ready, so it is withheld while anything is
          missing from the count. The sentence itself is still worth saying —
          the tracks that were read do play — but it is a report on what was
          reached rather than a promise about the stick.
        */
        <p className={plays > 0 && !hasGaps ? "scan-clear" : "note"}>
          {contents.failing.length === 0
            ? t.scan.allPlay(contents.tracks)
            : t.scan.someFail(plays, contents.tracks)}
        </p>
      )}

      {hasGaps && contents.tracks > 0 && (
        <p className="note">{t.scan.partial}</p>
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

      {contents.unreadable.length > 0 && (
        <Finding
          title={t.scan.unreadableTitle(contents.unreadable.length)}
          note={t.scan.unreadableNote}
        >
          {contents.unreadable.slice(0, NAMED_AT_MOST).map((folder) => (
            <li key={folder}>
              <span className="scan-path">{folder}</span>
            </li>
          ))}
          <More total={contents.unreadable.length} />
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
            <li className="finding-track" key={track.path}>
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
