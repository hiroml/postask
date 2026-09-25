---
id: "003"
spec: ../specs/003-ai-reply.md
plan: ../plans/003-ai-reply.md
branch: feat/003-ai-reply
status: pending # pending | changes-requested | approved（approved は人のみ）
requested: 2026-09-26
---

# 003: AI リプライ — 確認依頼

## 変更概要

分類が終わりタスクがあるポストに「🤖 AI に聞く」ボタンを付けました。押すと AI（`claude-haiku-4-5`）がポストへのリプライとして、
課題には解決の方針、調査には調べ方と概要、思い付きには試し方を書きます。調査のポストでは Web 検索を使い、出典（タイトルと URL）を表示し、クリックで既定のブラウザで開きます。
「聞き直す」で置き換え、失敗したら「再試行」できます。

## 変更ファイル

| 場所 | 内容 |
| --- | --- |
| `src-tauri/src/db.rs` | DB の版3（`replies` テーブル）、リプライの保存・取得、起動時に中断されたリプライを failed に |
| `src-tauri/src/reply.rs`（新規） | 依頼の受付（分類済み・タスクありのみ）と、AI 呼び出し・保存 |
| `src-tauri/src/llm/` | `LlmProvider::reply`、Claude の実装（Web 検索・出典の抽出・pause_turn の続き）、テスト用 Mock |
| `src-tauri/src/kind.rs` | 種別ごとのリプライの答え方（`reply_guide`） |
| `src-tauri/src/config.rs` | `CLAUDE_REPLY_MODEL`（既定 `claude-haiku-4-5`） |
| `src-tauri/src/commands.rs`, `lib.rs` | `request_ai_reply` コマンド、イベントを `post-updated` に改名 |
| `src/` と `styles.css` | ボタン、リプライ表示、出典リスト |
| `docs/project/architecture.md`, `.env.example` | データモデル・版管理・AI の設定・コマンド・イベント |

## 受け入れ条件の結果

| AC | 結果 | 確認方法 |
| --- | --- | --- |
| AC-1 調査に調べ方＋出典 | ✅ / ⚠️ | 実 API テスト `live_reply_examples` で、Haiku 4.5 ＋ Web 検索により出典3件付きのリプライを確認。画面での確認は人 |
| AC-2 課題に方針とステップ（出典なし） | ✅ / ⚠️ | 同じ実 API テストで確認。画面での確認は人 |
| AC-3 メモのみにはボタンなし、自動ではリプライしない | ✅ / ⚠️ | 単体テスト `posts_without_tasks_cannot_ask`、ヘッドレスブラウザで確認。実アプリは人 |
| AC-4 聞き直すと置き換わる | ✅ | 単体テスト `asking_again_replaces_previous_reply` ほか |
| AC-5 キーが無いと失敗＋再試行、ポスト・タスクは残る | ✅ / ⚠️ | 単体テスト `failure_marks_reply_failed_and_keeps_post_and_tasks`、ヘッドレスブラウザで確認。実アプリは人 |
| AC-6 再起動後も表示 | ✅ / ⚠️ | 単体テスト `replies_survive_reopen_via_timeline`。実アプリは人 |
| AC-7 生成中も他の操作ができる | ⚠️ 人の確認 | 生成は裏で行い、DB のロックは AI の応答待ちの間は持たない作り |
| AC-8 モックで単体テスト | ✅ | `cargo test --workspace` |
| AC-9 出典クリックでブラウザが開く | ⚠️ 人の確認 | ヘッドレスブラウザで `plugin:opener\|open_url` が呼ばれることは確認。実際にブラウザが開くかは人 |

## 実行した検証

- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`（wasm32 向けも実施）
- [x] `cargo test --workspace` — 60件通過（実 API・実 DB を使う3件は通常は無視）
- [x] `cargo test -p x-clone live_reply -- --ignored --nocapture` — 実 API で AC-1・AC-2 の例を確認（2回実行）
- [x] ヘッドレス Edge ＋偽の応答で「依頼 → 考え中 → 結果と出典 → 出典クリック → 失敗時の再試行」を確認
- [ ] `cargo tauri dev` での画面確認 → **人にお願いします**

## 人に確認してほしいこと

1. **プロンプトに「質問で終わらせない」を追加**: 実 API の確認で、課題のリプライが「教えていただけますか？」と質問で終わりました。このアプリではリプライに返信できないので、「情報が足りなければよくある場合を想定して答え切る」よう指示を足しました。
2. **失敗時は「聞き直す」ボタンを出さない**: 失敗時は「再試行」と「聞き直す」が同じ働きで重複するため、「再試行」だけにしました。
3. **Opus 5 などに切り替えた場合の設定**: リプライでは effort を `medium`、`max_tokens` を 8192 にしています（Haiku は 2048）。既定の Haiku には関係ありません。
4. **AI に送る内容**: ポスト本文と、タスクの種別名・タイトルだけです（仕様 NFR-2）。Web 検索を使うと、AI が作った検索語が検索に使われます。
5. **コミットの単位**: バックエンドの T1〜T4 は、途中で分けると clippy（未使用の警告）が通らないため、1コミットにしました。
6. **マージの方法**: 003 のブランチは 002 のブランチから作っているので、`feat/003-ai-reply` を `main` にマージすれば 002 も一緒に入ります。

## 動作確認手順（人向け）

`feat/003-ai-reply` ブランチで `cargo tauri dev` を起動してください（動いていれば起動し直す）。

1. **AC-3**: 「今日は眠い」をポスト → 分類後もボタンが出ない。他のポストにも、ポストしただけではリプライが付かない
2. **AC-1**: 「Rust の非同期ランタイムの違いを調べたい」をポスト → 分類後「🤖 AI に聞く」を押す → 「AI が考え中…」の後、調べ方と概要、その下に「出典」が出る（検索を含むので10〜20秒ほどかかる）
3. **AC-9**: 出典のリンクをクリック → 既定のブラウザでページが開く
4. **AC-7**: 2 の「考え中」の間に、別のポストの投稿やタスクの完了ができる
5. **AC-2**: 「ログイン画面のバリデーションを直す」で「AI に聞く」→ 方針とステップ。出典は出ない
6. **AC-4**: 「聞き直す」を押す → 新しいリプライに置き換わる
7. **AC-6**: アプリを再起動 → リプライと出典がそのまま出る
8. **AC-5**: アプリを止め、`.env` のキーを一時的に消して起動 → 「AI に聞く」で「リプライを作れませんでした」と「再試行」が出る。キーを戻して保存し「再試行」→ リプライが付く

---

## 人のレビュー欄

<!-- 人が記入 -->

- 判定: <!-- approved / changes-requested -->
- コメント:
