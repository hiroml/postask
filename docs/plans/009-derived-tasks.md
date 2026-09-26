---
id: "009"
spec: ../specs/009-derived-tasks.md
status: in-progress # in-progress | done
branch: feat/009-derived-tasks
---

# 009: 関連タスクの派生 — 実装計画

## 方針

- `feat/008-topics` から枝分かれして作る（マイグレーションは版5）。
- 親子は `posts.parent_task_id` で持つ。子の数・完了数は取得時に数える（保存しない）。
- AI の候補は、ボタンを押したときだけ1回呼ぶ。投稿は既存の `create_post` を候補ごとに呼ぶ。

## タスク

- [ ] **T1: DB（版5）とデータの型**（FR-2, FR-5〜FR-8 / AC-6）
  - マイグレーション版5: `posts.parent_task_id`
  - `models.rs`: `PostWithTasks.parent`（元のタスクの id とタイトル）、`Task.children`（total・done）、課題ページの `TaskRow` にも
  - `db.rs`: 親つきのポストの保存、子の数え方、元のタスクが消えたとき
  - 単体テスト
- [ ] **T2: AI の候補**（FR-1b / AC-8）
  - `LlmProvider::suggest_derived_tasks`、Mock、`derive.rs`（新規: 候補の親をそのポストのタスクに限る、280文字に切る）
  - Claude の実装（構造化出力、リプライと同じモデル、検索なし）、実 API の確認用テスト
- [ ] **T3: コマンド**
  - `create_post(body, parent_task_id)`、`suggest_derived_tasks(post_id)`
- [ ] **T4: 画面**（FR-1, FR-1b, FR-3〜FR-8）
  - タスクチップの「↳」と、ポストの下の入力欄（Esc で閉じる）
  - 派生ポストの「↳ 〇〇 から」、チップの「↳ 1/3」「✓ 関連タスク完了」、課題ページの一覧の親子表示
  - AI リプライの下の「↳ ここから関連タスクを作る」と候補の選択・編集・投稿
- [ ] **T5: 確認**
  - ヘッドレス Edge ＋偽の応答、実 API の確認用テスト。画面は人が確認

## 作業ログ

| 日付 | タスク | メモ |
| --- | --- | --- |
| 2026-09-26 | 計画作成 | |
