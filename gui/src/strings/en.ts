import type { Strings } from "./index";

/**
 * A count and its noun, agreeing.
 *
 * Japanese needs none of this, so the shared string keys take a bare number and
 * the agreement has to happen here.
 */
const many = (count: number, noun: string) =>
  `${count.toLocaleString()} ${noun}${count === 1 ? "" : "s"}`;

/** English. Typed against the Japanese set, so a missing key will not compile. */
export const en: Strings = {
  profiles: {
    "cdj-safe": {
      label: "Plays on everything",
      format: "MP3  320 kbps  44.1 kHz",
      note: "Every one of the 10 supported players will read it.",
    },
    lossless: {
      label: "Lossless, plays on everything",
      format: "AIFF  up to 48 kHz  24 bit",
      note: "Nothing is thrown away, and all 10 players still read it.",
    },
    archive: {
      label: "Keep a copy (no playback promise)",
      format: "FLAC  source rate and depth",
      note: "Players may refuse it.",
    },
    aiff: {
      label: "Convert to AIFF",
      format: "AIFF  source rate and depth",
      note: "Some players will refuse it.",
    },
    wav: {
      label: "Convert to WAV",
      format: "WAV  source rate and depth",
      note: "Some players will refuse it.",
    },
    flac: {
      label: "Convert to FLAC",
      format: "FLAC  source rate and depth",
      note: "Some players will refuse it.",
    },
  },

  settings: {
    open: "Settings",
    language: "Language",
    auto: "Automatic (follow the system)",
  },

  toolbar: {
    target: "Convert to",
    more: "Name a format",
    less: "Close",
    players: "Gear",
    allPlayers: (count) => `All ${count}`,
    somePlayers: (count) => `${count} of them`,
    selectAll: "Select all",
    keepComment: "Keep comments",
    keepArtwork: "Keep artwork",
    clear: "Clear",
    convert: (count) => `Convert ${count}`,
  },

  confirm: {
    cancel: "Keep them",
    clearTitle: "Take every track out of the list?",
    clearNote: (count) =>
      `${count} tracks leave the list. Not one file is deleted.`,
    clearGo: "Clear the list",
  },

  dialog: {
    pickTracks: "Choose tracks",
  },

  done: {
    converted: (count) => `Converted ${count}`,
    failed: (count) => `${count} could not be converted`,
    dismiss: "Close",
  },

  empty: {
    title: "Drop tracks or a folder here",
    note: "A folder is swept for audio; everything else in it is left alone.",
  },

  track: {
    unreadable: "Cannot be read",
    remove: "Take out of the list",
    dither: "dithered",
    thin: "cannot be improved",
    lampsNow: "NOW",
    lampsAfter: "AFTER",
    playsOn: (name) => `${name} — plays`,
    failsOn: (name) => `${name} — will not play`,
    reasonCount: (count) => `${count} player${count === 1 ? "" : "s"}`,
    // What is left after the conversion, not what the file arrived as.
    mended: (count) => `Converted, it plays on all ${count}`,
    reasonDetail: (reason, devices) => `${reason}. ${devices.join(", ")}`,
  },

  action: {
    copy: "copied as is",
    retag: "tags only",
    encode: "encoded",
  },

  verdict: {
    allPlay: (count) => `Plays on all ${count}`,
    nonePlay: (count) => `Plays on none of the ${count}`,
    somePlay: (count) => `Will not play on ${count}`,
  },

  issue: {
    codec: (codec) => `${codec} is not supported`,
    sampleRate: (codec, hz) => `${hz} Hz is not supported for ${codec}`,
    bitDepth: (codec, bits) => `${bits}-bit is not supported for ${codec}`,
    bitrate: (codec, kbps, low, high) =>
      `${kbps} kbps is outside what ${codec} allows (${low}–${high} kbps)`,
  },

  drive: {
    picking: "Looking for drives",
    none: "No drive found. Plug one in and look again.",
    refresh: "Look again",
    unreadable: "No player reads this",
    readOnly: "READ ONLY",
    count: (n) => (n === 1 ? "1 drive" : `${n} drives`),
    free: (n) => `${n.toFixed(1)} GB free`,
    gb: (n) => `${n.toFixed(1)} GB`,
    capacity: "Free space",
    format: "Filesystem",
    refused: "Will not read it",
    refusedNone: "none",
    refusedNames: (names) => names.join(", "),
    emptyTitle: "Choose a drive to see which players will read it",
    emptyNote: "Nothing is written to it, and it is never formatted.",
    nothingMounted: (path) => `Nothing is mounted at ${path}`,
    lamps: "READS",
    allRead: (count) => `All ${count} players read this drive.`,
    someFail: (count) => `${count} players will not read this drive.`,
  },

  scan: {
    title: "CONTENTS",
    otherFiles: (count) => `${many(count, "other file")}, which no player lists.`,
    noTracks: "No tracks on this drive.",
    allPlay: (count) => `All ${many(count, "track")} play on the chosen players.`,
    // "will play" rather than "play", which would have to agree with the first
    // number: "1 of 2 tracks plays" is correct and reads like a mistake.
    someFail: (plays, total) =>
      `${plays.toLocaleString()} of ${many(total, "track")} will play on the chosen players.`,

    /*
      All three are a count and a relative clause, never a count and a verb.
      "1 folder never appears" and "2 folders never appear" would each need
      their own agreement; "1 folder the player never shows" needs none.
    */
    // The drive mounts and the files are there — the browser simply stops.
    // Saying only "too deep" leaves it sounding cosmetic.
    deepTitle: (count) => `${many(count, "folder")} the player never shows`,
    deepNote: (limit) =>
      `The browser stops at ${limit} levels. Nothing inside these can be selected.`,
    crowdedTitle: (count) => `${many(count, "folder")} the player cuts short`,
    crowdedNote: (limit) =>
      `A folder lists at most ${limit.toLocaleString()} entries.`,
    crowdedEntries: (entries) =>
      `${entries.toLocaleString()} ${entries === 1 ? "entry" : "entries"}`,
    unreadableTitle: (count) => `${many(count, "folder")} that could not be read`,
    unreadableNote: "Nothing inside them was counted or checked.",
    // Placed under the count, where it changes how the count should be read.
    partial: "The folders below hold tracks that are not in these counts.",
    failingTitle: (count) =>
      `${many(count, "track")} at least one player will not take`,
    failingNote: "Drop them on CONVERT to see what they would become.",
    root: "drive root",
    andMore: (rest) => `and ${rest.toLocaleString()} more`,
  },

  devices: {
    yes: "yes",
    no: "no",
    source:
      "Every figure comes from a manufacturer's manual. Where official sources contradict each other, the stricter reading is used.",
  },

  status: {
    ffmpegMissing: "ffmpeg not found",
  },
};
