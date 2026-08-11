# Transcrate

[English](README.md)

USB に入れる曲を変換して, 現場で鳴るかどうかを出発前に確認する.

Transcrate は ffmpeg で音声を変換し, その結果を CDJ / XDJ が実際に受け付ける
範囲と照合する. コーデック, サンプリングレート, ビット深度, ファイルシステム —
すべてメーカーの説明書に基づく.

**開発初期.** `transcrate devices` は動く. 変換機能はまだない.

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

## ライセンス

[MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) の好きな方.

ffmpeg は別プロセスとして実行され, このプログラムにリンクされることはない.
配布ビルドには LGPL 版を同梱するが, システムに入っていればそちらを優先する.
