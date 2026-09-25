# 判断待ちボード

人に確認・決定してほしい事項の一覧です。AI は判断できないことをここに追加し、人が回答します。

## 使い方

**人**
1. 「回答待ち」の項目を読み、`回答:` に答えを書く（選択肢の記号だけでも、自由記述でもOK）
2. AI に「回答を反映して」と伝える（`/apply-answers`）

**AI**
- 新しい確認事項は「回答待ち」の末尾に、次の番号で追加する
- 回答された項目は、関連文書（仕様書など）に反映してから「決定済み」へ移す
- 設計に大きく影響する決定は `docs/decisions/` に ADR としても残す
- 回答の代わりに自分で決めてはいけない。推奨案は書いてよい

**優先度**: 🔴 これが決まらないと先に進めない / 🟡 実装前までに決めたい / 🟢 後で決めてもよい

項目の書き方:

```markdown
### Q-NNN 🔴 タイトル

- 関連: [仕様書など](specs/xxx.md)
- 背景: なぜ決める必要があるか
- 選択肢:
  - A. 〇〇
  - B. 〇〇
- AI の推奨: 〇（理由）
- 回答:
```

---

## 回答待ち

（なし）

---

## 決定済み

| ID | 内容 | 決定 | 日付 | 反映先 |
| --- | --- | --- | --- | --- |
| Q-001 | ポストの分類・AI リプライに使う AI | まず Claude API で試し、後からローカル LLM の API を試せるようにする | 2026-09-25 | [ADR-002](decisions/ADR-002-llm-provider.md), [overview](project/overview.md) |
| Q-002 | 利用者の想定 | 個人利用（ローカルで完結） | 2026-09-25 | [overview](project/overview.md) |
| Q-003 | 最初の仕様書の範囲 | ポスト＋自動分類（F1+F2） | 2026-09-25 | [001](specs/001-post-and-classify.md) |
| Q-004 | API キーの保存場所 | C: まず `.env` に置く（git には commit も push もしない）。設定画面とキーチェーンは後続の仕様 | 2026-09-25 | [001](specs/001-post-and-classify.md), `.gitignore`, `.env.example` |
| Q-005 | 分類の種類 | A: `issue` / `idea` / `research` / `note` の4種類で始め、後から追加する | 2026-09-25 | [001](specs/001-post-and-classify.md) |
| Q-006 | ポストの文字数制限 | A: 280文字。試しながら調整する | 2026-09-25 | [001](specs/001-post-and-classify.md) |
| Q-007 | 分類に使うモデル | B: `claude-haiku-4-5` で始め、精度が微妙なら `claude-opus-5` に変える | 2026-09-25 | [001](specs/001-post-and-classify.md) |
| Q-008 | 依存クレートの追加 | A: reqwest / tokio / async-trait / chrono / thiserror をすべて承認 | 2026-09-25 | [001](specs/001-post-and-classify.md) |
| Q-009 | 安全フィルタで断られたときの自動再試行 | A: 有効にする（対象は `claude-opus-5` 使用時。Haiku 4.5 では送らない） | 2026-09-25 | [001](specs/001-post-and-classify.md) |
| Q-010 | `.env` 読み込み用クレート | `dotenvy` を追加する | 2026-09-25 | [001](specs/001-post-and-classify.md) |
