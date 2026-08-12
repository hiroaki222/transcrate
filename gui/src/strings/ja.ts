/** Japanese, the language this was designed in. */

export const ja = {
  // Named by what each choice guarantees, not by the format it produces.
  profiles: {
    "cdj-safe": {
      label: "現場で確実に再生",
      note: "MP3 320 kbps / 44.1 kHz。対応する10機種すべてで再生できます。",
    },
    lossless: {
      label: "音質を保って現場で再生",
      note: "AIFF 最大48 kHz / 24 bit。音質を落とさずに、10機種すべてで再生できます。",
    },
    archive: {
      label: "保存用（再生保証なし）",
      note: "FLAC。元の音質のまま残します。機材で再生できるとは限りません。",
    },
    aiff: {
      label: "AIFFに変換（形式のみ）",
      note: "サンプルレートとビット深度は元のまま。機材によっては再生できません。",
    },
    wav: {
      label: "WAVに変換（形式のみ）",
      note: "サンプルレートとビット深度は元のまま。機材によっては再生できません。",
    },
    flac: {
      label: "FLACに変換（形式のみ）",
      note: "サンプルレートとビット深度は元のまま。機材によっては再生できません。",
    },
  } as Record<string, { label: string; note: string }>,

  settings: {
    open: "設定",
    language: "言語",
    auto: "自動（OSに合わせる）",
  },

  toolbar: {
    target: "変換先",
    more: "形式を直接指定",
    less: "閉じる",
    players: "使用機材",
    allPlayers: (count: number) => `全${count}機種`,
    somePlayers: (count: number) => `${count}機種`,
    selectAll: "すべて選択",
    keepComment: "コメントを残す",
    keepArtwork: "ジャケットを残す",
    pick: "曲を選ぶ",
    convert: (count: number) => `${count}曲を変換`,
  },

  dialog: {
    pickTracks: "曲またはフォルダを選択",
    pickDrive: "USBを選択",
  },

  done: {
    converted: (count: number) => `${count}曲を変換しました`,
    failed: (count: number) => `${count}曲は変換できませんでした`,
    reveal: "保存先を開く",
    dismiss: "閉じる",
  },

  empty: {
    title: "曲またはフォルダをここにドロップ",
    note: "フォルダを選ぶと、中の音声ファイルだけを読み込みます。",
  },

  track: {
    unreadable: "読み込めません",
    dither: "ディザ",
    lampsNow: "NOW",
    lampsAfter: "変換後",
    playsOn: (name: string) => `${name} — 再生できます`,
    failsOn: (name: string) => `${name} — 再生できません`,
    reasonCount: (count: number) => `${count}機種`,
  },

  action: {
    copy: "そのままコピー",
    retag: "曲情報だけ更新",
    encode: "変換",
  },

  verdict: {
    allPlay: (count: number) => `${count}機種すべてで再生できます`,
    nonePlay: (count: number) => `${count}機種すべてで再生できません`,
    somePlay: (count: number) => `${count}機種で再生できません`,
  },

  issue: {
    codec: (codec: string) => `${codec}に対応していません`,
    sampleRate: (codec: string, hz: string) =>
      `${codec}は${hz} Hzに対応していません`,
    bitDepth: (codec: string, bits: number) =>
      `${codec}は${bits} bitに対応していません`,
    bitrate: (codec: string, kbps: number, low: number, high: number) =>
      `${codec}は${kbps} kbpsに対応していません（対応範囲：${low}〜${high} kbps）`,
  },

  drive: {
    pick: "USBを選ぶ",
    readOnly: "READ ONLY",
    emptyTitle: "USBを選ぶと、対応機材を確認できます",
    emptyNote: "USBには書き込みません。初期化もしません。",
    nothingMounted: (path: string) => `${path}には何もマウントされていません`,
    lamps: "認識",
    allRead: (count: number) => `${count}機種すべてがこのUSBを認識します。`,
    someFail: (count: number) => `${count}機種がこのUSBを認識しません。`,
    failReason: (filesystem: string, names: string) =>
      `${filesystem}を認識しません。${names}`,
    fix: "対処",
    fixNote: (count: number) =>
      `FAT32で初期化すると、${count}機種すべてで認識できます。`,
  },

  devices: {
    yes: "可",
    no: "不可",
    source:
      "数値はメーカーの取扱説明書に基づいています。公式資料の記述が一致しない場合は、再生できないものとして判定しています。",
  },

  status: {
    ffmpegMissing: "ffmpegが見つかりません",
  },
};
