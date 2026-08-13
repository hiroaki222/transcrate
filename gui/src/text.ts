import type { AudioSpec, Codec, Issue, Lamp } from "./api";
import type { Strings } from "./strings";

const CODECS: Record<Codec, string> = {
  mp3: "MP3",
  "aac-lc": "AAC",
  alac: "ALAC",
  flac: "FLAC",
  "pcm-wav": "WAV",
  "pcm-aiff": "AIFF",
};

export const codecName = (codec: Codec) => CODECS[codec];

export const hz = (value: number) => value.toLocaleString("en-US");

/** `FLAC  96,000 Hz  24 bit`, `MP3  320 kbps  44,100 Hz` */
export function describeSpec(spec: AudioSpec): string {
  const parts = [codecName(spec.codec)];
  if (spec.bitrate_kbps !== null) parts.push(`${spec.bitrate_kbps} kbps`);
  parts.push(`${hz(spec.sample_rate_hz)} Hz`);
  if (spec.bit_depth !== null) parts.push(`${spec.bit_depth} bit`);
  return parts.join("  ");
}

/**
 * Why a player refuses a file, in plain words.
 *
 * The whole point of this screen is that nobody has to look up `E-8305`
 * while standing in a booth.
 */
export function describeIssue(t: Strings, issue: Issue): string {
  const codec = codecName(issue.codec);

  switch (issue.kind) {
    case "codec-unsupported":
      return t.issue.codec(codec);
    case "sample-rate-unsupported":
      return t.issue.sampleRate(codec, hz(issue.requested_hz));
    case "bit-depth-unsupported":
      return t.issue.bitDepth(codec, issue.requested_bits);
    case "bitrate-out-of-range":
      return t.issue.bitrate(
        codec,
        issue.requested_kbps,
        issue.allowed_kbps[0],
        issue.allowed_kbps[1],
      );
  }
}

/**
 * Collapse the players that failed for the same reason into one line.
 *
 * Ten separate lines saying the same thing do not get read.
 */
export function groupReasons(t: Strings, lamps: Lamp[]): { reason: string; devices: string[] }[] {
  const grouped = new Map<string, string[]>();

  for (const lamp of lamps) {
    if (lamp.ok) continue;
    for (const issue of lamp.issues) {
      const reason = describeIssue(t, issue);
      const devices = grouped.get(reason) ?? [];
      devices.push(lamp.name);
      grouped.set(reason, devices);
    }
  }

  return [...grouped].map(([reason, devices]) => ({ reason, devices }));
}
