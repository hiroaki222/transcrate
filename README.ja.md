# Transcrate

[English](README.md)

USB に入れる曲を変換して, 現場で鳴るかどうかを出発前に確認する.

Transcrate は ffmpeg で音声を変換し, その結果を CDJ / XDJ が実際に受け付ける
範囲と照合する. コーデック, サンプリングレート, ビット深度, ファイルシステム —
すべてメーカーの説明書に基づく.

**開発初期. ただし変換は動く.** 並列実行, 進捗表示, メタデータ操作はこれから.

## ビルド

Rust 1.88 以降が必要.

```sh
git clone https://github.com/hiroaki222/transcrate
cd transcrate
cargo run -p transcrate-cli -- devices
```

```
DEVICE         YEAR     MP3   AAC    WAV   AIFF   FLAC   ALAC  EXFAT
XDJ-AN         2026     48k   48k    48k    48k    48k    48k  yes
CDJ-3000X      2025     48k   48k    96k    96k    96k    96k  yes
XDJ-AZ         2025     48k   48k    96k    96k    96k    96k  yes
OMNIS-DUO      2024     48k   48k    48k    48k    48k    48k  yes
OPUS-QUAD      2023     48k   48k    96k    96k    96k    96k  yes
XDJ-RX3        2021     48k   48k    48k    48k    48k      -  yes
CDJ-3000       2020     48k   48k    96k    96k    96k    96k  yes
XDJ-XZ         2019     48k   48k    48k    48k    48k      -  sources disagree
XDJ-RR         2018     48k   48k    48k    48k      -      -  no
CDJ-2000NXS2   2016     48k   48k    96k    96k    96k    96k  no
```

フォルダごと変換する. ffmpeg が PATH に必要:

```sh
cargo run -p transcrate-cli -- convert ~/Music/*.flac
```

```
~/Music/track.flac
  FLAC 96 kHz 24-bit -> MP3 44.1 kHz 320 kbps  (encoded)
  ~/Music/_transcrate/track.mp3
~/Music/already-fine.mp3
  MP3 44.1 kHz 320 kbps -> MP3 44.1 kHz 320 kbps  (copied unchanged)
  ~/Music/_transcrate/already-fine.mp3
```

出力は入力と同じ階層の `_transcrate` フォルダに置かれる. 元ファイルには一切
書き込まない. 既に目的の形式になっているファイルは再エンコードせずコピーする.
速いうえに, ロッシー音源を二度潰さずに済む.

変換はコア数ぶん並列で走り, 各行は変換が終わったものから順に出る. 60 秒の
96 kHz FLAC 14 本を MP3 に変換した場合, 逐次で 2.96 秒, 14 コア並列で 0.56 秒
だった (この環境での実測). CPU 時間は同じで, 待ち時間だけが 5 分の 1 になる.
`-j N` で並列数を制限できる.

プロファイルは 3 つ. `-p` で選ぶ:

| プロファイル | 出力 | 用途 |
|---|---|---|
| `cdj-safe` (既定) | MP3 320 kbps, 44.1 kHz | 対応表の全機種で再生できる |
| `lossless` | AIFF, 最大 48 kHz / 24 bit | ロスレスかつ全機種で再生できる |
| `archive` | FLAC, 元のレートと深度のまま | 再生用ではなく保管用 |

形式だけを直接指定することもできる. この場合はコンテナだけが変わり, 元の
サンプリングレートとビット深度はそのまま残る:

```sh
cargo run -p transcrate-cli -- convert ~/Music/track.flac --to aiff
```

指定できるのは `mp3`, `aac`, `alac`, `flac`, `wav`, `aiff`. プロファイルと
違って上限を持たないため, 96 kHz の音源は 96 kHz のまま出力される. 現場に
持っていくなら, 出力を `check` にかけて確認すること.

ビット深度を下げるときは dither を自動で入れる. サンプリングレートの変更では
入れない. dither はそのための処理ではないため.

手持ちのファイルがどの機種で鳴るかを調べる. こちらは `ffprobe` が PATH に必要
(ffmpeg に同梱されている):

```sh
cargo run -p transcrate-cli -- check ~/Music/track.flac
```

```
~/Music/track.flac
  FLAC 96 kHz 24-bit
  plays on       CDJ-3000X, XDJ-AZ, OPUS-QUAD, CDJ-3000, CDJ-2000NXS2
  XDJ-AN         96 kHz is not supported for FLAC
  OMNIS-DUO      96 kHz is not supported for FLAC
  XDJ-RX3        96 kHz is not supported for FLAC
  XDJ-XZ         96 kHz is not supported for FLAC
  XDJ-RR         FLAC is not supported
```

実際に持っていく機材だけに絞ることもできる:

```sh
cargo run -p transcrate-cli -- check ~/Music/*.flac --device cdj-3000,xdj-rr
```

1 つでも弾かれた場合は非ゼロで終了するので, スクリプトの判定に使える.

### シェル補完

```sh
mkdir -p ~/.zfunc
transcrate completions zsh > ~/.zfunc/_transcrate
```

`~/.zshrc` に以下を追加する:

```sh
fpath=("$HOME/.zfunc" $fpath)
autoload -Uz compinit && compinit
```

`bash`, `fish`, `powershell`, `elvish` にも対応. 機種 ID も補完されるので,
`--device <TAB>` で 10 機種が一覧される.

zsh ではファイル引数の補完が音声ファイルとディレクトリだけになる. 曲と同じ
フォルダに置いてあるジャケット画像や PDF は候補に出ない.

テストと lint. CI で走るものと同じ:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## なぜ作るのか

DJ 機材は互いに仕様が食い違っていて, しかもその食い違い方が直感に反する.

- **2016 年の CDJ-2000NXS2 は 96 kHz の FLAC を再生する. 2026 年の XDJ-AN は
  48 kHz で止まる.** 新しい方が高性能とは限らない.
- **その CDJ-2000NXS2 は exFAT の USB を読めない.** 2020 年以降の機種はすべて
  読めるが, XDJ-RX3 は exFAT を読める代わりに 96 kHz を拒否する. 制約が交差
  するので, 機種を性能順に並べることができない.
- **`.m4a` の中身は AAC と ALAC のどちらでもありうる.** AAC しか受け付けない
  機種は ALAC に対して `E-8305` を返すが, 拡張子からは何も分からない.

どれか 1 つ外すと, ブースに立ってから気づくことになる.

## データの出どころ

表の数値はすべてメーカーの説明書から取っている. 根拠とした文書の型番は
[docs/device-compatibility.ja.md](docs/device-compatibility.ja.md) に記録して
ある.

公式の情報同士が食い違う箇所 (XDJ-XZ の exFAT) は, どちらかを選ばずに
「食い違っている」と表示する.

## これから作るもの

- WAV / FLAC / AIFF / M4A / MP3 の相互変換
- 上の表に基づく, 機種ごとの警告
- USB の診断. 読み取り専用で, ドライブには一切書き込まない
- メタデータをフィールド単位で保持 / 削除 / 上書き
- CLI と GUI で共有するプロファイル
- 同じコアの上に載る macOS / Windows 向け GUI

## 配布について

まだリリースはない. 始めたらこうなる.

- **CLI** — Homebrew tap とビルド済みバイナリ.
- **GUI** — `.dmg` と `.msi`. **署名はしない.** Apple の開発者証明書は年 $99
  かかり, 使う人がいない段階で払う理由が薄い. 署名がないと macOS は初回起動を
  ブロックするので, Apple の手順に従って開く:
  [開発元が不明なMacアプリを開く][unsigned-mac]. **一度やれば次からは不要**.
  Windows では SmartScreen が出るので **「詳細情報」→「実行」**.

[unsigned-mac]: https://support.apple.com/ja-jp/guide/mac-help/mh40616/mac

## ライセンス

[MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) の好きな方.

ffmpeg は別プロセスとして実行され, このプログラムにリンクされることはない.
配布ビルドには LGPL 版を同梱するが, システムに入っていればそちらを優先する.
