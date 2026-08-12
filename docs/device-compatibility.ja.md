# 機種別互換性データ

[English](device-compatibility.md)

`crates/transcrate-core/src/device.rs` の元データ。あのテーブルに入っている値は
すべて、ここに挙げた文書まで遡れる。根拠を示せない行はテーブルに入れない。

## 対象範囲

- **USB のみ.** CD と SD には別の制限がかかるが、Transcrate はそれを扱わない。
  特に CDJ-2000NXS2 は、ディスクに焼いた FLAC / ALAC / 88.2・96 kHz の PCM を
  再生できない。同じ機種でも USB からなら再生できる。
- **24 kHz 以下のサンプリングレートは省く.** いくつかの説明書には MPEG-2
  Layer-3 の行 (16 / 22.05 / 24 kHz) や低レートの AAC が載っている。再生は
  できるが、そこへ *変換する* 理由がないため、「再生できるすべて」ではなく
  「変換先として意味のある範囲」を記述している。
- **10 機種.** クラブやバーで現役の機種に絞る。生産終了した MP3 専用機
  (CDJ-200, CDJ-400, CDJ-800MK2) は対象外。

## 対応フォーマット

コーデックごとの、公表されている最大サンプリング周波数。`-` はその機種の
フォーマット表にコーデック自体が載っていないことを示す。

| 機種 | 発売 | MP3 | AAC | WAV | AIFF | FLAC | ALAC | exFAT |
|---|---|---|---|---|---|---|---|---|
| XDJ-AN | 2026 | 48k | 48k | 48k | 48k | 48k | 48k | 可 |
| CDJ-3000X | 2025 | 48k | 48k | 96k | 96k | 96k | 96k | 可 |
| XDJ-AZ | 2025 | 48k | 48k | 96k | 96k | 96k | 96k | 可 |
| OMNIS-DUO | 2024 | 48k | 48k | 48k | 48k | 48k | 48k | 可 |
| OPUS-QUAD | 2023 | 48k | 48k | 96k | 96k | 96k | 96k | 可 |
| XDJ-RX3 | 2021 | 48k | 48k | 48k | 48k | 48k | - | 可 |
| CDJ-3000 | 2020 | 48k | 48k | 96k | 96k | 96k | 96k | 可 |
| XDJ-XZ | 2019 | 48k | 48k | 48k | 48k | 48k | - | 情報が矛盾 |
| XDJ-RR | 2018 | 48k | 48k | 48k | 48k | - | - | 不可 |
| CDJ-2000NXS2 | 2016 | 48k | 48k | 96k | 96k | 96k | 96k | 不可 |

全機種に共通する事項: MP3 と AAC は 16 bit のみ、上限 48 kHz. ロスレス系は
16 bit と 24 bit に対応。フォルダ階層は 8 階層まで。1 フォルダあたり 10,000
件までしか表示されない。NTFS は非対応。USB ハブは非対応。

MP3 のビットレートは全機種 32〜320 kbps, AAC は 16〜320 kbps. 2021 年までの
世代は 32 kHz も対応表に載せているが、2023 年以降の説明書ではこの行が消えて
いる。この点では新しい機種の方が対応範囲が狭い。

## 実装に影響する注記

**サンプリングレートの上限はフォーマットごとに決まる。機種ごとではない.**
CDJ-3000 は 96 kHz の FLAC を再生するが、MP3 は 48 kHz までしか受け付けない。
機種に「最大サンプリングレート」という属性を 1 つ持たせると、半分のフォーマット
で誤った判定になる。

**`.m4a` には AAC と ALAC の両方が入る.** XDJ-RX3, XDJ-XZ, XDJ-RR は前者を
受け付けて後者を拒否するが、拡張子では区別がつかない。実際に返るエラーは
`E-8304` / `E-8305`, つまり *UNSUPPORTED FILE FORMAT* である。したがって
互換性の判定は、拡張子ではなくストリーム内のコーデックに基づく必要がある。

**発売年から性能は予測できない.** XDJ-AN (2026 年) はロスレスが 48 kHz 止まり、
CDJ-2000NXS2 (2016 年) は 96 kHz に対応する。逆方向では、NXS2 は exFAT の USB を
読めないが、2020 年以降の機種はすべて読める。2 つの軸が交差するため、機種を
1 本の尺度で順位付けすることはできない。

**XDJ-XZ の exFAT 対応は本当に不明.** 最新改訂の説明書 (DRI1625B) は exFAT
非対応と明記している。一方、それより後に出た公式サポート記事 2 本は、exFAT に
対応する機種として XDJ-XZ を挙げている。ファームウェアの変更履歴に追加の記載は
ない。どちらも撤回されていないため、テーブルは片方を採用せず矛盾のまま記録する。

**記載がないことは、否定ではない.** CDJ-3000 の説明書は FAT16 / FAT32 / HFS+ を
列挙し、非対応として名指ししているのは NTFS だけで、exFAT はどこにも出てこない。
後から出た公式サポート記事は、exFAT を読める機種として CDJ-3000 を挙げている。
ここでは何も矛盾していない — 説明書が言及しなかっただけである。したがって
テーブルは exFAT を対応として記録する。説明書が明確に排除している XDJ-XZ とは
扱いが違う。記載漏れは補い、矛盾はそのまま残す。

**ファームウェアでコーデックが追加された例は 1 件しかない.** XDJ-XZ が
ファームウェア 1.10 で FLAC に対応した、その 1 件だけである。それ以外の
フォーマット表は発売時から変わっていない — 全機種のファームウェア変更履歴を
通読して確認済み。「XDJ-RR や XDJ-700 がアップデートで FLAC に対応した」という
話は、公式の変更履歴を当たると裏付けが取れない。

**アートワーク.** JPEG のみ (`.jpg`, `.jpeg`). XDJ-RR, XDJ-AN, OMNIS-DUO の
説明書には、800×800 px を超える画像は表示されないと明記されている。それ以外の
説明書には制限の記載がないため、「無制限」ではなく「不明」として扱う。

**ID3.** 対応バージョンとして v1, v1.1, v2.2.0, v2.3.0, v2.4.0 が挙げられている。

## 一次資料

操作説明書。上の表と同じ並び順:

- XDJ-AN — `XDJ-AN_DRI2023A_EN_manual.pdf`
- CDJ-3000X — `CDJ-3000X_DRI1956B_manual.pdf`
- XDJ-AZ — `XDJ-AZ_DRI1936C_manual_EN.pdf`
- OMNIS-DUO — `OMNIS_DUO_DRI1882B_manual.pdf`
- OPUS-QUAD — `OPUS-QUAD_DRI1795D_manual.pdf`
- XDJ-RX3 — `XDJ-RX3_DRI1702C_manual.pdf`
- CDJ-3000 — `CDJ-3000_DRI1586A_manual.pdf`
- XDJ-XZ — `XDJ-XZ_DRI1625B_manual.pdf`
- XDJ-RR — `XDJ-RR_DRI1568B_manual.pdf`
- CDJ-2000NXS2 — `CDJ-2000NXS2_DRI1290A_manual.pdf`

CDJ シリーズは
`https://downloads.support.alphatheta.com/manuals/dj-players/<MODEL>/`,
XDJ シリーズは `.../manuals/all-in-one-dj-systems/<MODEL>/` から配信されている。

説明書に記載がない箇所を埋めるのに使ったサポート記事:

- exFAT の対応機種一覧 — <https://support.alphatheta.com/en-US/articles/8112988343193>
- XDJ-XZ のストレージ要件 — <https://support.alphatheta.com/en-US/articles/4408364513817>
- CDJ-2000NXS2 のハイレゾ再生と DSD 非対応 — <https://support.alphatheta.com/en-US/articles/4405915074969>

発売後にフォーマット対応が追加されていないことを確認するため、全機種の
ファームウェア変更履歴を通読した。履歴は
`https://downloads.support.alphatheta.com/firmwares/` 以下にある。

## テーブルの更新方法

説明書は URL のパターンが安定しているため、スペック表を書き写すのではなく
PDF を直接取得できる:

```
https://downloads.support.alphatheta.com/manuals/all-in-one-dj-systems/XDJ-AN/XDJ-AN_DRI2023A_EN_manual.pdf
```

`support.alphatheta.com` はブラウザの User-Agent がないと 403 を返すが、PDF の
CDN は返さない。新機種では PDF より先に HTML 版が公開されることがあり、その
場合は `.../html/en/whxdata/toc.js` に全トピックの URL が入っている。

機種を追加するときは、`DEVICES` に追加するのと同じコミットで文書番号をここに
記録すること。出典を示せない行は存在してはいけない。
