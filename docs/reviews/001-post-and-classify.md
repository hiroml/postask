---
id: "001"
spec: ../specs/001-post-and-classify.md
plan: ../plans/001-post-and-classify.md
branch: feat/001-post-and-classify
status: approved # pending | changes-requested | approved（approved は人のみ）
requested: 2026-09-25
---

# 001: ポスト投稿と自動分類・タスク化 — 確認依頼

## 変更概要

既存の X クローン機能（ログイン・いいね等）を削除し、「ポストすると AI が課題・思い付き・調査に分類してタスクにする」機能に置き換えました。
ポストはすぐ保存・表示され、分類は裏で Claude API（`claude-haiku-4-5`）が行い、終わるとポストの下に種別ごとに色分けしたチップが出ます。
AI の呼び出しは `LlmProvider` トレイトの裏にあり、後でローカル LLM に差し替えられます（ADR-002）。

## 変更ファイル

| 場所 | 内容 |
| --- | --- |
| `src-tauri/src/lib.rs` | 起動処理（.env 読み込み、DB 作成、コマンド登録） |
| `src-tauri/src/commands.rs` | `create_post` / `list_timeline` / `retry_classification` / `list_kinds` |
| `src-tauri/src/db.rs` | posts / tasks テーブルと読み書き |
| `src-tauri/src/classify.rs` | 分類処理（AI 呼び出し → note 除外 → 保存） |
| `src-tauri/src/kind.rs` | 種別の定義（表示名・色・AI への説明）。増やすときはここだけ直す |
| `src-tauri/src/llm/` | `LlmProvider` トレイト、`ClaudeProvider`、テスト用 `MockProvider` |
| `src-tauri/src/config.rs`, `models.rs`, `error.rs` | 設定・型・エラー |
| `src/` | 画面（入力欄、タイムライン、タスクチップ）と Tauri 連携 |
| `styles.css` | 画面のスタイル（不要なログイン画面等は削除） |
| `src-tauri/Cargo.toml` | 承認済みクレートの追加（Q-008, Q-010） |

## 受け入れ条件の結果

| AC | 結果 | 確認方法 |
| --- | --- | --- |
| AC-1 課題 | ✅ | 実際の API で分類（`live_acceptance_examples`）→ `issue`「ログイン画面のバリデーションを直す」 |
| AC-2 調査 | ✅ | 同上 → `research`「Rust の非同期ランタイムの違いを調べる」 |
| AC-3 思い付き | ✅ | 同上 → `idea`「タスクを付箋っぽく表示する」 |
| AC-4 2つに分ける | ✅ | 同上 → `issue`「READMEを書く」「CIを設定する」 |
| AC-5 メモはタスクにしない | ✅ | 同上 → `note` のみ。保存しないことは単体テスト `note_only_post_has_no_tasks_but_remains` |
| AC-6 キー未設定でもポストできる | ✅ | 単体テスト（`failure_marks_post_failed_and_keeps_it`, `missing_key_fails_without_network`）＋ 人が画面で確認（2026-09-25） |
| AC-7 キー設定後の再試行 | ✅ | 単体テスト（`retry_after_failure_succeeds_and_clears_error`, `key_is_reloaded_when_missing_at_startup`）＋ 人が画面で確認（2026-09-25） |
| AC-8 再起動後も表示 | ✅ | 単体テスト `data_survives_reopen` ＋ 人が画面で確認（2026-09-25） |
| AC-9 モックでテスト | ✅ | `cargo test --workspace`（36件通過、API を呼ばない） |
| AC-10 281文字でボタン無効 | ✅ | バックエンドの検証は単体テスト済み ＋ 人が画面で確認（2026-09-25） |
| AC-11 .env が git に入らない | ✅ | `git ls-files` に `.env` なし、`.env.example` あり。`git status --ignored` で `.env` が無視されている |

## 実行した検証

- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`（wasm32 向けのフロントも実施）
- [x] `cargo test --workspace` — 36件通過（1件は API を呼ぶため通常は無視）
- [x] `cargo test -p x-clone live_ -- --ignored --nocapture` — 実際の API で AC-1〜AC-5 の5件すべて期待どおり
- [x] `trunk build`
- [x] `cargo tauri dev` で画面の動作確認 → 人が「動作確認手順」の全ケースを実施し、問題なし（2026-09-25）
  - 確認の途中で一度「ポストボタンが押せない」状態になったが、再現できず、その後は解消。AI が実アプリ（WebView2）に接続して入力・IME・コマンド応答を検査した範囲では異常なし。原因は未特定

## 人に確認してほしいこと

仕様書に書かれていない判断・変更をしています。問題があれば指摘してください。

1. **`list_kinds` コマンドを追加した**: 種別の表示名と色を `kind.rs` の1か所で管理するため（仕様 FR-4「定義は1か所に」）、フロントがそれを受け取る読み取り専用コマンドを足しました。
2. **起動時、分類中のまま残ったポストを「失敗」にする**: 分類中にアプリを閉じると `pending` のまま止まってしまうため、次の起動時に「アプリの終了により分類が中断されました」として失敗扱いにし、再試行ボタンを出します。
3. **Opus 5 使用時の `max_tokens` を 4096 にした**: 仕様は 1024 ですが、Opus 5 は思考（thinking）が既定で有効なため、出力が途中で切れないよう余裕を持たせました。Haiku（既定）は仕様どおり 1024 です。
4. **API キーを分類のたびに読み直す場合がある**: 起動時にキーが無かったときだけ、分類のたびに `.env` を読み直します（AC-7 を満たすため）。キーがあるときは起動時の値を使い続けます。
5. **「さらに読み込む」ボタン**: タイムラインは50件ずつ表示し、それより古いポストはボタンで読み込みます（仕様の `list_timeline(limit, before_id)` を使う画面側の実装）。
6. **reqwest のバージョン**: 最新の 0.13 を使いました。0.13 では TLS 機能名が `rustls-tls` から `rustls` に変わっています（承認済みの reqwest と同じクレートです）。
7. **古い DB ファイル**: `src-tauri/.x_clone.sqlite`（旧 X クローンのデモデータ）は使われなくなりました。git には入っていません。不要なら手で削除してください。

## 動作確認手順（人向け）

1. `cargo tauri dev` で起動する（初回はビルドに数分かかります）
2. **AC-10**: 281文字以上を入力 → 残り文字数が赤いマイナス表示になり「ポスト」が押せない。空白だけでも押せない
3. **AC-1〜AC-5**: 次の5つをポストし、数秒後にチップが出ることを確認する
   - `ログイン画面のバリデーションを直す` → 赤い「課題」
   - `Rust の非同期ランタイムの違いを調べたい` → 青い「調査」
   - `タスクを付箋っぽく表示したら面白いかも` → 黄色い「思い付き」
   - `READMEを書く、あとCIも設定する` → 「課題」が2つ
   - `今日は眠い` → チップなし（ポストは残る）
4. **AC-8**: アプリを閉じて再起動し、上のポストとチップがそのまま表示される
5. **AC-6**: アプリを閉じ、`.env` の `ANTHROPIC_API_KEY=` の値を一時的に消して起動 → 何かポストすると、ポストは表示されたうえで「⚠ 分類に失敗しました」と「再試行」が出る
6. **AC-7**: アプリを閉じずに `.env` にキーを戻して保存 → 「再試行」を押すと分類されてチップが出る

うまくいかなかった場合は、ターミナルに出たログと画面の様子を教えてください。

---

## 人のレビュー欄

<!-- 人が記入 -->

- 判定: approved（hikou、2026-09-25 会話上で承認。AI が代理記入）
- コメント: 全ケースの動作確認済み。仕様外の判断7点もあわせて承認。
