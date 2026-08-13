import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import type {
  DeviceRow,
  Mounted,
  Outcome,
  Progress,
  ConvertOptions,
  Tools,
  Track,
} from "./api";
import {
  convertAll,
  devices as loadDevices,
  inspect,
  locale as loadLocale,
  tools as loadTools,
} from "./api";
import { Confirm } from "./components/Confirm";
import { DeviceTable } from "./components/DeviceTable";
import { DevicePicker } from "./components/DevicePicker";
import { DrivePanel } from "./components/DrivePanel";
import { DropZone } from "./components/DropZone";
import { SettingsButton } from "./components/SettingsButton";
import { UtilityPanel } from "./components/UtilityPanel";
import { TargetPicker } from "./components/TargetPicker";
import { TrackRow } from "./components/TrackRow";
import type { Choice } from "./strings";
import { StringsProvider, buttons, resolve, useStrings } from "./strings";

type Tab = "tracks" | "drive" | "devices" | "settings";

/** Gear is fixed per venue, so the choice outlives the session. */
const REMEMBERED = "transcrate.devices";

const LANGUAGE = "transcrate.language";

function remembered(): string[] | null {
  try {
    const saved = localStorage.getItem(REMEMBERED);
    if (saved === null) return null;
    const parsed: unknown = JSON.parse(saved);
    return Array.isArray(parsed) ? (parsed as string[]) : null;
  } catch {
    return null;
  }
}

/**
 * The result nearest the top of the output folder.
 *
 * A folder keeps its shape when it is converted, so the results are spread
 * through a tree rather than sitting in one flat list. Revealing whichever
 * happened to be first would open a folder five levels in, which is the same
 * confusion as not opening anything: the shallowest one puts the whole set in
 * view.
 */
function shallowest(outcomes: Outcome[]): string | null {
  const done = outcomes.filter((outcome) => outcome.error === null);
  if (done.length === 0) return null;

  const depth = (path: string) => path.split(/[/\\]/).length;
  return done.reduce((best, at) =>
    depth(at.outputPath) < depth(best.outputPath) ? at : best,
  ).outputPath;
}

export function App() {
  const [choice, setChoice] = useState<Choice>(
    () => (localStorage.getItem(LANGUAGE) as Choice | null) ?? "auto",
  );
  const [machine, setMachine] = useState<string | null>(null);

  useEffect(() => {
    void loadLocale().then(setMachine);
  }, []);

  useEffect(() => {
    localStorage.setItem(LANGUAGE, choice);
  }, [choice]);

  return (
    <StringsProvider value={resolve(choice, machine)}>
      <Window choice={choice} onChooseLanguage={setChoice} />
    </StringsProvider>
  );
}

type WindowProps = {
  choice: Choice;
  onChooseLanguage: (choice: Choice) => void;
};

function Window({ choice, onChooseLanguage }: WindowProps) {
  const t = useStrings();

  const [tab, setTab] = useState<Tab>("tracks");
  const [profile, setProfile] = useState("cdj-safe");
  // Kept by default: more people write their own cues and keys in the comment
  // than are bothered by a shop's advertising, and losing notes is worse.
  const [keepComment, setKeepComment] = useState(true);
  const [artwork, setArtwork] = useState(true);
  const [chosen, setChosen] = useState<string[]>([]);

  const [dropped, setDropped] = useState<string[]>([]);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [outcomes, setOutcomes] = useState<Outcome[] | null>(null);

  const [sticks, setSticks] = useState<Mounted[]>([]);
  const [busy, setBusy] = useState<"inspect" | "convert" | "scan" | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const [rows, setRows] = useState<DeviceRow[]>([]);
  const [tools, setTools] = useState<Tools | null>(null);
  const [hovering, setHovering] = useState(false);
  const [asking, setAsking] = useState(false);

  const settings: ConvertOptions = useMemo(
    () => ({ profile, keepComment, artwork, devices: chosen }),
    [profile, keepComment, artwork, chosen],
  );

  useEffect(() => {
    void loadDevices().then((loaded) => {
      setRows(loaded);

      // Only restore ids the table still has, in case a profile was dropped.
      const all = loaded.map((row) => row.id);
      const saved = remembered()?.filter((id) => all.includes(id)) ?? [];
      setChosen(saved.length > 0 ? saved : all);
    });

    void loadTools().then(setTools);
  }, []);

  useEffect(() => {
    if (chosen.length > 0) localStorage.setItem(REMEMBERED, JSON.stringify(chosen));
  }, [chosen]);

  useEffect(() => {
    const listeners = [
      listen<Progress>("inspect", (event) => setProgress(event.payload)),
      listen<Progress>("convert", (event) => setProgress(event.payload)),
      listen<Progress>("scan", (event) => setProgress(event.payload)),
    ];

    return () => {
      void Promise.all(listeners).then((offs) => offs.forEach((off) => off()));
    };
  }, []);

  // The drive panel reads through the same counter the conversion screen does,
  // so the window never shows two different notions of "working".
  const onScanning = useCallback((running: boolean) => {
    setBusy(running ? "scan" : null);
    if (!running) setProgress(null);
  }, []);

  const examine = useCallback(
    async (paths: string[]) => {
      setDropped(paths);
      setOutcomes(null);
      setFailure(null);
      setBusy("inspect");

      try {
        setTracks(await inspect(paths, settings));
      } catch (error) {
        setFailure(String(error));
      } finally {
        setBusy(null);
        setProgress(null);
      }
    },
    [settings],
  );

  useEffect(() => {
    const listener = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") setHovering(true);
      else if (event.payload.type === "leave") setHovering(false);
      else if (event.payload.type === "drop") {
        setHovering(false);
        setTab("tracks");
        void examine(event.payload.paths);
      }
    });

    return () => {
      void listener.then((off) => off());
    };
  }, [examine]);

  /*
    Re-judge whatever is listed whenever the settings that decide it change.

    Keyed on `settings` rather than on the four values it is built from. Named
    one by one, the list is the memo's dependency list written out a second
    time: a fifth setting added to the memo and forgotten here would leave the
    verdicts on screen answering the old question, with nothing to say so.
  */
  useEffect(() => {
    if (dropped.length > 0 && busy === null) void examine(dropped);
    // `examine` is rebuilt on every settings change, so depending on it loops.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings]);

  /*
    The list is rewritten to what is left rather than filtered on the way out.
    A drop is usually one folder, and that folder is re-read whenever a setting
    changes — which would bring back everything ever taken out of the list.
  */
  function remove(path: string) {
    const left = tracks.filter((track) => track.path !== path);

    setTracks(left);
    setDropped(left.map((track) => track.path));
    if (selected === path) setSelected(null);
  }

  /*
    Nothing on disk is touched — this empties the list and no more. It is marked
    as the destructive one because it is the only control here that throws away
    work already done, and it sits beside the one that acts on all of it.
  */
  function clear() {
    setAsking(false);
    setTracks([]);
    setDropped([]);
    setSelected(null);
    setOutcomes(null);
    setFailure(null);
  }

  async function choose() {
    const picked = await open({ multiple: true, title: t.dialog.pickTracks });
    if (picked === null) return;
    await examine(Array.isArray(picked) ? picked : [picked]);
  }

  async function run() {
    setBusy("convert");
    setFailure(null);

    try {
      const done = await convertAll(dropped, settings);
      setOutcomes(done);

      // Show where it landed, rather than leaving people to hunt for it.
      const landing = shallowest(done);
      if (landing !== null) await revealItemInDir(landing);

      await examine(dropped);
    } catch (error) {
      setFailure(String(error));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  }

  const failing = tracks.filter(
    (track) => track.error !== null || track.now.some((lamp) => !lamp.ok),
  ).length;

  // A stick any chosen player refuses is one to deal with before the gig.
  const refusedSticks = sticks.filter(
    (stick) => stick.readable < stick.players,
  ).length;

  const converted = outcomes?.filter((outcome) => outcome.error === null).length ?? 0;
  // Named, not counted. "1 could not be converted" out of a list of forty
  // leaves the one file that needs attention to be found by hand.
  const refused = outcomes?.filter((outcome) => outcome.error !== null) ?? [];

  const landed = outcomes === null ? null : shallowest(outcomes);
  const missing = tools !== null && (!tools.ffmpeg || !tools.ffprobe);

  const tabs: [Tab, string][] = [
    ["tracks", buttons.tracks],
    ["drive", buttons.drive],
    ["devices", buttons.devices],
  ];

  return (
    <div className="app" data-hovering={hovering ? "" : undefined}>
      <header className="topbar">
        <span className="lamp-bar" />
        <span className="mark">TRANSCRATE</span>
        <span className="push" />
        {missing && <span className="modetag">{t.status.ffmpegMissing}</span>}
        <SettingsButton
          onOpen={() => setTab("settings")}
          open={tab === "settings"}
        />
      </header>

      <nav className="tabs">
        {tabs.map(([id, label]) => (
          <button
            className="tab"
            data-on={tab === id ? "" : undefined}
            key={id}
            onClick={() => setTab(id)}
            type="button"
          >
            {label}
          </button>
        ))}
      </nav>

      {tab === "tracks" && (
        <div className="pane">
          <div className="bar">
            <DevicePicker chosen={chosen} onChange={setChosen} rows={rows} />

            <button
              className="box-btn"
              data-on={keepComment ? "" : undefined}
              onClick={() => setKeepComment((on) => !on)}
              type="button"
            >
              {t.toolbar.keepComment}
            </button>
            <button
              className="box-btn"
              data-on={artwork ? "" : undefined}
              onClick={() => setArtwork((on) => !on)}
              type="button"
            >
              {t.toolbar.keepArtwork}
            </button>

            <span className="push" />
            <button
              className="danger-btn"
              disabled={tracks.length === 0 || busy !== null}
              onClick={() => setAsking(true)}
              type="button"
            >
              {t.toolbar.clear}
            </button>
            <button
              className="go-btn"
              disabled={tracks.length === 0 || busy !== null}
              onClick={run}
              type="button"
            >
              {t.toolbar.convert(tracks.length)}
            </button>
          </div>

          {failure !== null && <div className="failure">{failure}</div>}

          {outcomes !== null && (
            <div className="done" data-partial={refused.length > 0 ? "" : undefined}>
              <span className="done-mark" />
              <span className="done-text">
                {t.done.converted(converted)}
                {refused.length > 0 && (
                  <span className="done-failed">
                    {t.done.failed(refused.length)}
                  </span>
                )}
              </span>
              <span className="push" />
              {landed !== null && (
                <button
                  className="box-btn"
                  onClick={() => void revealItemInDir(landed)}
                  type="button"
                >
                  {t.done.reveal}
                </button>
              )}
              <button
                className="box-btn"
                onClick={() => setOutcomes(null)}
                type="button"
              >
                {t.done.dismiss}
              </button>
            </div>
          )}

          {refused.length > 0 && (
            <ul className="refused">
              {refused.map((outcome) => (
                <li key={outcome.path}>
                  <span className="refused-name">{outcome.name}</span>
                  <span className="refused-why">{outcome.error}</span>
                </li>
              ))}
            </ul>
          )}

          <TargetPicker onChange={setProfile} profile={profile} />

          {tracks.length === 0 ? (
            <DropZone hovering={hovering} onPick={choose} />
          ) : (
            <div className="rows">
              {tracks.map((track, at) => (
                <TrackRow
                  frozen={busy !== null}
                  index={at}
                  key={track.path}
                  onRemove={() => remove(track.path)}
                  onSelect={() =>
                    setSelected((was) => (was === track.path ? null : track.path))
                  }
                  selected={selected === track.path}
                  track={track}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {tab === "drive" && (
        <DrivePanel
          chosen={chosen}
          onChooseDevices={setChosen}
          onDrives={setSticks}
          onScanning={onScanning}
          rows={rows}
          settings={settings}
        />
      )}
      {tab === "devices" && <DeviceTable rows={rows} />}
      {tab === "settings" && (
        <UtilityPanel choice={choice} onChange={onChooseLanguage} />
      )}

      {asking && (
        <Confirm
          confirm={t.confirm.clearGo}
          note={t.confirm.clearNote(tracks.length)}
          onCancel={() => setAsking(false)}
          onConfirm={clear}
          title={t.confirm.clearTitle}
        />
      )}

      <footer className="deckbar" data-busy={busy !== null ? "" : undefined}>
        {busy === null ? (
          /*
            These count what has been dropped for conversion. The drive screen
            keeps its own tally under the same two words, and showing both at
            once puts one TRACKS directly below another holding a different
            number.
          */
          tab === "drive" ? (
            <>
              <span className="cell">
                <span className="cell-key">USB</span>
                <span className="cell-val">{sticks.length}</span>
              </span>
              <span className="cell">
                <span className="cell-key">REJECTED</span>
                <span className={refusedSticks > 0 ? "cell-val ng" : "cell-val"}>
                  {refusedSticks}
                </span>
              </span>
            </>
          ) : (
            <>
              <span className="cell">
                <span className="cell-key">TRACKS</span>
                <span className="cell-val">{tracks.length}</span>
              </span>
              <span className="cell">
                <span className="cell-key">REJECTED</span>
                <span className={failing > 0 ? "cell-val ng" : "cell-val"}>
                  {failing}
                </span>
              </span>
              {outcomes !== null && (
                <span className="cell">
                  <span className="cell-key">CONVERTED</span>
                  <span className="cell-val hot">
                    {converted}
                    <small> / {outcomes.length}</small>
                  </span>
                </span>
              )}
            </>
          )
        ) : (
          <>
            <span className="cell">
              <span className="cell-key">
                {busy === "convert" ? "CONVERTING" : "READING"}
              </span>
              <span className="cell-val">
                {progress?.done ?? 0}
                <small> / {progress?.total ?? 0}</small>
              </span>
            </span>
            <span className="meter">
              <i
                style={{
                  width:
                    progress && progress.total > 0
                      ? `${(progress.done / progress.total) * 100}%`
                      : "0%",
                }}
              />
            </span>
            <span className="cell-name">{progress?.name ?? ""}</span>
          </>
        )}
        <span className="push" />
        <span className="modetag">{profile.toUpperCase()}</span>
      </footer>
    </div>
  );
}
