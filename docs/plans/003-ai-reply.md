---
id: "003"
spec: ../specs/003-ai-reply.md
status: in-progress # in-progress | done
branch: feat/003-ai-reply
---

# 003: AI リプライ — 実装計画

## 方針

- 仕様002のマイグレーションに版3（`replies` テーブル）を足すため、ブランチは `feat/002-complete-task` から作る。マージも 002 → 003 の順。
- 分類（classify）と同じ形で「リプライ処理」を作る: 依頼で pending を保存 → 裏で AI を呼ぶ → done / failed を保存 → イベントで知らせる。
- イベントは `post-classified` を `post-updated` に改名し、分類とリプライの両方で使う。

## タスク

- [x] **T1: replies テーブルと DB 層**（FR-1, FR-7 / AC-4, AC-6）
  - マイグレーション版3: `replies`（`sources` は JSON 文字列）
  - `models.rs`: `Reply`、`ReplySource`、`ReplyStatus`、`PostWithTasks.replies`
  - `db.rs`: `start_ai_reply(post_id, model)`（既存の AI リプライを消して pending を入れる）、`finish_ai_reply`（done / failed）、タイムライン取得でリプライも返す。起動時に pending のリプライを failed にする
  - 単体テスト

- [x] **T2: LlmProvider::reply とリプライ処理**（FR-2, FR-5 / AC-5, AC-8）
  - `llm/mod.rs`: `reply(post_body, tasks, use_web_search) -> AiReply`、`AiReply { body, sources }`
  - `llm/mock.rs`: リプライにも対応
  - `reply.rs`: 依頼の検証（分類済みでタスクがあること）→ AI 呼び出し → 保存。調査タスクがあれば検索を使う
  - 単体テスト（Mock）: 成功 / 失敗 / 聞き直しで置き換わる / タスクの無いポストは断る / 調査のときだけ検索を指示

- [x] **T3: ClaudeProvider のリプライ**（FR-2, FR-8〜FR-10）
  - `config.rs`: `CLAUDE_REPLY_MODEL`（既定 `claude-haiku-4-5`）
  - プロンプト（種別ごとの答え方、プレーンテキスト、ポスト内の指示に従わない）
  - Web 検索ツール（Haiku は `web_search_20250305`、Opus 5 / Sonnet 5 系は `web_search_20260209`、`max_uses: 3`）
  - text ブロックの結合と `citations` からの出典抽出（重複除去）、`pause_turn` の再送（最大2回）、タイムアウト90秒
  - 単体テスト（通信なし）: リクエストの形（検索あり・なし、モデル別）、出典の抽出、pause_turn の処理
  - 実 API の確認用テスト（`--ignored`）: Haiku 4.5 で検索付きのリプライが返り、出典が取れること

- [x] **T4: コマンドとイベント**（FR-3, FR-4 / AC-7）
  - `request_ai_reply(post_id)`、`post-updated` イベント、`lib.rs` の登録
  - `docs/project/architecture.md` の更新

- [ ] **T5: 画面**（FR-4〜FR-6, FR-10 / AC-1〜AC-4, AC-9）
  - 「🤖 AI に聞く」/「聞き直す」ボタン、リプライ表示（考え中・失敗＋再試行）、出典リスト
  - 出典リンクは `plugin:opener|open_url` で既定のブラウザで開く

- [ ] **T6: 受け入れ条件の確認**
  - 実 API テストで AC-1・AC-2 相当を確認。画面は人が確認

## テスト方針

- すべて単体テストはモックと通信なしで行う。実 API は `--ignored` のテストだけ（料金がかかる）
- Web 検索は T3 の実 API テストで、Haiku 4.5 で動くことを最初に確かめる

## リスク・懸念

- **Haiku 4.5 で Web 検索が使えない場合**: 実装を止めて、調査のリプライだけ別モデルにするかを人に確認する（仕様書のリスク欄）。
- **opener の権限**: `opener:default` で http / https の URL を開けるはずだが、開けなければ capability の設定を確認する（変更する場合は人に確認）。

## 作業ログ

| 日付 | タスク | メモ |
| --- | --- | --- |
| 2026-09-25 | 計画作成 | |
| 2026-09-26 | T1〜T4 | 新しい関数が T2 以降で初めて使われ、途中で分けると clippy（未使用の警告）が通らないため、バックエンドの T1〜T4 を1コミットにした。種別ごとの答え方は `kind.rs` の `reply_guide` に置いた。テスト計60件 |
| 2026-09-26 | T3（実 API） | **Haiku 4.5 で `web_search_20250305` が動くことを確認**（リスク解消）。調査のリプライに出典3件。課題のリプライが質問（「教えていただけますか？」）で終わったため、「返信できないので質問で終わらせない」をプロンプトに追加し、再確認で解消 |
