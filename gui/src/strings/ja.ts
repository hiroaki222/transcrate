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
      note: "圧縮せずに、10機種すべてで再生できます。48 kHz を超える曲はレートを下げます。",
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
    pickTracks: "曲を選択",
  },

  done: {
    converted: (count: number) => `${count}曲を変換しました`,
    failed: (count: number) => `${count}曲は変換できませんでした`,
    dismiss: "閉じる",
  },

  empty: {
    title: "曲またはフォルダをここにドロップ",
    note: "フォルダをドロップすると、中の音声ファイルだけを読み込みます。",
  },

  track: {
    unreadable: "読み込めません",
    // What to do about it, rather than what is wrong with it. Every row here
    // is a file about to be converted, and "再生できません" reads as a dead end
    // for a state the conversion settles.
    convert: "変換が必要",
    remove: "リストから外す",
    dither: "ディザ",
    // Read on hover, and by anything reading the row aloud. The bitrate is
    // already beside it; this says what that number means.
    thin: "音質が悪いファイルです",
    thinNote: "変換しても、元より良くはなりません。",
    lampsNow: "変換前",
    // Alone, with no second strip beside it, "変換前" would send the reader
    // looking for the half that is not there.
    lampsOnly: "現在",
    lampsAfter: "変換後",
    playsOn: (name: string) => `${name} — 再生できます`,
    failsOn: (name: string) => `${name} — 再生できません`,
    reasonCount: (count: number) => `${count}機種`,
    // Punctuation belongs to the language, not to whichever component happens
    // to be joining the parts up.
    reasonDetail: (reason: string, devices: string[]) =>
      `${reason}。${devices.join("、")}`,
  },

  // The subject of every one of these is the player, named alongside them.
  // `${codec}は` made the codec the subject instead — "FLAC does not support
  // 96,000 Hz" rather than "it does not support FLAC at 96,000 Hz" — which
  // blames the format for a limit belonging to the hardware.
  issue: {
    codec: (codec: string) => `${codec}に対応していません`,
    sampleRate: (codec: string, hz: string) =>
      `${codec}の${hz} Hzに対応していません`,
    bitDepth: (codec: string, bits: number) =>
      `${codec}の${bits} bitに対応していません`,
    bitrate: (codec: string, kbps: number, low: number, high: number) =>
      `${codec}の${kbps} kbpsに対応していません（対応範囲：${low}〜${high} kbps）`,
  },

  drive: {
    picking: "接続されているUSBを探しています",
    none: "USBが見つかりません。挿してから、もう一度お試しください。",
    refresh: "再検索",
    unreadable: "どの機材も読めません",
    readOnly: "READ ONLY",
    count: (n: number) => `${n}枚`,
    free: (n: number) => `空き ${n.toFixed(1)} GB`,
    gb: (n: number) => `${n.toFixed(1)} GB`,
    capacity: "空き容量",
    format: "ファイル形式",
    refused: "認識できない機材",
    refusedNone: "なし",
    refusedNames: (names: string[]) => names.join("、"),
    emptyTitle: "USBを選ぶと、対応機材を確認できます",
    emptyNote: "USBには書き込みません。初期化もしません。",
    nothingMounted: (path: string) => `${path}には何もマウントされていません`,
    lamps: "認識",
    allRead: (count: number) => `${count}機種すべてがこのUSBを認識します。`,
    someFail: (count: number) => `${count}機種がこのUSBを認識しません。`,
  },

  scan: {
    title: "中身",
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
    unreadableTitle: (count: number) =>
      `${count.toLocaleString()}フォルダが、読み取れませんでした`,
    unreadableNote: "中身は曲数にも判定にも入っていません。",
    // Placed under the count, where it changes how the count should be read.
    partial: "下に挙げたフォルダの中の曲は、上の数に入っていません。",
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
