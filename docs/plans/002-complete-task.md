---
id: "002"
spec: ../specs/002-complete-task.md
status: done # in-progress | done
branch: feat/002-complete-task
---

# 002: タスクを完了にする — 実装計画

## 方針

- DB の版管理（マイグレーション）を `db.rs` に作り、版2で `tasks.completed_at` を足す（B-012 の最小版）。
- `tasks.status` を文字列から `TaskStatus` 型に変え、完了・未完了の切り替えは `set_task_status` コマンド1つで行う。
- 画面は楽観的更新（先に表示を変え、保存に失敗したら戻す）にする。

## タスク

- [x] **T1: マイグレーションの仕組み**（FR-6 / AC-7, AC-8）
  - `db.rs`: `MIGRATIONS`（版ごとの SQL の一覧）と `migrate()`。`PRAGMA user_version` を見て足りない版だけを1件ずつトランザクションで当てる
  - 版1 = 仕様001のスキーマ（`CREATE TABLE IF NOT EXISTS` なので既存 DB にも安全）、版2 = `ALTER TABLE tasks ADD COLUMN completed_at TEXT`
  - 単体テスト: 新規 DB が最新版になる / 仕様001の形の DB（版0・データあり）が版2に上がりデータが残る / 2回目の起動で何もしない

- [x] **T2: TaskStatus と set_task_status**（FR-1〜FR-5 / AC-4〜AC-6）
  - `models.rs`: `TaskStatus`（open / done）、`Task.completed_at`
  - `db.rs`: `set_task_status(task_id, status)`（done なら `completed_at` に現在時刻、open なら NULL）→ そのタスクを含む `PostWithTasks` を返す
  - `error.rs`: `TaskNotFound`
  - `commands.rs`: `set_task_status` コマンドを追加
  - 単体テスト: 完了→日時あり / 未完了に戻す→NULL / 他のタスクは変わらない / 存在しない ID はエラー

- [x] **T3: 画面**（FR-1, FR-2, FR-4, FR-5 / AC-1, AC-2）
  - タスクチップの左端にチェックボックス（○ / ✓）。Space / Enter でも切り替え
  - 完了は取り消し線＋薄い色、`title` 属性で「完了: MM/DD HH:MM」
  - 楽観的更新。失敗したら元に戻し、エラーを表示

- [x] **T4: 受け入れ条件の確認**（AC-1〜AC-8）
  - 単体テストで AC-4〜AC-8 の自動部分を確認。画面の AC-1〜AC-3、AC-7 の手動部分は人が確認

## テスト方針

- DB 層・マイグレーション・コマンドの中身は単体テスト（インメモリ DB）
- 画面は `cargo tauri dev` で人が確認

## リスク・懸念

- **既存 DB の更新**: 失敗しても版数を上げないので、途中で止まっても次の起動でやり直せる。念のため、実装後に人の DB を更新する前に `x-clone.sqlite` を手でコピーしておくと安心（確認依頼で案内する）。

## 作業ログ

| 日付 | タスク | メモ |
| --- | --- | --- |
| 2026-09-25 | 計画作成 | |
| 2026-09-25 | T1 | `PRAGMA user_version` による版管理。版1=仕様001のスキーマ、版2=`completed_at`。仕様001の形の DB が版2に上がりデータが残ることをテストで確認。テスト計39件 |
| 2026-09-26 | T2 | `TaskStatus` 型（DB 保存用のマクロ `sql_text_enum!` を追加し `ClassifyStatus` も共通化）、`db::set_task_status`、コマンド登録。テスト計42件。※ T1 のコミット時に clippy の失敗を見落としていたため `6b1ce1c` で修正し、以降は検証が通ったときだけ commit するようにした |
| 2026-09-26 | T3 | チップ左端のチェック（button + role=checkbox）、完了は取り消し線＋薄い色＋完了日時のツールチップ、楽観的更新と失敗時の巻き戻し。ヘッドレス Edge ＋偽の Tauri 応答で「完了→戻す→保存失敗で巻き戻し」を確認。ユーザーの開発サーバーと衝突しないよう、trunk build は一時フォルダに出力 |
| 2026-09-26 | T4 | 人が画面で確認し承認 |
