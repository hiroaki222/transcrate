import { invoke } from "@tauri-apps/api/core";

export type Codec = "mp3" | "aac-lc" | "alac" | "flac" | "pcm-wav" | "pcm-aiff";

export type AudioSpec = {
  codec: Codec;
  sample_rate_hz: number;
  bit_depth: number | null;
  bitrate_kbps: number | null;
};

export type Issue =
  | { kind: "codec-unsupported"; codec: Codec }
  | { kind: "sample-rate-unsupported"; codec: Codec; requested_hz: number }
  | { kind: "bit-depth-unsupported"; codec: Codec; requested_bits: number }
  | {
      kind: "bitrate-out-of-range";
      codec: Codec;
      requested_kbps: number;
      allowed_kbps: [number, number];
    };

export type Lamp = {
  id: string;
  name: string;
  short: string;
  ok: boolean;
  issues: Issue[];
};

export type Track = {
  path: string;
  name: string;
  source: AudioSpec | null;
  output: AudioSpec | null;
  outputPath: string | null;
  action: "copy" | "retag" | "encode" | null;
  dither: boolean;
  now: Lamp[];
  after: Lamp[];
  error: string | null;
};

export type DeviceRow = {
  id: string;
  name: string;
  short: string;
  year: number;
  /** In the order mp3, aac, wav, aiff, flac, alac. `null` where unsupported. */
  ratesHz: (number | null)[];
  exfat: boolean;
  maxFolderDepth: number;
};

export type Mounted = {
  mountPoint: string;
  name: string;
  filesystem: string | null;
  reportedAs: string;
  /** How many of the chosen players read it, out of how many were chosen. */
  readable: number;
  players: number;
  totalBytes: number;
  freeBytes: number;
};

export type Drive = {
  mountPoint: string;
  name: string;
  filesystem: string | null;
  reportedAs: string;
  lamps: Lamp[];
  readable: number;
};

export type Crowded = { folder: string; entries: number };

export type FailingTrack = {
  path: string;
  name: string;
  folder: string;
  spec: AudioSpec | null;
  lamps: Lamp[];
  error: string | null;
};

/** What is on a drive, judged against the limits of the chosen players. */
export type Contents = {
  tracks: number;
  folders: number;
  otherFiles: number;
  deepest: number;
  depthLimit: number;
  entryLimit: number | null;
  unreachable: string[];
  crowded: Crowded[];
  /** Only the tracks at least one player refuses. */
  failing: FailingTrack[];
};

export type Outcome = {
  path: string;
  name: string;
  outputPath: string;
  error: string | null;
};

export type Tools = { ffmpeg: boolean; ffprobe: boolean };

export type Progress = { done: number; total: number; name: string };

export type ConvertOptions = {
  profile: string;
  keepComment: boolean;
  artwork: boolean;
  devices: string[];
};

export const tools = () => invoke<Tools>("tools");

export const locale = () => invoke<string | null>("locale");

export const devices = () => invoke<DeviceRow[]>("devices");

export const inspect = (paths: string[], settings: ConvertOptions) =>
  invoke<Track[]>("inspect", { paths, settings });

export const convertAll = (paths: string[], settings: ConvertOptions) =>
  invoke<Outcome[]>("convert_all", { paths, settings });

export const drives = (settings: ConvertOptions) =>
  invoke<Mounted[]>("drives", { settings });

export const checkDrive = (path: string, settings: ConvertOptions) =>
  invoke<Drive | null>("check_drive", { path, settings });

export const scanDrive = (path: string, settings: ConvertOptions) =>
  invoke<Contents | null>("scan_drive", { path, settings });
