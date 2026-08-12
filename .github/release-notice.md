> [!IMPORTANT]
> ### Which file
>
> **macOS** — the one ending in **`.dmg`**. Apple silicon only.
> **Windows** — the one ending in **`.exe`**.
>
> Both carry their own ffmpeg. Nothing else needs installing.
>
> The `transcrate-…` archives are the command line tool, which expects an
> ffmpeg you installed yourself.
>
> ---
>
> ### If it will not open the first time
>
> The app is not damaged, whatever the warning says. It carries no paid
> certificate, and this is what macOS shows for one. Do this once and it opens
> normally from then on.
>
> **macOS** — System Settings → Privacy & Security → scroll to the bottom →
> **Open Anyway** → enter your password.
> [The macOS steps](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)
>
> If it says **damaged** and offers only the Trash, that setting will not be
> there. Run this in Terminal instead, then open it normally:
>
> ```
> xattr -dr com.apple.quarantine /Applications/Transcrate.app
> ```
>
> **Windows** — On the blue warning, click **More info** → **Run anyway**.
>
> ---
>
> ### どれをダウンロードすればいいか
>
> **macOS** — 拡張子が **`.dmg`** のもの. Apple Silicon 専用です.
> **Windows** — 拡張子が **`.exe`** のもの.
>
> どちらも ffmpeg を同梱しています. 他に入れるものはありません.
>
> `transcrate-` で始まる書庫はコマンドライン版です. こちらは自分で入れた
> ffmpeg を使います.
>
> ---
>
> ### 初回起動時に開けないとき
>
> 警告に何と出ていても, アプリケーションは壊れていません. 有料の証明書を付けて
> いないだけで, macOS はそういうアプリにこの表示を出します. 一度だけ下の操作を
> すれば, 次からは普通に開きます.
>
> **macOS** — システム設定 → プライバシーとセキュリティ → 一番下までスクロール
> → **「このまま開く」** → パスワードを入力.
> [macOS の手順](https://support.apple.com/ja-jp/guide/mac-help/mh40616/mac)
>
> **「壊れているため開けません」と表示され, 「ゴミ箱に入れる」しか選べないとき**
> は上の設定項目が出てきません. ターミナルで次を実行してから, 普通に開いて
> ください.
>
> ```
> xattr -dr com.apple.quarantine /Applications/Transcrate.app
> ```
>
> **Windows** — 青い警告画面で **「詳細情報」** → **「実行」** の順にクリック.
