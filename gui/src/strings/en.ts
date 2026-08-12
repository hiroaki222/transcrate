import type { Strings } from "./index";

/** English. Typed against the Japanese set, so a missing key will not compile. */
export const en: Strings = {
  profiles: {
    "cdj-safe": {
      label: "Plays on everything",
      note: "MP3 320 kbps / 44.1 kHz. Every one of the 10 supported players will read it.",
    },
    lossless: {
      label: "Lossless, still plays on everything",
      note: "AIFF up to 48 kHz / 24 bit. Nothing is thrown away, and all 10 players read it.",
    },
    archive: {
      label: "Keep a copy (no playback promise)",
      note: "FLAC at the source's own rate and depth. Players may refuse it.",
    },
    aiff: {
      label: "AIFF (container only)",
      note: "Rate and bit depth stay as they are, so some players will refuse it.",
    },
    wav: {
      label: "WAV (container only)",
      note: "Rate and bit depth stay as they are, so some players will refuse it.",
    },
    flac: {
      label: "FLAC (container only)",
      note: "Rate and bit depth stay as they are, so some players will refuse it.",
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
    pick: "Choose tracks",
    convert: (count) => `Convert ${count}`,
  },

  dialog: {
    pickTracks: "Choose tracks or a folder",
    pickDrive: "Choose a drive",
  },

  done: {
    converted: (count) => `Converted ${count}`,
    failed: (count) => `${count} could not be converted`,
    reveal: "Show where they went",
    dismiss: "Close",
  },

  empty: {
    title: "Drop tracks or a folder here",
    note: "A folder is swept for audio; everything else in it is left alone.",
  },

  track: {
    unreadable: "Cannot be read",
    dither: "dithered",
    lampsNow: "NOW",
    lampsAfter: "AFTER",
    playsOn: (name) => `${name} — plays`,
    failsOn: (name) => `${name} — will not play`,
    reasonCount: (count) => `${count} player${count === 1 ? "" : "s"}`,
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
    pick: "Choose a drive",
    readOnly: "READ ONLY",
    emptyTitle: "Choose a drive to see which players will read it",
    emptyNote: "Nothing is written to it, and it is never formatted.",
    nothingMounted: (path) => `Nothing is mounted at ${path}`,
    lamps: "READS",
    allRead: (count) => `All ${count} players read this drive.`,
    someFail: (count) => `${count} players will not read this drive.`,
    failReason: (filesystem, names) => `Does not read ${filesystem}. ${names}`,
    fix: "Fix",
    fixNote: (count) => `Formatted FAT32, all ${count} would read it.`,
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
