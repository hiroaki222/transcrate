# Transcrate

[English](README.md)

DJ 向けのオーディオ変換ツール. ffmpeg をバックエンドに使い, 手持ちの機材で実際に
再生できるファイルを作る.

フォルダを指定して, 持っていく機材を選ぶだけでいい. 再生できない設定になっていれば
変換前に警告する.

## なぜ作るのか

USB に曲を入れる作業は単純に見えて, 機材の仕様が食い違っているせいで詰まる.

- 2016 年の CDJ-2000NXS2 は 96 kHz の FLAC を再生できる. 2026 年の XDJ-AN は
  48 kHz で頭打ちになる. 新しいから高性能とは限らない.
- その CDJ-2000NXS2 は exFAT の USB を読めない. 2020 年以降の機種はすべて読める.
  一方 XDJ-RX3 は exFAT を読めるが 96 kHz は再生できない. 制約が交差するので,
  機種を性能順に一直線に並べることができない.
- `.m4a` には AAC と ALAC の両方が入りうる. AAC しか受け付けない機種は ALAC に
  対してエラー `E-8305` を返すが, 拡張子を見ても区別がつかない.

Transcrate はメーカーが公表している制限を 1 つの表にまとめ, 実際に挿す機材に対して
出力を照合する. ブースに立つ前に問題が分かる.

## 現在動くもの

互換性テーブルと `devices` コマンド. 変換機能はまだ実装していない.

## 必要なもの

- Rust 1.88 以降

ffmpeg は現時点では不要. 変換機能を実装した時点で別プロセスとして呼び出す.
システムに入っていればそれを優先し, なければ同梱の LGPL ビルドを使う.

## ビルドと実行

```sh
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

テストと lint. CI で走るものと同じ 3 つ:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## 互換性データについて

表に載っている数値はすべてメーカーの操作説明書に基づく. 根拠とした文書の型番は
[docs/device-compatibility.ja.md](docs/device-compatibility.ja.md) に記録してある.

公式の情報同士が食い違っている箇所 (XDJ-XZ の exFAT) は, どちらかを採用せず
「矛盾している」という状態のまま記録する.

## 実装予定

- WAV / FLAC / AIFF / M4A / MP3 の相互変換. DJ 用途に適したデフォルト値付き
- 上記の表に基づく機種ごとの警告
- USB の診断. 読み取り専用で, ドライブへの書き込みやフォーマットは一切しない
- メタデータをフィールド単位で制御 (保持 / 削除 / 上書き)
- CLI と GUI で共有するプロファイル
- 同じコアの上に載る Tauri 製 GUI (macOS / Windows)

## ライセンス

[MIT](LICENSE-MIT) または [Apache-2.0](LICENSE-APACHE) のデュアルライセンス.
好きな方を選んで使える.
