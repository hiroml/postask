---
id: "004"
spec: ../specs/004-dev-db.md
plan: ../plans/004-dev-db.md
branch: feat/004-dev-db（feat/006-tasks-page に含まれる）
status: pending # pending | changes-requested | approved（approved は人のみ）
requested: 2026-09-26
---

# 004: 開発用 DB の分離 — 確認依頼

## 変更概要

開発用ビルド（`cargo tauri dev`）は `x-clone-dev.sqlite`、リリースビルドは今までどおり `x-clone.sqlite` を使うようにしました。
開発用 DB が無いときだけ、普段使いの DB を読み取り専用で開いてコピーします。開発版はタイトルと画面に「開発版」と出ます。

## 変更ファイル

| 場所 | 内容 |
| --- | --- |
| `src-tauri/src/db.rs` | `resolve_db_location`（使う DB の決定）、`prepare_dev_db`（初回コピー） |
| `src-tauri/src/lib.rs` | 起動時の DB の決定、パスの表示、開発版のタイトル |
| `src-tauri/src/commands.rs` | `app_info` コマンド |
| `src/`、`styles.css` | 「開発版」の印（005 でメニューの上に移動） |

## 受け入れ条件の結果

| AC | 結果 | 確認方法 |
| --- | --- | --- |
| AC-1 パス表示・タイトルと画面に「開発版」 | ✅ / ⚠️ | 起動時のパス表示は診断用の起動で確認（`DB: …\x-clone-dev.sqlite`）。画面の印はヘッドレスブラウザで確認。**タイトルは人の確認** |
| AC-2 開発版でポストしても普段使いの DB が変わらない | ✅ / ⚠️ | 単体テスト `dev_db_is_copied_once_from_prod_without_changing_it`。実環境でも、あなたの `cargo tauri dev` が自動再起動した後、`x-clone.sqlite` の更新日時が 9:45 のまま変わらないことを確認。ポストしての確認は人 |
| AC-3 初回は普段使いのデータで始まる | ✅ | 実環境で 10:24 に `x-clone-dev.sqlite` が作られた。単体テストでも確認 |
| AC-4 `X_CLONE_DB` の指定 | ✅ | 単体テスト `db_location_depends_on_build_and_override` |
| AC-5 DB の決め方の単体テスト | ✅ | 同上＋ `dev_db_is_not_created_when_prod_is_missing` |

## 人に確認してほしいこと

1. **コピーの方法**: 普段使いの DB を読み取り専用で開き、SQLite の `VACUUM INTO` で一貫したコピーを作っています（使用中でも壊れないため）。そのぶん空き領域が詰められ、コピーはファイルが小さくなります（40KB → 28KB）。中身は同じです。
2. **`.env` の API キーがコメントアウトされています**: 確認中に気づきました。`# ANTHROPIC_API_KEY=...` のように行頭に `#` があり、今は分類・AI リプライが失敗します（003 の AC-5 の確認で外したままだと思います）。`#` を外してください。AI はキーの中身は読んでいません。

## 動作確認手順（人向け）

1. `cargo tauri dev` を起動し直す → ターミナルに `DB: …\x-clone-dev.sqlite` が出て、ウィンドウのタイトルが「x-clone（開発版）」、メニューの上に「開発版」
2. 何かポストする → エクスプローラーで `%APPDATA%\com.hikou.x-clone\` を開き、`x-clone.sqlite` の更新日時が変わっていないこと（`x-clone-dev.sqlite` は変わる）

---

## 人のレビュー欄

<!-- 人が記入 -->

- 判定: <!-- approved / changes-requested -->
- コメント:
