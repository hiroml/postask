---
id: "002"
spec: ../specs/002-complete-task.md
plan: ../plans/002-complete-task.md
branch: feat/002-complete-task（feat/003-ai-reply に含まれる）
status: pending # pending | changes-requested | approved（approved は人のみ）
requested: 2026-09-26
---

# 002: タスクを完了にする — 確認依頼

## 変更概要

タスクチップの左端にチェック（○）を付け、クリックで完了（✓・取り消し線・薄い色）と未完了を切り替えられるようにしました。
完了日時を記録し、マウスを乗せると表示します。あわせて、DB の形を安全に更新する仕組み（マイグレーション、B-012 の最小版）を作りました。

## 変更ファイル

| 場所 | 内容 |
| --- | --- |
| `src-tauri/src/db.rs` | DB の版管理（`MIGRATIONS` と `PRAGMA user_version`）、`set_task_status` |
| `src-tauri/src/models.rs` | `TaskStatus`、`Task.completed_at`、DB 保存用の enum マクロ `sql_text_enum!` |
| `src-tauri/src/commands.rs`, `lib.rs`, `error.rs` | `set_task_status` コマンド、`TaskNotFound` |
| `src/components/post_card.rs`, `src/app.rs`, `src/models.rs` | チェック、完了の見た目、先に表示を変えて失敗時に戻す処理 |
| `styles.css` | チェックと完了の見た目 |

## 受け入れ条件の結果

| AC | 結果 | 確認方法 |
| --- | --- | --- |
| AC-1 ○ で完了、取り消し線・薄い色 | ⚠️ 人の確認 | ヘッドレスブラウザ＋偽の応答では確認済み。実アプリでの確認が必要 |
| AC-2 もう一度で未完了に戻る | ⚠️ 人の確認 | 同上 |
| AC-3 再起動後も完了のまま | ⚠️ 人の確認 | DB への保存は単体テストで確認済み |
| AC-4 クリックしたタスクだけ切り替わる | ✅ | 単体テスト `completing_a_task_records_time_and_only_changes_that_task` |
| AC-5 存在しない ID はエラー・DB 変化なし | ✅ | 単体テスト `missing_task_is_not_found_and_changes_nothing` |
| AC-6 完了日時の記録と消去、ツールチップ | ✅ / ⚠️ | 記録・消去は単体テスト。ツールチップはヘッドレスブラウザで確認済み、実アプリは人の確認 |
| AC-7 仕様001の DB が更新されデータが残る | ✅ | 単体テスト `spec001_db_is_upgraded_keeping_data`。**実際の DB でも更新済み**（下記「人に確認してほしいこと」1） |
| AC-8 更新が二重に走らない | ✅ | 単体テスト `migration_runs_only_once` |

## 実行した検証

- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`（wasm32 向けも実施）
- [x] `cargo test --workspace` — 002・003 合わせて60件通過（実 API・実 DB を使う3件は通常は無視）
- [x] ヘッドレス Edge ＋偽の Tauri 応答で「完了 → 戻す → 保存失敗で元に戻る」を確認
- [x] 実際の DB のコピーで、版の更新後もポスト9件・タスク9件が残ることを確認
- [ ] `cargo tauri dev` での画面確認 → **人にお願いします**

## 人に確認してほしいこと

1. **実際の DB がすでに新しい版（3）に更新されています**: 作業中、あなたの `cargo tauri dev` が動いていて、コードの変更を検知してアプリを自動で再起動したため、実装ブランチのコードで DB が更新されました（2026-09-26 00:14）。
   確認したところデータは無事でした（ポスト9件・タスク9件）。念のためのコピーを `%APPDATA%\com.hikou.x-clone\x-clone.sqlite.bak-20260926-v3` に置いています（更新後の状態のコピーです）。再発防止の案をバックログ B-022 に起票しました。
2. **T1 のコミットで clippy の失敗を見落としていました**: すぐ次のコミット（`6b1ce1c`）で直し、以降は検証が通ったときだけ commit するようにしました。
3. **内部の整理**: DB に文字列で保存する enum 用のマクロ（`sql_text_enum!`）を作り、既存の分類状態もこれに揃えました（振る舞いは変わりません）。

## 動作確認手順（人向け）

003 と同じブランチ（`feat/003-ai-reply`）にいる状態で行ってください。

1. `cargo tauri dev` を起動する（動いていれば、いったん止めて起動し直す）
2. **AC-1**: タスクチップの左の ○ をクリック → ✓ になり、タイトルに取り消し線が付いて薄くなる
3. **AC-6**: 完了したチップにマウスを乗せる → 「完了: MM/DD HH:MM」が出る
4. **AC-2**: ✓ をもう一度クリック → 元に戻る
5. **AC-4**: 「READMEを書く、あとCIも設定する」のように2つタスクがあるポストで、片方だけ切り替わる
6. **AC-3**: 1つ完了にしてからアプリを再起動 → 完了のまま

---

## 人のレビュー欄

<!-- 人が記入 -->

- 判定: <!-- approved / changes-requested -->
- コメント:
