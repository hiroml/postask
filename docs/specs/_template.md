---
id: "000"
title: 機能名
status: draft # draft | review | approved | done | rejected（approved/done は人のみ変更可）
created: YYYY-MM-DD
updated: YYYY-MM-DD
approved_by: # 承認した人の名前（人が記入）
---

# 000: 機能名

## 背景・目的

なぜこの機能が必要か。誰のどんな課題を解決するか。

## ユーザーストーリー

- ユーザーとして、〇〇したい。なぜなら〇〇だから。

## 要件

### 機能要件

- [ ] FR-1:
- [ ] FR-2:

### 非機能要件

- NFR-1:

## 受け入れ条件

実装完了の判定基準。人が確認するときのチェックリストになる。

- [ ] AC-1: 〇〇のとき、〇〇となる
- [ ] AC-2:

## 画面・UI

画面の構成、操作の流れ（必要ならASCII図やMermaid）。

## データ・API

- DB の変更:
- Tauri コマンド:

```rust
#[tauri::command]
fn example(arg: String) -> Result<Output, String>
```

## スコープ外

今回やらないこと。

## 未決事項

人に判断してほしい点。詳細と回答欄は [判断待ちボード](../questions.md) に書き、ここにはリンクだけを置く。
承認前にすべて解消すること。

- [ ] [Q-000](../questions.md) 項目名

## 変更履歴

| 日付 | 内容 | 担当 |
| --- | --- | --- |
| YYYY-MM-DD | 初版 | AI |
