# konnyaku

どのアプリでも、外国語を選択して `Control+J` を押すと、日本語訳がウィンドウに表示される macOS 用の翻訳ツールです。見出し、リスト、表、コードなどの構造と画像は、原文のまま残ります。

翻訳には、あなたが自分でインストールしてログインした [Claude Code](https://code.claude.com/) を使います。料金は、あなたの Claude のサブスクリプションの利用枠から消費されます。konnyaku 自体は、ログイン情報を保存したり中継したりしません。

## できること

- 選択した部分を、構造を保ったまま翻訳する。Safari と TextEdit で動作を確かめています。選択範囲をコピーできるアプリなら使えるはずです。
- 画像は翻訳せず、元の位置にそのまま表示する。読み込めない画像は、説明文と「ブラウザで開く」リンクの枠で表示します。
- コードブロックとインラインコードは翻訳しない。
- 訳文を、生成された順に少しずつ表示する。
- 原文がすでに日本語のときは、Claude を呼ばない。

## 必要なもの

- macOS（Apple Silicon で開発・検証しています）
- Claude Code。インストールしたうえで、`claude` を起動して `/login` でログインしておいてください。
- Node.js と pnpm
- Rust（stable）
- Xcode Command Line Tools

## ビルドとインストール

1. リポジトリを取得して、依存パッケージを入れます。

   ```sh
   git clone https://github.com/crypt0box/konnyaku.git
   cd konnyaku
   pnpm install
   ```

2. 開発用の署名証明書を作ります。最初の1回だけ必要です。

   ```sh
   sh scripts/setup-dev-signing.sh
   ```

   ログインキーチェーンに「konnyaku Local Dev」という自己署名の証明書を1つ追加します。この証明書で署名しておくと、ビルドし直してもアクセシビリティの許可が外れません。

3. アプリをビルドして署名します。

   ```sh
   sh scripts/build-dev-app.sh
   ```

   最後の行に、できたアプリのパス（`src-tauri/target/debug/bundle/macos/konnyaku.app`）が表示されます。署名の途中で「codesign がキーを使おうとしています」と確認されたら、「常に許可」を選んでください。

4. アクセシビリティを許可します。

   ```sh
   open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
   ```

   一覧の「＋」から `konnyaku.app` を追加して、スイッチをオンにします。ファイル選択画面で `⌘⇧G` を押すと、パスを直接入力できます。選択範囲を取り込むときにコピー操作を送るため、この許可が必要です。

5. 起動します。

   ```sh
   open src-tauri/target/debug/bundle/macos/konnyaku.app
   ```

## 使い方

| 操作 | 動き |
|---|---|
| テキストを選択して `Control+J` | 選択範囲を取り込み、翻訳ウィンドウに訳文を表示する |
| `Esc`、または閉じるボタン | ウィンドウを閉じる。翻訳中なら止める |
| Dock のアイコンをクリック | 直前の訳文をもう一度表示する |
| `⌘,` | 設定を開く |
| `⌘Q` | 終了する |

翻訳中にもう一度 `Control+J` を押すと、前の翻訳を止めて新しい選択範囲を翻訳します。選択範囲を取り込めなかったときは、そのときクリップボードにある内容を翻訳します。

### 設定

| 項目 | 初期値 | 選べる値 |
|---|---|---|
| ショートカット | `Control+J` | Control、Option、Command のいずれかを含む組み合わせ |
| モデル | sonnet | haiku、sonnet、opus |
| effort | low | low、medium、high、xhigh、max |
| 文体 | 原文に合わせる | 原文に合わせる、です・ます調、だ・である調 |
| Claude Code の場所 | 自動で探す | `claude` の実行ファイルのパス |
| ログイン時に起動 | オン | オン、オフ |

`claude` が見つからないときは、ターミナルで `which claude` を実行し、表示されたパスを「Claude Code の場所」に入力してください。

`scripts/build-dev-app.sh` で作る開発版は、「ログイン時に起動」の設定を変えても macOS のログイン項目に登録しません。

## プライバシー

- 選択した文章は、あなたの Claude Code を通して Anthropic に送られます。画像とコードは送りません。
- 翻訳の履歴は保存しません。直前の訳文はメモリ上にだけ置き、アプリを終了すると消えます。
- Claude Code の会話記録にも残しません（`--no-session-persistence`）。
- 翻訳ウィンドウはプライベートモードで動くので、表示した画像のキャッシュや閲覧記録も残りません。
- パスワード管理アプリが「機密」として扱うクリップボードの内容は、取り込みません。
- 選択範囲を取り込んだ後、クリップボードは元の内容に戻します。

## 開発

```sh
pnpm test                         # TypeScript のテスト
(cd src-tauri && cargo test)      # Rust のテスト
sh scripts/build-dev-app.sh && scripts/e2e/gui-verify.sh <出力先>   # 実機での通し検証
```

`scripts/e2e/gui-verify.sh` は、開発版の konnyaku 自身にキー入力とクリックを送らせて、選択範囲の取り込みや設定の変更などを実際の画面で確かめます。実行には約2分半かかり、その間は画面が自動で操作されます。クリップボードと設定は、検証の前後で元に戻します。

設計の背景は次のファイルにまとめています。

- [`CONTEXT.md`](CONTEXT.md)：用語の定義
- [`docs/adr/`](docs/adr/)：大きな設計判断とその理由
- [`docs/backlog.md`](docs/backlog.md)：後回しにした改善点

## 今の制限

- 動くのは macOS だけです。Windows 対応は予定していますが、まだできていません。
- 正式な署名と公証をした配布版は、まだありません。ソースからビルドして使ってください。
- モデルを haiku にすると、まれに翻訳を断られることがあります。

## ライセンス

[MIT](LICENSE)
