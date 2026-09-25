---
id: "000"
spec: ../specs/000-xxx.md
plan: ../plans/000-xxx.md
status: pending # pending | changes-requested | approved（approved は人のみ）
requested: YYYY-MM-DD
---

# 000: 機能名 — 確認依頼

## 変更概要

何を実装したか（3〜5行）。

## 変更ファイル

- `path/to/file.rs` — 内容

## 受け入れ条件の結果

| AC | 結果 | 確認方法 |
| --- | --- | --- |
| AC-1 | ✅ / ❌ / ⚠️ | テスト名 or 手動確認手順 |

## 実行した検証

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo tauri dev` で動作確認

## 人に確認してほしいこと

AI が判断できなかった点、仕様から逸脱した点、トレードオフ。

1.

## 動作確認手順（人向け）

1. `cargo tauri dev` で起動
2.

---

## 人のレビュー欄

<!-- 人が記入 -->

- 判定: <!-- approved / changes-requested -->
- コメント:
