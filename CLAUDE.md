# CLAUDE.md

このファイルは AI（Claude Code）がこのリポジトリで作業するときの基本ルールです。

## プロジェクト概要

X（旧Twitter）のようにポストするだけで、AI が課題・思い付き・調査などに分類してタスク化・可視化する、個人用のタスク管理デスクトップアプリ。
詳細は [docs/project/overview.md](docs/project/overview.md)。

| レイヤ | 技術 | 場所 |
| --- | --- | --- |
| フロントエンド | Leptos 0.8 (CSR) → WASM, Trunk | `src/`, `index.html`, `styles.css` |
| デスクトップシェル / コマンド | Tauri 2 | `src-tauri/` |
| DB | SQLite (rusqlite, bundled) | `src-tauri/` |
| AI | Claude API（reqwest で直接呼ぶ。キーは `.env`） | `src-tauri/src/llm/` |
| バックエンド（予定） | Rust | `backend/` |

## よく使うコマンド

```sh
cargo tauri dev          # 開発起動（trunk serve --port 1421 が自動で走る）
cargo tauri build        # リリースビルド
trunk build              # フロントエンドのみビルド
cargo check --workspace  # 型チェック
cargo test --workspace   # テスト
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

## 開発ワークフロー（必ず守ること）

```
要望 → [AI] 仕様書ドラフト → ★人の承認 → [AI] 実装計画 → [AI] 実装・テスト → [AI] レビュー依頼 → ★人の確認
```

1. **仕様を書く**: 新機能・大きめの変更は `/write-spec` で `docs/specs/` に仕様書を作る。
2. **承認を待つ（ゲート1）**: 仕様書の `status` が `approved` になるまで実装に着手しない。
   AI が自分で `approved` に変更してはいけない。
3. **計画する**: `/plan-from-spec` で `docs/plans/` にタスク分解を作る。
4. **実装する**: `/implement-task` で計画のタスクを1つずつ実装し、チェックを更新する。
5. **レビュー依頼（ゲート2）**: `/request-review` で `docs/reviews/` に確認依頼を作り、人の確認を待つ。

小さなバグ修正・typo・リファクタ等は仕様書不要。ただし判断に迷ったら人に聞く。

## 人に確認すべきこと（勝手に決めない）

- 仕様書に書かれていない振る舞いの追加・変更
- DB スキーマの変更、データ削除を伴う操作
- 新しい依存クレートの追加
- Tauri の capabilities / セキュリティ設定（CSP 等）の変更
- 公開 API（Tauri コマンドのシグネチャ）の破壊的変更
- 仕様と実装が食い違うことに気づいたとき

不明点・判断が必要なことは [docs/questions.md](docs/questions.md)（判断待ちボード）に追加する。
人の回答は `/apply-answers` で関連文書に反映する。回答を待たずに推奨案で決めてはいけない。

## Git 運用（必ず守ること）

要件（docs/）とソースコードは、すべて git で管理する。**ファイルを変更したら、作業のまとまりごとに必ず `git add` と `git commit` を行う。**

### いつ commit するか

- 仕様書・判断待ちボード・ADR などの文書を作成・更新したとき（スキル1回の実行 = 1コミットが目安）
- 実装計画のタスクを1つ終えたとき（1タスク = 1コミット）
- 人の回答を反映したとき
- 作業の途中でターンを終えるときも、壊れていない状態なら commit しておく

### ブランチ

| ブランチ | 用途 |
| --- | --- |
| `main` | 承認済みの状態。文書だけの変更（仕様書・ボード・ADR・計画）はここに直接 commit してよい |
| `feat/NNN-名前` | 仕様 NNN の実装用。`/implement-task` の最初のタスクで `main` から作る |

- 実装ブランチは、人のレビュー（ゲート2）で承認されてから `main` にマージする（`git merge --no-ff`）。マージは人の指示を受けて行う。

### commit の手順

1. `git status` で変更内容を確認する。`.env` や API キーなどの秘密情報が含まれていないことを必ず確かめる
2. コードの変更を含むときは、`cargo fmt --all`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace` が通ってから commit する
3. 関係するファイルを `git add` する
4. 次の形式で commit する

```
<種別>(<仕様番号>): <要約>

<何をなぜ変えたか。関連する Q-NNN や AC があれば書く>

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
```

- 種別: `docs`（文書）/ `feat`（機能）/ `fix`（バグ修正）/ `refactor` / `test` / `chore`（設定・依存など）
- 例: `docs(001): 判断待ちボードの回答を仕様書に反映`、`feat(001): posts テーブルと create_post コマンドを追加`

### 禁止事項

- `.env`（API キー）を commit・push すること。見本は値を空にした `.env.example` に書く
- 人の指示なしに `git push` すること（リモートの設定も人が行う）
- `--no-verify` でフックを飛ばすこと、`push --force`、`reset --hard` など履歴を消す操作
- テストや clippy が失敗したままコードを commit すること

## コーディング規約

- `cargo fmt` と `cargo clippy -D warnings` を通すこと
- エラーは `unwrap()` せず `Result` で扱う（テストコードを除く）
- Tauri コマンドは `src-tauri/src/` に置き、フロントからは `invoke` で呼ぶ
- 大きな設計判断は `docs/decisions/` に ADR として残す

## ドキュメント構成

- [docs/README.md](docs/README.md) — ドキュメント全体の案内
- [docs/questions.md](docs/questions.md) — 判断待ちボード（人への確認事項と決定の記録）
- `docs/project/` — 概要・用語集・アーキテクチャ
- `docs/specs/` — 機能仕様書
- `docs/plans/` — 実装計画（タスク分解）
- `docs/reviews/` — 人への確認依頼・レビュー記録
- `docs/decisions/` — ADR（設計判断の記録）
- `.claude/skills/` — AI 用のワークフロー手順
- `.claude/agents/` — 専用サブエージェント
