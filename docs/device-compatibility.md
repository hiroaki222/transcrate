# Device compatibility data

The source behind `crates/transcrate-core/src/device.rs`. Every value in that
table traces back to a document listed here. Nothing goes into the table on the
strength of a forum post or a blog.

## Scope

- **USB only.** CD and SD media carry extra restrictions that Transcrate does
  not model. Most notably a CDJ-2000NXS2 refuses FLAC, ALAC and 88.2/96 kHz PCM
  when they are burnt to a disc, while accepting all of them from USB.
- **Sampling rates at or below 24 kHz are omitted.** Several manuals list an
  MPEG-2 Layer-3 row (16/22.05/24 kHz) and low-rate AAC. They are playable, but
  nothing sane converts *into* them, so the table describes the range worth
  targeting rather than everything a player will accept.
- **Ten players.** The models in current club and bar circulation. Discontinued
  MP3-only decks (CDJ-200, CDJ-400, CDJ-800MK2) are out of scope.

## Format support

Highest documented sampling frequency per codec. `-` means the codec does not
appear in the player's format table at all.

| Player | Year | MP3 | AAC | WAV | AIFF | FLAC | ALAC | exFAT |
|---|---|---|---|---|---|---|---|---|
| XDJ-AN | 2026 | 48k | 48k | 48k | 48k | 48k | 48k | yes |
| CDJ-3000X | 2025 | 48k | 48k | 96k | 96k | 96k | 96k | yes |
| XDJ-AZ | 2025 | 48k | 48k | 96k | 96k | 96k | 96k | yes |
| OMNIS-DUO | 2024 | 48k | 48k | 48k | 48k | 48k | 48k | yes |
| OPUS-QUAD | 2023 | 48k | 48k | 96k | 96k | 96k | 96k | yes |
| XDJ-RX3 | 2021 | 48k | 48k | 48k | 48k | 48k | - | yes |
| CDJ-3000 | 2020 | 48k | 48k | 96k | 96k | 96k | 96k | yes |
| XDJ-XZ | 2019 | 48k | 48k | 48k | 48k | 48k | - | disputed |
| XDJ-RR | 2018 | 48k | 48k | 48k | 48k | - | - | no |
| CDJ-2000NXS2 | 2016 | 48k | 48k | 96k | 96k | 96k | 96k | no |

Common to every player above: MP3 and AAC are 16-bit only and capped at 48 kHz;
lossless formats accept 16- and 24-bit; folder nesting is limited to 8 levels;
the browser lists at most 10,000 entries per folder; NTFS is not supported; USB
hubs are not supported.

MP3 accepts 32–320 kbps and AAC 16–320 kbps everywhere. Players up to the 2021
generation also list 32 kHz for both; the 2023-and-later manuals dropped that
row, so newer hardware is *narrower* here.

## Notes that drive the code

**Sample-rate limits belong to the format, not the player.** A CDJ-3000 plays
96 kHz FLAC and 48 kHz MP3. Any single "maximum sample rate" attribute on a
device would be wrong for half its formats.

**`.m4a` carries either AAC or ALAC.** The XDJ-RX3, XDJ-XZ and XDJ-RR accept the
former and reject the latter, and the extension does not distinguish them. The
documented failure is error `E-8304`/`E-8305`, *UNSUPPORTED FILE FORMAT*.
Compatibility has to be decided from the codec in the stream.

**Release year does not predict capability.** The XDJ-AN (2026) tops out at
48 kHz for lossless; the CDJ-2000NXS2 (2016) reaches 96 kHz. In the other
direction the NXS2 cannot read an exFAT stick, which every player from 2020
onward can. The two axes cross, so devices cannot be ranked on one scale.

**The XDJ-XZ's exFAT support is genuinely unclear.** Its manual (DRI1625B, the
newest revision) states exFAT is not supported. Two later support articles list
the XDJ-XZ among the players that do support it. No firmware changelog entry
mentions adding it. Neither claim has been withdrawn, so the table records the
contradiction instead of picking a side.

**No player has ever gained a codec through firmware, with one exception.** The
XDJ-XZ gained FLAC in firmware 1.10. Every other format table in this document
has been fixed since launch — verified against the full firmware change history
of each model. Claims that the XDJ-RR or XDJ-700 gained FLAC support in an
update do not survive contact with the official changelogs.

**Artwork.** JPEG only (`.jpg`, `.jpeg`). The XDJ-RR, XDJ-AN and OMNIS-DUO
manuals state that images larger than 800×800 px are not displayed. The other
manuals state no limit, which is recorded as unknown rather than unlimited.

**ID3.** Players list v1, v1.1, v2.2.0, v2.3.0 and v2.4.0 as supported.

## Primary sources

Operating instructions, in the order the table lists the players:

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

All are served from
`https://downloads.support.alphatheta.com/manuals/dj-players/<MODEL>/` for the
CDJ line and `.../manuals/all-in-one-dj-systems/<MODEL>/` for the XDJ line.

Support articles used where a manual is silent:

- exFAT support across the range — <https://support.alphatheta.com/en-US/articles/8112988343193>
- XDJ-XZ storage requirements — <https://support.alphatheta.com/en-US/articles/4408364513817>
- CDJ-2000NXS2 hi-res playback, DSD not supported — <https://support.alphatheta.com/en-US/articles/4405915074969>

Firmware change histories were read in full for every model to confirm that no
format support was added after launch. They live under
`https://downloads.support.alphatheta.com/firmwares/`.

## Updating this table

Manuals are addressable by a stable URL pattern, so a new model's data can be
pulled directly rather than transcribed from a spec sheet:

```
https://downloads.support.alphatheta.com/manuals/all-in-one-dj-systems/XDJ-AN/XDJ-AN_DRI2023A_EN_manual.pdf
```

`support.alphatheta.com` answers 403 without a browser user agent; the PDF CDN
does not. Newly released models sometimes publish HTML before PDF, in which case
`.../html/en/whxdata/toc.js` lists every topic URL.

When adding a player, record its document number here in the same commit that
adds it to `DEVICES`. A row whose source cannot be named should not exist.
