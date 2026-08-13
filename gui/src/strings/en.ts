import type { Strings } from "./index";

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

  scan: {
    title: "CONTENTS",
    reading: (done, total) =>
      `Reading ${done.toLocaleString()} of ${total.toLocaleString()}`,
    summary: (tracks, folders, deepest) =>
      `${tracks.toLocaleString()} tracks, ${folders.toLocaleString()} folders, ${deepest} levels deep`,
    otherFiles: (count) =>
      `${count.toLocaleString()} other files, which no player lists.`,
    noTracks: "No tracks on this drive.",
    allPlay: (count) =>
      `All ${count.toLocaleString()} tracks play on the chosen players.`,
    someFail: (plays, total) =>
      `${plays.toLocaleString()} of ${total.toLocaleString()} tracks play on the chosen players.`,

    // The drive mounts and the files are there — the browser simply stops.
    // Saying only "too deep" leaves it sounding cosmetic.
    deepTitle: (count) =>
      `${count.toLocaleString()} folders never appear on the player`,
    deepNote: (limit) =>
      `The browser stops at ${limit} levels. Nothing inside these can be selected.`,
    crowdedTitle: (count) =>
      `${count.toLocaleString()} folders are cut short on the player`,
    crowdedNote: (limit) =>
      `A folder lists at most ${limit.toLocaleString()} entries.`,
    crowdedEntries: (entries) => `${entries.toLocaleString()} entries`,
    failingTitle: (count) =>
      `${count.toLocaleString()} tracks at least one player will not take`,
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
