---
id: "001"
spec: ../specs/001-post-and-classify.md
status: in-progress # in-progress | done
branch: feat/001-post-and-classify
---

# 001: ポスト投稿と自動分類・タスク化 — 実装計画

## 方針

既存の X クローン実装（`src-tauri/src/lib.rs` に全部入り、`src/app.rs` に全画面）を削除し（Q-011）、
責務ごとにモジュールを分けて作り直す。AI 呼び出しは `LlmProvider` トレイトの裏に隠し（ADR-002）、
分類ロジックとDBはモックとインメモリ SQLite で単体テストできるようにする。

### 完成後の構成

```
src-tauri/src/
├── lib.rs            # Tauri の起動、状態の登録、コマンドの登録だけ
├── main.rs
├── config.rs         # .env の読み込み（ANTHROPIC_API_KEY, CLAUDE_MODEL）
├── error.rs          # AppError（thiserror）
├── models.rs         # Post, Task, PostWithTasks, ClassifyStatus
├── kind.rs           # TaskKind の定義を1か所に集約（enum・表示名・AI への説明）
├── db.rs             # 接続・スキーマ・読み書き関数
├── classify.rs       # 分類処理（プロバイダ呼び出し → note 除外 → 保存）
├── commands.rs       # create_post / list_timeline / retry_classification
└── llm/
    ├── mod.rs        # LlmProvider トレイト、ClassifiedTask、LlmError
    ├── claude.rs     # ClaudeProvider（reqwest で Messages API を呼ぶ）
    └── mock.rs       # MockProvider（テスト用）

src/
├── main.rs
├── app.rs            # 画面全体（入力欄 + タイムライン）
├── tauri.rs          # invoke / listen のバインディング
├── models.rs         # フロント側の型（バックエンドと同じ JSON 形）
└── components/       # Composer, Timeline, PostCard, TaskChip
```

### 主な設計判断

- **DB 接続**: `Mutex<rusqlite::Connection>` を Tauri の状態として1つ持つ。`await` をまたいでロックを持たない（分類は「API 呼び出し → ロックして保存」の順）。
- **非同期処理**: `create_post` は保存して即座に返し、分類は `tauri::async_runtime::spawn` で裏で実行する。終わったらイベント `post-classified` を送る。
- **DB の場所**: `app.path().app_data_dir()` 配下の `x-clone.sqlite`（Q-012）。テストでは `Connection::open_in_memory()`。
- **文字数**: `MAX_POST_CHARS = 280` をバックエンドとフロントでそれぞれ定数1か所に置く（Q-006）。数え方は前後の空白を除いた `chars().count()`。
- **モデル別パラメータ**: `ClaudeProvider` 内で、モデル名から「effort・フォールバックを送るか」を決める関数を1つ作る（仕様書の表どおり）。
- **フロントからのイベント受信**: `window.__TAURI__.event.listen` を wasm_bindgen でバインドする（`withGlobalTauri: true` 設定済み）。

## タスク

各タスクの終わりにビルドとテストが通る状態にし、1タスク = 1コミットにする。

- [x] **T1: 既存の X クローン機能を削除し、骨組みを作る**（Q-011）
  - `src-tauri/src/lib.rs` から login / get_posts / create_post / toggle_like / toggle_retweet / get_profile とデモデータを削除
  - 上記のモジュール構成で空のファイルを作る（`mod` 宣言だけ）
  - `src/app.rs` を「入力欄とタイムラインの枠だけ」の画面に置き換える（ログイン画面・プロフィール画面を削除）
  - `styles.css` から不要なスタイル（ログイン・プロフィール・トレンド）を削除
  - 確認: `cargo check --workspace` と `trunk build` が通る

- [x] **T2: 依存クレートの追加と DB 層**（FR-2, FR-7 / AC-8）
  - `src-tauri/Cargo.toml` に reqwest（`json`, `rustls-tls`、default-features 無効）/ tokio / async-trait / chrono（`serde`）/ thiserror / dotenvy を追加（Q-008, Q-010）
  - `error.rs`, `models.rs`, `db.rs`: スキーマ作成、`PRAGMA foreign_keys = ON`、`insert_post` / `list_timeline(limit, before_id)` / `replace_tasks(post_id, tasks)` / `set_classify_status`
  - 単体テスト（インメモリ DB）: 保存・新しい順の取得・ページング・タスクの置き換え・ポスト削除でタスクも消えること

- [ ] **T3: 種別の定義、LlmProvider、分類処理**（FR-3, FR-4, FR-8 / AC-5, AC-9）
  - `kind.rs`: `TaskKind`（issue / idea / research / note）と、表示名・AI への説明・タスクにするかどうかを1か所に定義（Q-005）
  - `llm/mod.rs`: `LlmProvider` トレイト、`ClassifiedTask`、`LlmError`
  - `llm/mock.rs`: 決まった結果やエラーを返す `MockProvider`
  - `classify.rs`: プロバイダを呼び、`note` を除外し、成功なら `replace_tasks` + `done`、失敗なら `failed` + エラー文を保存
  - 単体テスト（Mock + インメモリ DB）: 複数タスク / note のみ / 失敗時 / 再分類で古いタスクが置き換わる

- [ ] **T4: ClaudeProvider と設定読み込み**（FR-8 / AC-6 の土台）
  - `config.rs`: `dotenvy` で `.env` を読み、`ANTHROPIC_API_KEY`（無ければ「未設定」）と `CLAUDE_MODEL`（既定 `claude-haiku-4-5`）を得る
  - `llm/claude.rs`:
    - リクエスト JSON の組み立て（構造化出力のスキーマは `kind.rs` から生成。モデル別に effort・fallbacks を付ける/付けない）
    - レスポンスの解釈（`stop_reason` の確認 → text ブロックを JSON として読む）
    - タイムアウト30秒、429・5xx は最大2回リトライ
    - API キー未設定なら、通信せずに「API キーが設定されていません」エラーを返す
  - 単体テスト（通信なし）: モデル別のリクエスト JSON、正常レスポンス、`refusal` / `max_tokens`、壊れた JSON、キー未設定

- [ ] **T5: Tauri コマンドとイベント**（FR-1〜FR-3, FR-6 / AC-6, AC-7）
  - `commands.rs`: `create_post`（280文字チェック → 保存 → 裏で分類開始）/ `list_timeline` / `retry_classification`
  - 分類の完了時（成功・失敗とも）に `post-classified` イベントで `PostWithTasks` を送る
  - `lib.rs`: DB と `Arc<dyn LlmProvider>` を状態として登録、コマンドを登録
  - 単体テスト: 文字数チェック（0文字・空白のみ・280文字・281文字）
  - `docs/project/architecture.md` のコマンド一覧とデータモデルを更新

- [ ] **T6: 画面の実装**（FR-1, FR-5, FR-6 / AC-10）
  - `tauri.rs`: `invoke` と `listen` のバインディング
  - Composer: 残り文字数の表示（マイナスは赤）、0文字・280文字超えでボタン無効、Ctrl+Enter で投稿
  - Timeline / PostCard: 新しい順、分類状態（分類中… / 失敗 + 再試行ボタン）、`post-classified` を受けてその投稿だけ更新
  - TaskChip: 種別ごとの色（課題=赤系 / 思い付き=黄系 / 調査=青系）、note は表示しない
  - 確認: `trunk build` が通り、`cargo tauri dev` で画面が出る

- [ ] **T7: 受け入れ条件の確認**（AC-1〜AC-11）
  - `.env` を用意して `cargo tauri dev` で AC-1〜AC-8、AC-10 を手動確認し、結果を作業ログに残す
  - AC-11: `git status` と `git ls-files` で `.env` が入っておらず `.env.example` があることを確認
  - AC-1〜AC-5 で Haiku の分類精度が足りなければ、`CLAUDE_MODEL=claude-opus-5` でも試して比較を記録する（切り替えるかは人が判断）

## テスト方針

- **単体テスト（`cargo test --workspace`）**: DB 層、分類処理、Claude のリクエスト/レスポンス処理、文字数チェック。すべて通信なし（Mock とインメモリ DB）で動くようにする。
- **手動確認**: 画面と、実際の Claude API を使う AC-1〜AC-8。API を呼ぶのは T7 だけにする（料金がかかるため）。

## リスク・懸念

- **分類結果が毎回同じになるとは限らない**: AC-1〜AC-5 は LLM の出力に依存する。プロンプトに種別ごとの例を入れて安定させる。Haiku で精度が足りなければ Opus 5 への切り替えを提案する（Q-007）。
- **`.env` の場所**: `cargo tauri dev` は `src-tauri/` で動くが、`dotenvy::dotenv()` は親フォルダまでさかのぼって探すので、プロジェクト直下の `.env` が読まれる見込み。T4 で確認する。リリース版（exe 単体）では `.env` が無いので、配布する前にキーチェーン方式（後続の仕様）が必要。
- **Leptos でのイベント受信**: Tauri のイベントを WASM 側で受ける部分は自前のバインディングになる。うまくいかない場合は、分類状態を定期的に取りに行く方式で代替する（その場合は人に確認する）。
- **古い DB ファイル**: `src-tauri/.x_clone.sqlite` は使われなくなる（git には入っていない）。不要なら手で削除してよい。
- **DB スキーマの変更**: 新規作成なのでマイグレーションは不要。今後スキーマを変えるときの仕組みは後続で検討する。

## 作業ログ

| 日付 | タスク | メモ |
| --- | --- | --- |
| 2026-09-25 | 計画作成 | 既存コードとの食い違いを Q-011 / Q-012 で解消 |
| 2026-09-25 | T1 | 既存機能を削除し、モジュールの空ファイルと最小画面を作成。clippy・trunk build・test 通過 |
| 2026-09-25 | T2 | reqwest 0.13 では機能名が `rustls-tls` から `rustls` に変わったのでそれを使用。DB 型で必要なため `kind.rs` と文字数チェックも前倒しで実装。テスト13件通過。T5 までは `dead_code` を一時的に許可 |
