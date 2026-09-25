---
id: "001"
title: ポスト投稿と自動分類・タスク化
status: review # draft | review | approved | done | rejected（approved/done は人のみ変更可）
created: 2026-09-25
updated: 2026-09-25
approved_by: # 承認した人の名前（人が記入）
---

# 001: ポスト投稿と自動分類・タスク化

## 背景・目的

タスク管理ツールは「登録の手間」で使われなくなりがち。本ツールでは X のように思ったことを
ポストするだけで、AI がそれを課題・思い付き・調査などに分類し、タスクに起こす。

この仕様は最小の体験「ポストする → 自動でタスクになる」を作る（overview の F1 + F2）。
可視化（F3）、AI リプライ（F4）、関連タスクの派生（F5）は後続の仕様で扱う。

## ユーザーストーリー

- ユーザーとして、思い付いたことを1行ポストするだけで記録したい。フォームを埋めるのが面倒だから。
- ユーザーとして、ポストが自動で「課題」「思い付き」「調査」などに分けられてほしい。後で整理する手間を省きたいから。
- ユーザーとして、1つのポストに複数のやることが含まれていたら、別々のタスクになってほしい。

## 要件

### 機能要件

- [ ] FR-1: 画面上部の入力欄に本文を書いて「ポスト」ボタン（または Ctrl+Enter）で投稿できる。
  - 本文は1〜280文字（前後の空白は除いて数える）。入力欄の下に残り文字数を表示し、0文字または280文字を超えたらボタンを押せなくする。
  - 上限値は定数1か所で定義し、後で変えやすくする（使ってみて調整する予定。Q-006）。
  - Tauri コマンド側でも同じチェックを行い、範囲外ならエラーを返す。
- [ ] FR-2: 投稿はすぐに SQLite に保存され、タイムラインの先頭に表示される（AI の処理を待たない）。
- [ ] FR-3: 投稿後、裏で AI がポストを分類し、0個以上のタスクを作る。
- [ ] FR-4: 各タスクには次の情報が付く。
  - 種別 `kind`: `issue`（課題・やること）/ `idea`（思い付き）/ `research`（調査）/ `note`（メモ。タスクにしない）
    - 後から種別を増やす予定（Q-005）。種別の定義（enum・表示名・色・AI への説明）は1か所にまとめ、追加がそこだけで済むようにする。
  - タイトル `title`: 「〇〇の△△をする」のような短い文
  - 対象 `target`: 「〇〇の」にあたる部分（例: 「ログイン画面」）。なければ空
  - 行動 `action`: 「△△をする」にあたる部分（例: 「バリデーションを追加する」）
- [ ] FR-5: 分類が終わると、タイムライン上のポストの下にタスクがチップ表示される（種別ごとに色分け）。
- [ ] FR-6: 分類中は「分類中…」、失敗したら「分類に失敗しました」と「再試行」ボタンを表示する。
- [ ] FR-7: タイムラインはアプリ再起動後もポストとタスクを表示する（新しい順）。
- [ ] FR-8: AI との通信部分はプロバイダを差し替えられる形で実装する（[ADR-002](../decisions/ADR-002-llm-provider.md)）。この仕様で実装するのは Claude のみ。

### 非機能要件

- NFR-1: ポストしてからタイムラインに表示されるまで 200ms 以内（AI 処理は含めない）。
- NFR-2: ネットワークが無い、または API キーが設定されていなくても、ポストの保存と表示はできる（分類だけ失敗扱いになる）。
- NFR-3: API キーをフロントエンド（WASM）側に渡さない。AI の呼び出しはすべて Tauri（Rust）側で行う。
- NFR-4: AI に送るのはポストの本文だけにする。

## 受け入れ条件

- [ ] AC-1: 「ログイン画面のバリデーションを直す」とポストすると、`issue` のタスク「ログイン画面のバリデーションを直す」ができ、ポストの下に表示される。
- [ ] AC-2: 「Rust の非同期ランタイムの違いを調べたい」とポストすると `research` のタスクができる。
- [ ] AC-3: 「タスクを付箋っぽく表示したら面白いかも」とポストすると `idea` のタスクができる。
- [ ] AC-4: 「READMEを書く、あとCIも設定する」とポストすると、`issue` のタスクが2つできる。
- [ ] AC-5: 「今日は眠い」とポストすると `note` と判定され、タスクは作られない（ポストは残る）。
- [ ] AC-6: API キー未設定の状態でポストしても、ポストは保存・表示され、「分類に失敗しました」と再試行ボタンが出る。
- [ ] AC-7: AC-6 の後に API キーを設定して再試行すると、分類されてタスクが表示される。
- [ ] AC-8: アプリを再起動しても、それまでのポストとタスクが同じように表示される。
- [ ] AC-9: 分類ロジックの単体テストが、Claude API を呼ばないモックのプロバイダで通る。
- [ ] AC-10: 281文字入力するとポストボタンが押せず、残り文字数がマイナスで表示される。空白だけのときも押せない。
- [ ] AC-11: `.env` がリポジトリに含まれていない（`git status` に出ない）。`.env.example` は含まれている。

## 画面・UI

```
┌──────────────────────────────────────────┐
│ ┌──────────────────────────────────────┐ │
│ │ いま何してる？ / 何が気になる？        │ │
│ └──────────────────────────────────────┘ │
│                              [ ポスト ]  │
├──────────────────────────────────────────┤
│ 09/25 14:02                              │
│ READMEを書く、あとCIも設定する            │
│ [課題] READMEを書く  [課題] CIを設定する  │
├──────────────────────────────────────────┤
│ 09/25 13:40                              │
│ Rust の非同期ランタイムの違いを調べたい   │
│ ⏳ 分類中…                               │
├──────────────────────────────────────────┤
│ 09/25 13:10                              │
│ ログイン画面のバリデーションを直す        │
│ ⚠ 分類に失敗しました  [再試行]            │
└──────────────────────────────────────────┘
```

- 種別チップの色: 課題=赤系 / 思い付き=黄系 / 調査=青系
- `note` はチップを出さない

## データ・API

### DB

```sql
CREATE TABLE posts (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  body            TEXT    NOT NULL,
  created_at      TEXT    NOT NULL,           -- ISO 8601 (UTC)
  classify_status TEXT    NOT NULL DEFAULT 'pending', -- pending | done | failed
  classify_error  TEXT                        -- 失敗理由（表示・デバッグ用）
);

CREATE TABLE tasks (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id    INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
  kind       TEXT    NOT NULL,                -- issue | idea | research
  title      TEXT    NOT NULL,
  target     TEXT    NOT NULL DEFAULT '',
  action     TEXT    NOT NULL,
  status     TEXT    NOT NULL DEFAULT 'open', -- open | done（完了操作は後続の仕様）
  created_at TEXT    NOT NULL
);
```

- 再試行のときは、そのポストの既存タスクを消してから作り直す。
- F5（関連タスク）のための `parent_task_id` は後続の仕様で追加する。

### Tauri コマンド

```rust
#[tauri::command]
async fn create_post(body: String) -> Result<Post, String>;             // 保存して即返す。分類は裏で開始する

#[tauri::command]
fn list_timeline(limit: u32, before_id: Option<i64>) -> Result<Vec<PostWithTasks>, String>;

#[tauri::command]
async fn retry_classification(post_id: i64) -> Result<(), String>;
```

- 分類が終わったら（成功・失敗とも）Tauri のイベント `post-classified`（payload: `PostWithTasks`）でフロントに知らせる。

### AI 呼び出し（分類）

プロバイダの抽象化（[ADR-002](../decisions/ADR-002-llm-provider.md)）:

```rust
#[async_trait]
trait LlmProvider: Send + Sync {
    async fn classify(&self, post_body: &str) -> Result<Vec<ClassifiedTask>, LlmError>;
}
// 実装: ClaudeProvider（この仕様）、MockProvider（テスト用）、LocalLlmProvider（後続）
```

Claude の実装（`ClaudeProvider`）:

- エンドポイント: `POST https://api.anthropic.com/v1/messages`（Rust の公式 SDK は無いので `reqwest` で直接呼ぶ）
- ヘッダ: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- モデル: `claude-haiku-4-5` から始める（Q-007）。精度が足りなければ `claude-opus-5` に切り替える。
  - `.env` の `CLAUDE_MODEL` で変えられるようにする（未設定なら `claude-haiku-4-5`）
- モデルによって送れるパラメータが違うので、モデルごとに切り替える。

  | パラメータ | `claude-haiku-4-5` | `claude-opus-5` |
  | --- | --- | --- |
  | `output_config.format`（構造化出力） | 送る | 送る |
  | `output_config.effort: "low"` | 送らない（Haiku 4.5 は非対応でエラーになる） | 送る（分類は単純なのでコストと応答時間を抑える） |
  | フォールバック（Q-009）: ヘッダ `anthropic-beta: server-side-fallback-2026-07-01` と `"fallbacks": "default"` | 送らない（対象外） | 送る（安全フィルタで断られたとき別モデルでやり直す） |

- `max_tokens`: 1024
- 出力形式は `output_config.format`（`type: "json_schema"`）で次のスキーマに固定する。

```json
{
  "type": "object",
  "properties": {
    "tasks": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "kind":   { "type": "string", "enum": ["issue", "idea", "research", "note"] },
          "title":  { "type": "string" },
          "target": { "type": "string" },
          "action": { "type": "string" }
        },
        "required": ["kind", "title", "target", "action"],
        "additionalProperties": false
      }
    }
  },
  "required": ["tasks"],
  "additionalProperties": false
}
```

- レスポンスは `stop_reason` を先に確認する。`end_turn` 以外（`max_tokens`, `refusal` など）は失敗扱いにする。
- 本文の text ブロックを JSON として読み、`kind: "note"` の要素は保存しない。
- API キーはプロジェクト直下の `.env` に `ANTHROPIC_API_KEY=...` として書き、起動時に `dotenvy` で読み込む（Q-004, Q-010）。
  - `.env` は `.gitignore` に入れ、commit も push もしない。
  - 書き方の見本として、値を空にした `.env.example` をリポジトリに入れる。
  - 設定画面から入力して OS のキーチェーンに保存する方式は、後続の仕様で扱う。
- タイムアウトは 30 秒。429 と 5xx は最大2回リトライし、それでも駄目なら失敗扱いにする。

### 追加する依存クレート（Q-008, Q-010 で承認済み）

| クレート | 用途 |
| --- | --- |
| `dotenvy` | `.env` から API キーとモデル名を読み込む |
| `reqwest`（`json`, `rustls-tls`） | Claude API の呼び出し |
| `tokio` | 非同期処理（Tauri が内部で使っているランタイムに合わせる） |
| `async-trait` | `LlmProvider` トレイトの定義 |
| `chrono` | 日時の扱い |
| `thiserror` | エラー型の定義 |

## スコープ外

- 分類結果をユーザーが手で直すこと（後続の仕様）
- タスクの完了・編集・削除
- 可視化（F3）、AI リプライ（F4）、関連タスクの派生（F5）
- ローカル LLM プロバイダの実装（抽象化だけ作っておく）
- ポストの編集・削除

## 未決事項

詳細と回答欄は [判断待ちボード](../questions.md) にある。

- [x] Q-004 API キーの保存場所 → `.env` に置く。git には入れない
- [x] Q-005 分類の種類 → 4種類で始め、後から追加する
- [x] Q-006 ポストの文字数制限 → 280文字。試しながら調整する
- [x] Q-007 分類に使うモデル → `claude-haiku-4-5` で始め、微妙なら `claude-opus-5`
- [x] Q-008 依存クレートの追加 → すべて承認
- [x] Q-009 安全フィルタで断られたときの自動再試行 → 有効（`claude-opus-5` 使用時のみ）
- [x] Q-010 `.env` 読み込み用の `dotenvy` 追加 → 承認

## 変更履歴

| 日付 | 内容 | 担当 |
| --- | --- | --- |
| 2026-09-25 | 初版 | AI |
| 2026-09-25 | Q-004〜Q-010 の回答を反映（.env、280文字、Haiku 4.5、モデル別パラメータ、dotenvy） | AI |
