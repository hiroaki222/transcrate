/** Japanese, the language this was designed in. */

export const ja = {
  // Named by what each choice guarantees, not by the format it produces.
  profiles: {
    "cdj-safe": {
      label: "現場で確実に再生",
      format: "MP3  320 kbps  44.1 kHz",
      note: "対応する10機種すべてで再生できます。",
    },
    lossless: {
      label: "音質を保って現場で再生",
      format: "AIFF  最大 48 kHz  24 bit",
      note: "音質を落とさずに、10機種すべてで再生できます。",
    },
    archive: {
      label: "保存用（再生保証なし）",
      format: "FLAC  元のレートと深度のまま",
      note: "機材で再生できるとは限りません。",
    },
    aiff: {
      label: "AIFFに変換",
      format: "AIFF  元のレートと深度のまま",
      note: "機材によっては再生できません。",
    },
    wav: {
      label: "WAVに変換",
      format: "WAV  元のレートと深度のまま",
      note: "機材によっては再生できません。",
    },
    flac: {
      label: "FLACに変換",
      format: "FLAC  元のレートと深度のまま",
      note: "機材によっては再生できません。",
    },
  } as Record<string, { label: string; format: string; note: string }>,

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
    clear: "すべて外す",
    convert: (count: number) => `${count}曲を変換`,
  },

  confirm: {
    cancel: "やめる",
    clearTitle: "リストの曲をすべて外しますか",
    clearNote: (count: number) =>
      `${count}曲がリストから消えます。ファイル自体は消えません。`,
    clearGo: "すべて外す",
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
    remove: "リストから外す",
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
    picking: "接続されているUSBを探しています",
    none: "USBが見つかりません。挿してから、もう一度お試しください。",
    refresh: "再検索",
    unreadable: "どの機材も読めません",
    readOnly: "READ ONLY",
    count: (n: number) => `${n} 枚`,
    free: (n: number) => `空き ${n.toFixed(1)} GB`,
    gb: (n: number) => `${n.toFixed(1)} GB`,
    capacity: "空き容量",
    format: "ファイル形式",
    refused: "認識できない機材",
    refusedNone: "なし",
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

  scan: {
    title: "中身",
    reading: (done: number, total: number) =>
      `${done.toLocaleString()} / ${total.toLocaleString()}曲を確認中`,
    summary: (tracks: number, folders: number, deepest: number) =>
      `${tracks.toLocaleString()}曲、${folders.toLocaleString()}フォルダ、最大${deepest}階層`,
    otherFiles: (count: number) =>
      `ほかに${count.toLocaleString()}件、機材の一覧に出ないファイルがあります。`,
    noTracks: "曲が見つかりませんでした。",
    allPlay: (count: number) =>
      `${count.toLocaleString()}曲すべてが、選んだ機材で再生できます。`,
    someFail: (plays: number, total: number) =>
      `${total.toLocaleString()}曲のうち${plays.toLocaleString()}曲は、選んだ機材で再生できます。`,

    // The drive mounts and the files are there — the browser simply stops.
    // Saying only "too deep" leaves it sounding cosmetic.
    deepTitle: (count: number) =>
      `${count.toLocaleString()}フォルダが、機材の画面に出てきません`,
    deepNote: (limit: number) =>
      `${limit}階層までしか表示されません。中の曲は選べません。`,
    crowdedTitle: (count: number) =>
      `${count.toLocaleString()}フォルダが、最後まで表示されません`,
    crowdedNote: (limit: number) =>
      `1フォルダに表示されるのは${limit.toLocaleString()}件までです。`,
    crowdedEntries: (entries: number) => `${entries.toLocaleString()}件`,
    failingTitle: (count: number) =>
      `${count.toLocaleString()}曲に、再生できない機材があります`,
    failingNote: "CONVERTタブに入れると、変換後の判定を確認できます。",
    root: "USB直下",
    andMore: (rest: number) => `ほか${rest.toLocaleString()}件`,
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
