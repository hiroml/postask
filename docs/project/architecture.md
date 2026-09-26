# アーキテクチャ

## 全体構成

```mermaid
flowchart LR
  UI["Leptos (WASM)<br/>src/"] -- "invoke()" --> CMD["Tauri コマンド<br/>src-tauri/src/"]
  CMD --> DB[("SQLite<br/>rusqlite")]
```

| レイヤ | 場所 | 責務 |
| --- | --- | --- |
| UI | `src/app.rs`（ルーター）, `src/pages/`, `src/components/`, `src/store.rs` | 画面描画・入力。Tauri コマンドを `invoke` で呼ぶ。タイムラインの状態は `store.rs` でアプリ全体に共有（ページを移動しても残る） |
| コマンド | `src-tauri/src/lib.rs` | `#[tauri::command]` でフロントに API を公開 |
| 永続化 | `src-tauri/src/db.rs` | rusqlite で SQLite を読み書き。ファイルは `%APPDATA%\com.hikou.x-clone\x-clone.sqlite`（Q-012） |
| AI | `src-tauri/src/llm/`, `classify.rs`, `reply.rs`, `topics.rs`, `derive.rs` | `LlmProvider` トレイトの裏で Claude API を呼ぶ（ADR-002）。分類・AI リプライ・話題・関連タスクの候補 |
| backend | `backend/` | 未使用（将来のサーバー用途を想定） |

## ビルドの流れ

- 開発: `cargo tauri dev` → `trunk serve --port 1421` でフロントを配信し、Tauri がそれを読み込む
- 本番: `trunk build` で `dist/` を生成 → Tauri がバンドル

## データモデル

定義の正本は `src-tauri/src/db.rs` の `MIGRATIONS`（DB の版ごとの更新の一覧）。

| テーブル | 主な列 | 説明 |
| --- | --- | --- |
| `posts` | `id`, `body`, `created_at`, `classify_status`（pending / done / failed）, `classify_error`, `kind`（post / topic）, `parent_task_id`（→ tasks, SET NULL） | ユーザーのポスト。`kind = topic` は話題として投稿したもの（タスクに分類しない）。`parent_task_id` があれば派生ポスト（仕様009）で、そのタスクが元のタスクの子になる |
| `tasks` | `id`, `post_id`（→ posts, CASCADE）, `kind`, `title`, `target`, `action`, `status`（open / done）, `created_at`, `completed_at` | ポストから AI が起こしたタスク |
| `replies` | `id`, `post_id`（→ posts, CASCADE）, `author`（`ai`）, `body`, `sources`（出典の JSON 配列）, `model`, `status`（pending / done / failed）, `error`, `created_at` | ポストへのリプライ。AI のリプライはポストごとに1件 |
| `topics` | `id`, `name`（一意、20文字まで）, `description`, `source_post_id`（話題として投稿したポスト）, `created_at` | 話題（仕様008） |
| `post_topics` | `post_id`, `topic_id`, `assigned_by`（`ai` / `user`）, `removed`（人が外した）, `created_at` | ポストと話題の対応。1ポストに最大3つ（AI）。`removed = 1` の話題は AI が付け直さない |

- **DB の版管理（マイグレーション）**: `PRAGMA user_version` に版数を記録し、起動時に足りない版だけを当てる。テーブルや列を増やすときは `MIGRATIONS` の末尾に版を足す（既存の版は変えない）。

  | 版 | 内容 | 仕様 |
  | --- | --- | --- |
  | 1 | posts / tasks | 001 |
  | 2 | `tasks.completed_at` | 002 |
  | 3 | `replies` | 003 |
  | 4 | `topics`、`post_topics`、`posts.kind` | 008 |
  | 5 | `posts.parent_task_id` | 009 |

- 種別（`kind`）の定義（表示名・色・分類の基準・リプライでの答え方）は `src-tauri/src/kind.rs` の `KINDS` にまとめてある。増やすときはここだけ直す。
- 起動時に `pending` のまま残っているポスト・リプライ（前回の終了で中断されたもの）は `failed` にする。

## AI の設定（`.env`）

| 変数 | 既定値 | 用途 |
| --- | --- | --- |
| `ANTHROPIC_API_KEY` | なし | Claude API のキー |
| `CLAUDE_MODEL` | `claude-haiku-4-5` | 分類に使うモデル |
| `CLAUDE_REPLY_MODEL` | `claude-haiku-4-5` | リプライに使うモデル。調査のポストでは Web 検索（最大3回）を使う |
| `X_CLONE_DB` | なし | 使う DB ファイルのパスを指定する（テスト・確認用、仕様004） |

## Tauri コマンド一覧

引数名は snake_case（例: `before_id`）。

| コマンド名 | 引数 | 戻り値 | 仕様書 |
| --- | --- | --- | --- |
| `create_post` | `body: String`, `parent_task_id: Option<i64>` | `Post`（保存して即返す。分類は裏で開始）。`parent_task_id` を渡すと派生ポスト（無いタスクはエラー） | [001](../specs/001-post-and-classify.md), [009](../specs/009-derived-tasks.md) |
| `list_timeline` | `limit: u32`, `before_id: Option<i64>` | `Vec<PostWithTasks>`（新しい順。タスクとリプライを含む） | [001](../specs/001-post-and-classify.md) |
| `retry_classification` | `post_id: i64` | `()` | [001](../specs/001-post-and-classify.md) |
| `list_kinds` | なし | `Vec<KindInfo>`（種別・表示名・色） | [001](../specs/001-post-and-classify.md)（実装時に追加） |
| `set_task_status` | `task_id: i64`, `status: "open" \| "done"` | `PostWithTasks` | [002](../specs/002-complete-task.md) |
| `request_ai_reply` | `post_id: i64` | `()`（受け付けてすぐ返す。生成は裏で行う） | [003](../specs/003-ai-reply.md) |
| `get_task_dashboard` | `period`（`all` / `7d` / `30d` / `90d`）, `kind: Option<種別>`, `status: Option<open / done>` | `TaskDashboard`（数字・種別ごとの数・推移・一覧。日付はパソコンのタイムゾーンで数える） | [006](../specs/006-tasks-page.md) |
| `app_info` | なし | `{ dev: bool, db_path: String }`（開発版かどうか） | [004](../specs/004-dev-db.md) |
| `suggest_topic_name` | `body: String` | `String`（AI が提案した話題の名前） | [008](../specs/008-topics.md) |
| `create_topic` | `name`, `description`, `body: Option<String>` | `{ topic, post: Option<PostWithTasks> }`。`body` があれば話題として投稿（ポストも保存）。直近100件の振り分け直しを裏で行い `post-updated` で知らせる | [008](../specs/008-topics.md) |
| `list_topics` / `update_topic` / `delete_topic` | なし / `topic_id, name, description` / `topic_id` | 話題の一覧（ポスト数・未完了のタスク数つき）・編集・削除（ポストとタスクは残る） | [008](../specs/008-topics.md) |
| `remove_post_topic` | `post_id`, `topic_id` | `PostWithTasks`（外した話題は AI が付け直さない） | [008](../specs/008-topics.md) |
| `suggest_derived_tasks` | `post_id` | `Vec<{ text, task_id }>`（AI リプライをもとにした関連タスクの候補、最大5件。投稿は `create_post` を候補ごとに呼ぶ） | [009](../specs/009-derived-tasks.md) |

プラグインのコマンド: `plugin:opener|open_url`（出典の URL を既定のブラウザで開く）

## イベント

| イベント名 | payload | いつ |
| --- | --- | --- |
| `post-updated` | `PostWithTasks` | 分類・AI リプライが終わったとき（成功・失敗とも）。仕様003 で `post-classified` から改名。派生ポストの分類・タスクの完了切り替えでは、元のタスクのポストも送る（子の数が変わるため、仕様009） |

## DB ファイル（仕様004）

| ビルド | ファイル | 備考 |
| --- | --- | --- |
| 開発用（`cargo tauri dev`） | `%APPDATA%\com.hikou.x-clone\x-clone-dev.sqlite` | 無ければ普段使いの DB を読み取り専用でコピーして作る。画面・タイトルに「開発版」 |
| リリース（`cargo tauri build`） | `%APPDATA%\com.hikou.x-clone\x-clone.sqlite` | 普段使い |

## ページ（仕様005）

| パス | ページ | 中身 |
| --- | --- | --- |
| `/` | ホーム | 入力欄とタイムライン（`pages/home.rs`） |
| `/tasks` | 課題 | 集計・グラフ・一覧（`pages/tasks.rs`、仕様006） |
| `/topics` | 話題 | 話題の一覧・作成・編集・削除（`pages/topics.rs`、仕様008） |
| `/notifications`, `/profile`, `/settings` | 通知・プロフィール・設定 | 準備中（`pages/coming_soon.rs`） |

- メニューの項目は `src/components/nav.rs` の `NAV_ITEMS`。中身ができたら `ready: true` にして `app.rs` のルートを差し替える
- グラフは自前の SVG / HTML（`src/components/charts/`）。色は dataviz スキルの検証を通した2色（未完了・作成 `#2a78d6` / 完了 `#eb6834`）
