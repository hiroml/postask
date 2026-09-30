# postask

**ポストするだけで、AI がタスクに整理してくれる個人用のタスク管理アプリ**

X（旧 Twitter）のように、思ったことを気軽にポストします。すると AI（Claude）が内容を読み、
「課題」「思い付き」「調査」「メモ」に分けてタスクにします。タスクは完了にでき、グラフで振り返れます。
AI に助言を頼んだり、関連するタスクを派生させたりもできます。

Windows 向けのデスクトップアプリです（Tauri 2 + Leptos）。データはすべて自分のパソコンの SQLite に保存されます。

## できること

| 機能 | 内容 |
| --- | --- |
| ポストと自動分類 | 280文字までポストすると、AI が種別・タイトルを付けてタスクにする。1つのポストから複数のタスクもできる |
| タスクの完了 | チップの ○ で完了・未完了を切り替え。完了日時を記録する |
| AI リプライ | 「🤖 AI に聞く」で、ポストへの助言をもらう。「調査」のポストでは Web 検索を使い、出典を付ける |
| 課題ページ | 未完了・完了の数、種別ごとの棒グラフ、作成・完了の推移、一覧（絞り込み・その場で完了） |
| 話題 | 取り組んでいるテーマを「話題」として作ると、AI がポストを話題ごとに振り分ける |
| 関連タスクの派生 | タスクから「関連するやること」をポストする。AI リプライから候補を出すこともできる。元のタスクに「↳ 1/3」のように進み具合が出る |

## 使っている技術

| レイヤ | 技術 |
| --- | --- |
| 画面 | [Leptos](https://leptos.dev/) 0.8（CSR）を WebAssembly にビルド、[Trunk](https://trunkrs.dev/) で配信 |
| デスクトップアプリ | [Tauri](https://tauri.app/) 2 |
| データベース | SQLite（[rusqlite](https://github.com/rusqlite/rusqlite)、アプリに同梱） |
| AI | [Claude API](https://docs.anthropic.com/)（reqwest で直接呼ぶ。分類は構造化出力、調査は Web 検索） |
| グラフ | 自前の SVG / HTML（ライブラリなし） |

言語はフロントエンド・バックエンドとも Rust です。

## 必要なもの

| もの | 入れ方・備考 |
| --- | --- |
| Rust（stable） | [rustup](https://rustup.rs/) で入れる。1.98 で動作確認 |
| WebAssembly のターゲット | `rustup target add wasm32-unknown-unknown` |
| Trunk | `cargo install trunk --locked`（0.21 で動作確認） |
| Tauri CLI | `cargo install tauri-cli --version "^2" --locked`（2.11 で動作確認） |
| Tauri の前提ソフト | Windows では Microsoft C++ Build Tools と WebView2（Windows 11 は標準で入っている）。[Tauri の前提条件](https://v2.tauri.app/start/prerequisites/)を参照 |
| Claude API キー | [Anthropic Console](https://console.anthropic.com/) で発行する。使った分だけ料金がかかる |

macOS・Linux でも Tauri は動きますが、このアプリは Windows でしか確かめていません。

## はじめかた

```sh
git clone git@github.com:hiroml/postask.git
cd postask

# API キーを設定する（.env は git に含めない）
cp .env.example .env
# .env を開き、ANTHROPIC_API_KEY= のあとにキーを書く

# 開発版を起動する（初回はビルドに数分かかる）
cargo tauri dev
```

### 設定（`.env`）

| 変数 | 既定値 | 用途 |
| --- | --- | --- |
| `ANTHROPIC_API_KEY` | なし（必須） | Claude API のキー |
| `CLAUDE_MODEL` | `claude-haiku-4-5` | 分類・話題の判定に使うモデル |
| `CLAUDE_REPLY_MODEL` | `claude-haiku-4-5` | AI リプライと関連タスクの候補に使うモデル |
| `X_CLONE_DB` | なし | 使う DB ファイルのパスを指定する（テスト・確認用） |

キーが無くてもアプリは起動します。その場合、分類が失敗と表示されます。キーを入れてから「再試行」を押すと分類されます。

## 使いかた

1. **ポストする**: ホームの入力欄に書いて「ポスト」（Ctrl+Enter でも可）。少し待つと、下にタスクのチップが付く
2. **完了にする**: チップの左の ○ を押す。もう一度押すと未完了に戻る
3. **AI に聞く**: ポストの「🤖 AI に聞く」を押すと、助言のリプライが付く。気に入らなければ「聞き直す」
4. **関連タスクを作る**: チップの「↳」で、そのタスクに関連するやることをポストする。AI リプライの下の「↳ ここから関連タスクを作る」を押すと、候補から選んでまとめて投稿できる
5. **話題を作る**: 入力欄の右の選択を「# 話題として投稿」にしてテーマを書く。AI が名前を提案する。以後、関係するポストに話題のチップが付く（× で外せる）
6. **振り返る**: メニューの「✅ 課題」で数字・グラフ・一覧を、「# 話題」で話題ごとのポスト数・未完了のタスク数を見る

## データの保存場所

| ビルド | ファイル |
| --- | --- |
| リリース版（`cargo tauri build`） | `%APPDATA%\com.hikou.x-clone\x-clone.sqlite` |
| 開発版（`cargo tauri dev`） | `%APPDATA%\com.hikou.x-clone\x-clone-dev.sqlite` |

- 開発版は、初回起動時にリリース版の DB を読み取り専用でコピーして使います。開発中に普段のデータを壊しません。画面には「開発版」と出ます。
- 旧名（x-clone）のフォルダ名・ファイル名は、データを引き継ぐためにそのままにしています。
- DB の形が変わるときは、起動時に自動で更新されます（データは消えません）。

## コマンド

```sh
cargo tauri dev          # 開発版を起動（trunk serve --port 1421 が自動で走る）
cargo tauri build        # リリース版をビルド（インストーラーは target/release/bundle/ に出る）
trunk build              # 画面だけビルド
cargo check --workspace  # 型チェック
cargo test --workspace   # テスト
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

実際の Claude API を呼ぶ確認用のテストは、ふだんは実行されません（料金がかかるため）。

```sh
cargo test -p postask live_ -- --ignored --nocapture
```

## フォルダの構成

| 場所 | 中身 |
| --- | --- |
| `src/` | 画面（Leptos）。`pages/` がページ、`components/` が部品、`store.rs` がタイムラインの状態 |
| `src-tauri/src/` | Tauri のコマンド、DB（`db.rs`）、分類・リプライ・話題・派生の処理、`llm/`（Claude API） |
| `styles.css`, `index.html` | 画面の見た目と入口 |
| `docs/` | 仕様書・実装計画・確認依頼・設計判断・バックログ（[docs/README.md](docs/README.md)） |
| `.claude/` | AI（Claude Code）用のワークフローの手順 |

## 開発の進め方

このリポジトリは、AI（Claude Code）と人が一緒に開発しています。

```
要望 → 仕様書（AI） → 人の承認 → 実装計画（AI） → 実装・テスト（AI） → 確認依頼（AI） → 人の確認 → マージ
```

- 仕様書は [docs/specs/](docs/specs/)、判断が必要なことは [docs/questions.md](docs/questions.md)、残課題は [docs/backlog.md](docs/backlog.md) にあります。
- AI 向けのルールは [CLAUDE.md](CLAUDE.md) にあります。
- 全体の設計は [docs/project/architecture.md](docs/project/architecture.md) を見てください。
