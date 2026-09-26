use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::models::{
    now, ClassifiedTask, ClassifyStatus, Post, PostKind, PostWithTasks, Reply, ReplySource,
    ReplyStatus, Task, TaskStatus, Topic, TopicBrief, TopicRef,
};

pub const DB_FILE_NAME: &str = "x-clone.sqlite";
/// 開発用ビルドで使う DB（仕様004）
pub const DEV_DB_FILE_NAME: &str = "x-clone-dev.sqlite";

/// 使う DB ファイルと、開発版かどうか（仕様004）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbLocation {
    pub path: PathBuf,
    pub dev: bool,
    /// 環境変数 `X_CLONE_DB` で指定されたファイルか
    pub overridden: bool,
}

/// 使う DB ファイルを決める。優先順位: `X_CLONE_DB` > 開発用ビルドなら `x-clone-dev.sqlite` > `x-clone.sqlite`
pub fn resolve_db_location(
    env_override: Option<&str>,
    is_debug: bool,
    data_dir: &Path,
) -> DbLocation {
    if let Some(path) = env_override.map(str::trim).filter(|p| !p.is_empty()) {
        return DbLocation {
            path: PathBuf::from(path),
            dev: is_debug,
            overridden: true,
        };
    }
    let name = if is_debug {
        DEV_DB_FILE_NAME
    } else {
        DB_FILE_NAME
    };
    DbLocation {
        path: data_dir.join(name),
        dev: is_debug,
        overridden: false,
    }
}

/// 開発用 DB が無く、普段使いの DB があるときだけ、普段使いの DB をコピーする（Q-023）。コピーしたら true。
/// 普段使いの DB は読み取り専用で開き、`VACUUM INTO` で一貫したコピーを作る（コピー元は変更しない）。
pub fn prepare_dev_db(dev_path: &Path, prod_path: &Path) -> AppResult<bool> {
    if dev_path.exists() || !prod_path.exists() {
        return Ok(false);
    }
    let source = Connection::open_with_flags(prod_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.execute("VACUUM INTO ?1", [dev_path.to_string_lossy().as_ref()])?;
    Ok(true)
}

/// 版1: 仕様001のスキーマ。`IF NOT EXISTS` なので、版管理を入れる前に作られた DB（版0・テーブルあり）にも安全に当てられる。
const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS posts (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  body            TEXT    NOT NULL,
  created_at      TEXT    NOT NULL,
  classify_status TEXT    NOT NULL DEFAULT 'pending',
  classify_error  TEXT
);

CREATE TABLE IF NOT EXISTS tasks (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id    INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
  kind       TEXT    NOT NULL,
  title      TEXT    NOT NULL,
  target     TEXT    NOT NULL DEFAULT '',
  action     TEXT    NOT NULL,
  status     TEXT    NOT NULL DEFAULT 'open',
  created_at TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_post_id ON tasks(post_id);
"#;

/// DB の版ごとの更新。`MIGRATIONS[i]` を当てると版が `i + 1` になる。
/// テーブルや列を増やすときは、既存の要素は変えずに末尾へ追加する（B-012）。
const MIGRATIONS: &[&str] = &[
    // 版1: 仕様001（posts / tasks）
    SCHEMA_V1,
    // 版2: 仕様002 タスクの完了日時
    "ALTER TABLE tasks ADD COLUMN completed_at TEXT;",
    // 版3: 仕様003 リプライ
    r#"
    CREATE TABLE replies (
      id         INTEGER PRIMARY KEY AUTOINCREMENT,
      post_id    INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
      author     TEXT    NOT NULL,
      body       TEXT    NOT NULL DEFAULT '',
      sources    TEXT    NOT NULL DEFAULT '[]',
      model      TEXT,
      status     TEXT    NOT NULL,
      error      TEXT,
      created_at TEXT    NOT NULL
    );
    CREATE INDEX idx_replies_post_id ON replies(post_id);
    "#,
    // 版4: 仕様008 話題
    r#"
    CREATE TABLE topics (
      id             INTEGER PRIMARY KEY AUTOINCREMENT,
      name           TEXT    NOT NULL UNIQUE,
      description    TEXT    NOT NULL DEFAULT '',
      source_post_id INTEGER REFERENCES posts(id) ON DELETE SET NULL,
      created_at     TEXT    NOT NULL
    );
    CREATE TABLE post_topics (
      post_id     INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
      topic_id    INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
      assigned_by TEXT    NOT NULL,
      removed     INTEGER NOT NULL DEFAULT 0,
      created_at  TEXT    NOT NULL,
      PRIMARY KEY (post_id, topic_id)
    );
    CREATE INDEX idx_post_topics_topic_id ON post_topics(topic_id);
    ALTER TABLE posts ADD COLUMN kind TEXT NOT NULL DEFAULT 'post';
    "#,
];

#[cfg(test)]
const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

fn user_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// 足りない版の更新を順番に当てる。1版ずつトランザクションで行い、成功したら版数を上げる。
fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current = user_version(conn)?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = i as i64 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

/// DB ファイルを開き（無ければ作り）、スキーマを最新の版にする。
pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    init(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    init(&mut conn)?;
    Ok(conn)
}

fn init(conn: &mut Connection) -> AppResult<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(conn)
}

const POST_COLUMNS: &str = "id, body, created_at, classify_status, classify_error, kind";
const TASK_COLUMNS: &str =
    "id, post_id, kind, title, target, action, status, created_at, completed_at";

fn post_from_row(row: &Row<'_>) -> rusqlite::Result<Post> {
    Ok(Post {
        id: row.get(0)?,
        body: row.get(1)?,
        created_at: row.get(2)?,
        classify_status: row.get(3)?,
        classify_error: row.get(4)?,
        kind: row.get(5)?,
    })
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        post_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        target: row.get(4)?,
        action: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        completed_at: row.get(8)?,
    })
}

/// 検証済みの本文でポストを保存する。分類状態は pending。
pub fn insert_post(conn: &Connection, body: &str) -> AppResult<Post> {
    conn.execute(
        "INSERT INTO posts (body, created_at, classify_status) VALUES (?1, ?2, ?3)",
        params![body, now(), ClassifyStatus::Pending],
    )?;
    get_post(conn, conn.last_insert_rowid())
}

pub fn get_post(conn: &Connection, post_id: i64) -> AppResult<Post> {
    conn.query_row(
        &format!("SELECT {POST_COLUMNS} FROM posts WHERE id = ?1"),
        [post_id],
        post_from_row,
    )
    .optional()?
    .ok_or(AppError::PostNotFound(post_id))
}

fn tasks_of(conn: &Connection, post_id: i64) -> AppResult<Vec<Task>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TASK_COLUMNS} FROM tasks WHERE post_id = ?1 ORDER BY id"
    ))?;
    let tasks = stmt
        .query_map([post_id], task_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tasks)
}

const REPLY_COLUMNS: &str = "id, post_id, author, body, sources, model, status, error, created_at";

/// AI のリプライの `author`（仕様003）
pub const AI_AUTHOR: &str = "ai";

fn reply_from_row(row: &Row<'_>) -> rusqlite::Result<Reply> {
    let sources_json: String = row.get(4)?;
    let sources = serde_json::from_str(&sources_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(Reply {
        id: row.get(0)?,
        post_id: row.get(1)?,
        author: row.get(2)?,
        body: row.get(3)?,
        sources,
        model: row.get(5)?,
        status: row.get(6)?,
        error: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn replies_of(conn: &Connection, post_id: i64) -> AppResult<Vec<Reply>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {REPLY_COLUMNS} FROM replies WHERE post_id = ?1 ORDER BY id"
    ))?;
    let replies = stmt
        .query_map([post_id], reply_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(replies)
}

fn with_details(conn: &Connection, post: Post) -> AppResult<PostWithTasks> {
    let tasks = tasks_of(conn, post.id)?;
    let replies = replies_of(conn, post.id)?;
    let topics = topics_of(conn, post.id)?;
    Ok(PostWithTasks {
        post,
        tasks,
        replies,
        topics,
    })
}

/// すべてのタスクと、元のポストの本文を返す（課題ページの集計用、仕様006）
pub fn all_tasks_with_post_body(conn: &Connection) -> AppResult<Vec<(Task, String)>> {
    let columns = TASK_COLUMNS
        .split(", ")
        .map(|c| format!("t.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT {columns}, p.body FROM tasks t JOIN posts p ON p.id = t.post_id"
    ))?;
    let rows = stmt
        .query_map([], |row| Ok((task_from_row(row)?, row.get(9)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_post_with_tasks(conn: &Connection, post_id: i64) -> AppResult<PostWithTasks> {
    let post = get_post(conn, post_id)?;
    with_details(conn, post)
}

/// AI のリプライを「生成中」にする。既にある AI のリプライは消す（聞き直しで置き換えるため）。
pub fn start_ai_reply(conn: &mut Connection, post_id: i64, model: &str) -> AppResult<()> {
    get_post(conn, post_id)?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM replies WHERE post_id = ?1 AND author = ?2",
        params![post_id, AI_AUTHOR],
    )?;
    tx.execute(
        "INSERT INTO replies (post_id, author, model, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![post_id, AI_AUTHOR, model, ReplyStatus::Pending, now()],
    )?;
    tx.commit()?;
    Ok(())
}

/// AI のリプライの結果を保存する。成功なら本文と出典、失敗ならエラー文。
pub fn finish_ai_reply(
    conn: &Connection,
    post_id: i64,
    result: Result<(&str, &[ReplySource]), &str>,
) -> AppResult<()> {
    let updated = match result {
        Ok((body, sources)) => {
            let sources_json = serde_json::to_string(sources)
                .map_err(|e| AppError::Other(format!("出典を保存できませんでした: {e}")))?;
            conn.execute(
                "UPDATE replies SET body = ?1, sources = ?2, status = ?3, error = NULL
                 WHERE post_id = ?4 AND author = ?5",
                params![body, sources_json, ReplyStatus::Done, post_id, AI_AUTHOR],
            )?
        }
        Err(error) => conn.execute(
            "UPDATE replies SET status = ?1, error = ?2 WHERE post_id = ?3 AND author = ?4",
            params![ReplyStatus::Failed, error, post_id, AI_AUTHOR],
        )?,
    };
    if updated == 0 {
        return Err(AppError::PostNotFound(post_id));
    }
    Ok(())
}

/// 新しい順にポストを返す。`before_id` を渡すと、それより古いものだけを返す（ページング用）。
pub fn list_timeline(
    conn: &Connection,
    limit: u32,
    before_id: Option<i64>,
) -> AppResult<Vec<PostWithTasks>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {POST_COLUMNS} FROM posts
         WHERE (?1 IS NULL OR id < ?1)
         ORDER BY id DESC
         LIMIT ?2"
    ))?;
    let posts = stmt
        .query_map(params![before_id, limit], post_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    posts
        .into_iter()
        .map(|post| with_details(conn, post))
        .collect()
}

/// ポストのタスクをすべて消し、渡されたタスクで作り直す（再分類用）。
pub fn replace_tasks(
    conn: &mut Connection,
    post_id: i64,
    tasks: &[ClassifiedTask],
) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM tasks WHERE post_id = ?1", [post_id])?;
    let created_at = now();
    for t in tasks {
        tx.execute(
            "INSERT INTO tasks (post_id, kind, title, target, action, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![post_id, t.kind, t.title, t.target, t.action, created_at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn set_classify_status(
    conn: &Connection,
    post_id: i64,
    status: ClassifyStatus,
    error: Option<&str>,
) -> AppResult<()> {
    let updated = conn.execute(
        "UPDATE posts SET classify_status = ?1, classify_error = ?2 WHERE id = ?3",
        params![status, error, post_id],
    )?;
    if updated == 0 {
        return Err(AppError::PostNotFound(post_id));
    }
    Ok(())
}

/// タスクを完了・未完了にする（仕様002）。完了なら完了日時を記録し、未完了なら消す。
/// 画面の更新用に、そのタスクを含むポスト全体を返す。
pub fn set_task_status(
    conn: &Connection,
    task_id: i64,
    status: TaskStatus,
) -> AppResult<PostWithTasks> {
    let completed_at = match status {
        TaskStatus::Done => Some(now()),
        TaskStatus::Open => None,
    };
    let updated = conn.execute(
        "UPDATE tasks SET status = ?1, completed_at = ?2 WHERE id = ?3",
        params![status, completed_at, task_id],
    )?;
    if updated == 0 {
        return Err(AppError::TaskNotFound(task_id));
    }
    let post_id: i64 =
        conn.query_row("SELECT post_id FROM tasks WHERE id = ?1", [task_id], |r| {
            r.get(0)
        })?;
    get_post_with_tasks(conn, post_id)
}

// ---- 話題（仕様008） ----

/// ポストの付け方: AI が付けた / 人が操作した
const ASSIGNED_BY_AI: &str = "ai";
const ASSIGNED_BY_USER: &str = "user";

fn topics_of(conn: &Connection, post_id: i64) -> AppResult<Vec<TopicRef>> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.id, t.name FROM post_topics pt JOIN topics t ON t.id = pt.topic_id
         WHERE pt.post_id = ?1 AND pt.removed = 0 ORDER BY t.name",
    )?;
    let topics = stmt
        .query_map([post_id], |r| {
            Ok(TopicRef {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(topics)
}

/// 話題として投稿したポストを保存する。タスクには分類しないので、分類状態は最初から done。
pub fn insert_topic_post(conn: &Connection, body: &str) -> AppResult<Post> {
    conn.execute(
        "INSERT INTO posts (body, created_at, classify_status, kind) VALUES (?1, ?2, ?3, ?4)",
        params![body, now(), ClassifyStatus::Done, PostKind::Topic],
    )?;
    get_post(conn, conn.last_insert_rowid())
}

fn topic_name_taken(conn: &Connection, name: &str, except_id: Option<i64>) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM topics WHERE name = ?1 AND (?2 IS NULL OR id != ?2))",
        params![name, except_id],
        |r| r.get(0),
    )?)
}

/// 話題を作る。名前は検証済みのものを渡す。同じ名前があればエラー。
pub fn create_topic(
    conn: &Connection,
    name: &str,
    description: &str,
    source_post_id: Option<i64>,
) -> AppResult<Topic> {
    if topic_name_taken(conn, name, None)? {
        return Err(AppError::Validation(format!(
            "「{name}」という話題はすでにあります"
        )));
    }
    conn.execute(
        "INSERT INTO topics (name, description, source_post_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![name, description, source_post_id, now()],
    )?;
    get_topic(conn, conn.last_insert_rowid())
}

const TOPIC_SELECT: &str = "
    SELECT t.id, t.name, t.description, t.source_post_id, t.created_at,
      (SELECT COUNT(*) FROM post_topics pt WHERE pt.topic_id = t.id AND pt.removed = 0),
      (SELECT COUNT(*) FROM post_topics pt JOIN tasks k ON k.post_id = pt.post_id
         WHERE pt.topic_id = t.id AND pt.removed = 0 AND k.status = 'open')
    FROM topics t";

fn topic_from_row(r: &Row<'_>) -> rusqlite::Result<Topic> {
    Ok(Topic {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        source_post_id: r.get(3)?,
        created_at: r.get(4)?,
        post_count: r.get(5)?,
        open_task_count: r.get(6)?,
    })
}

pub fn get_topic(conn: &Connection, topic_id: i64) -> AppResult<Topic> {
    conn.query_row(
        &format!("{TOPIC_SELECT} WHERE t.id = ?1"),
        [topic_id],
        topic_from_row,
    )
    .optional()?
    .ok_or(AppError::TopicNotFound(topic_id))
}

/// 話題の一覧（新しい順、ポスト数・未完了のタスク数つき）
pub fn list_topics(conn: &Connection) -> AppResult<Vec<Topic>> {
    let mut stmt = conn.prepare(&format!("{TOPIC_SELECT} ORDER BY t.id DESC"))?;
    let topics = stmt
        .query_map([], topic_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(topics)
}

pub fn update_topic(
    conn: &Connection,
    topic_id: i64,
    name: &str,
    description: &str,
) -> AppResult<Topic> {
    if topic_name_taken(conn, name, Some(topic_id))? {
        return Err(AppError::Validation(format!(
            "「{name}」という話題はすでにあります"
        )));
    }
    let updated = conn.execute(
        "UPDATE topics SET name = ?1, description = ?2 WHERE id = ?3",
        params![name, description, topic_id],
    )?;
    if updated == 0 {
        return Err(AppError::TopicNotFound(topic_id));
    }
    get_topic(conn, topic_id)
}

/// 話題を削除する。ポストとタスクは消えない（振り分けだけ消える、FR-11）
pub fn delete_topic(conn: &Connection, topic_id: i64) -> AppResult<()> {
    let deleted = conn.execute("DELETE FROM topics WHERE id = ?1", [topic_id])?;
    if deleted == 0 {
        return Err(AppError::TopicNotFound(topic_id));
    }
    Ok(())
}

/// 分類のときに AI に渡す話題。最近使われた順（まだ使われていない話題は新しい順）に最大 `limit` 件（NFR-1）
pub fn topics_for_ai(conn: &Connection, limit: u32) -> AppResult<Vec<TopicBrief>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.description FROM topics t
         ORDER BY (SELECT MAX(pt.created_at) FROM post_topics pt WHERE pt.topic_id = t.id AND pt.removed = 0) DESC,
                  t.id DESC
         LIMIT ?1",
    )?;
    let topics = stmt
        .query_map([limit], |r| {
            Ok(TopicBrief {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(topics)
}

/// 分類の結果として、AI が選んだ話題をポストに付け直す。
/// 前回 AI が付けた話題は外し、人が外した話題（removed）は付け直さない。
pub fn set_ai_topics(conn: &mut Connection, post_id: i64, topic_ids: &[i64]) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM post_topics WHERE post_id = ?1 AND assigned_by = ?2 AND removed = 0",
        params![post_id, ASSIGNED_BY_AI],
    )?;
    let created_at = now();
    for topic_id in topic_ids {
        // すでに行がある（人が外した）ときは何もしない
        tx.execute(
            "INSERT OR IGNORE INTO post_topics (post_id, topic_id, assigned_by, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![post_id, topic_id, ASSIGNED_BY_AI, created_at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// 振り分け直し（話題を作ったとき、FR-6）で、ポストに話題を1つ足す。すでに付いている・外されたときは何もしない。
pub fn add_ai_topic(conn: &Connection, post_id: i64, topic_id: i64) -> AppResult<bool> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO post_topics (post_id, topic_id, assigned_by, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![post_id, topic_id, ASSIGNED_BY_AI, now()],
    )?;
    Ok(inserted > 0)
}

/// ポストから話題を外す（FR-9）。AI が付け直さないよう、行は残して removed にする。
pub fn remove_post_topic(
    conn: &Connection,
    post_id: i64,
    topic_id: i64,
) -> AppResult<PostWithTasks> {
    get_post(conn, post_id)?;
    get_topic(conn, topic_id)?;
    conn.execute(
        "INSERT INTO post_topics (post_id, topic_id, assigned_by, removed, created_at) VALUES (?1, ?2, ?3, 1, ?4)
         ON CONFLICT (post_id, topic_id) DO UPDATE SET removed = 1, assigned_by = ?3",
        params![post_id, topic_id, ASSIGNED_BY_USER, now()],
    )?;
    get_post_with_tasks(conn, post_id)
}

/// 振り分け直しの対象: 普段のポスト（話題として投稿したものを除く）の新しい順
pub fn recent_posts_for_matching(conn: &Connection, limit: u32) -> AppResult<Vec<(i64, String)>> {
    let mut stmt =
        conn.prepare("SELECT id, body FROM posts WHERE kind = 'post' ORDER BY id DESC LIMIT ?1")?;
    let posts = stmt
        .query_map([limit], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(posts)
}

pub const INTERRUPTED_MESSAGE: &str = "アプリの終了により分類が中断されました";

pub const REPLY_INTERRUPTED_MESSAGE: &str = "アプリの終了によりリプライの作成が中断されました";

/// 起動時に呼ぶ。前回の終了時に分類中だったポストと、生成中だったリプライを failed にして、再試行できるようにする。
/// 戻り値は failed にした件数（ポスト＋リプライ）。
pub fn fail_interrupted(conn: &Connection) -> AppResult<usize> {
    let posts = conn.execute(
        "UPDATE posts SET classify_status = ?1, classify_error = ?2 WHERE classify_status = ?3",
        params![
            ClassifyStatus::Failed,
            INTERRUPTED_MESSAGE,
            ClassifyStatus::Pending
        ],
    )?;
    let replies = conn.execute(
        "UPDATE replies SET status = ?1, error = ?2 WHERE status = ?3",
        params![
            ReplyStatus::Failed,
            REPLY_INTERRUPTED_MESSAGE,
            ReplyStatus::Pending
        ],
    )?;
    Ok(posts + replies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::TaskKind;

    fn source(n: u32) -> ReplySource {
        ReplySource {
            title: format!("ページ{n}"),
            url: format!("https://example.com/{n}"),
        }
    }

    #[test]
    fn ai_reply_lifecycle_pending_then_done() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "Rust の非同期ランタイムの違いを調べたい").unwrap();

        start_ai_reply(&mut conn, post.id, "claude-haiku-4-5").unwrap();
        let r = &get_post_with_tasks(&conn, post.id).unwrap().replies[0];
        assert_eq!(r.status, ReplyStatus::Pending);
        assert_eq!(r.author, AI_AUTHOR);
        assert_eq!(r.model.as_deref(), Some("claude-haiku-4-5"));

        finish_ai_reply(&conn, post.id, Ok(("調べ方です", &[source(1), source(2)]))).unwrap();
        let r = &get_post_with_tasks(&conn, post.id).unwrap().replies[0];
        assert_eq!(r.status, ReplyStatus::Done);
        assert_eq!(r.body, "調べ方です");
        assert_eq!(r.sources, vec![source(1), source(2)]);
        assert_eq!(r.error, None);
    }

    #[test]
    fn ai_reply_failure_records_error() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "x").unwrap();
        start_ai_reply(&mut conn, post.id, "m").unwrap();

        finish_ai_reply(&conn, post.id, Err("API キーが設定されていません")).unwrap();
        let r = &get_post_with_tasks(&conn, post.id).unwrap().replies[0];
        assert_eq!(r.status, ReplyStatus::Failed);
        assert_eq!(r.error.as_deref(), Some("API キーが設定されていません"));
    }

    #[test]
    fn asking_again_replaces_the_ai_reply() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "x").unwrap();
        start_ai_reply(&mut conn, post.id, "m").unwrap();
        finish_ai_reply(&conn, post.id, Ok(("古い", &[]))).unwrap();

        start_ai_reply(&mut conn, post.id, "m").unwrap();
        finish_ai_reply(&conn, post.id, Ok(("新しい", &[]))).unwrap();

        let replies = get_post_with_tasks(&conn, post.id).unwrap().replies;
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].body, "新しい");
    }

    #[test]
    fn replies_survive_reopen_via_timeline() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "x").unwrap();
        start_ai_reply(&mut conn, post.id, "m").unwrap();
        finish_ai_reply(&conn, post.id, Ok(("本文", &[source(1)]))).unwrap();

        let timeline = list_timeline(&conn, 10, None).unwrap();
        assert_eq!(timeline[0].replies[0].sources, vec![source(1)]);
    }

    #[test]
    fn ai_reply_for_missing_post_is_not_found() {
        let mut conn = open_in_memory().unwrap();
        assert!(matches!(
            start_ai_reply(&mut conn, 999, "m"),
            Err(AppError::PostNotFound(999))
        ));
    }

    #[test]
    fn interrupted_pending_replies_become_failed() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "x").unwrap();
        set_classify_status(&conn, post.id, ClassifyStatus::Done, None).unwrap();
        start_ai_reply(&mut conn, post.id, "m").unwrap();

        assert_eq!(fail_interrupted(&conn).unwrap(), 1);
        let r = &get_post_with_tasks(&conn, post.id).unwrap().replies[0];
        assert_eq!(r.status, ReplyStatus::Failed);
        assert_eq!(r.error.as_deref(), Some(REPLY_INTERRUPTED_MESSAGE));
    }

    fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        names.iter().any(|n| n == column)
    }

    fn post_with_two_tasks(conn: &mut Connection) -> (i64, i64, i64) {
        let post = insert_post(conn, "READMEを書く、あとCIも設定する").unwrap();
        replace_tasks(
            conn,
            post.id,
            &[
                task(TaskKind::Issue, "READMEを書く"),
                task(TaskKind::Issue, "CIを設定する"),
            ],
        )
        .unwrap();
        let tasks = get_post_with_tasks(conn, post.id).unwrap().tasks;
        (post.id, tasks[0].id, tasks[1].id)
    }

    #[test]
    fn completing_a_task_records_time_and_only_changes_that_task() {
        let mut conn = open_in_memory().unwrap();
        let (post_id, first, second) = post_with_two_tasks(&mut conn);

        let pwt = set_task_status(&conn, first, TaskStatus::Done).unwrap();

        assert_eq!(pwt.post.id, post_id);
        let t1 = pwt.tasks.iter().find(|t| t.id == first).unwrap();
        let t2 = pwt.tasks.iter().find(|t| t.id == second).unwrap();
        assert_eq!(t1.status, TaskStatus::Done);
        assert!(t1.completed_at.is_some());
        assert_eq!(t2.status, TaskStatus::Open);
        assert_eq!(t2.completed_at, None);
    }

    #[test]
    fn reopening_a_task_clears_completed_at() {
        let mut conn = open_in_memory().unwrap();
        let (_, first, _) = post_with_two_tasks(&mut conn);
        set_task_status(&conn, first, TaskStatus::Done).unwrap();

        let pwt = set_task_status(&conn, first, TaskStatus::Open).unwrap();
        let t1 = pwt.tasks.iter().find(|t| t.id == first).unwrap();
        assert_eq!(t1.status, TaskStatus::Open);
        assert_eq!(t1.completed_at, None);
    }

    #[test]
    fn missing_task_is_not_found_and_changes_nothing() {
        let mut conn = open_in_memory().unwrap();
        let (post_id, _, _) = post_with_two_tasks(&mut conn);

        assert!(matches!(
            set_task_status(&conn, 999, TaskStatus::Done),
            Err(AppError::TaskNotFound(999))
        ));
        let tasks = get_post_with_tasks(&conn, post_id).unwrap().tasks;
        assert!(tasks.iter().all(|t| t.status == TaskStatus::Open));
    }

    #[test]
    fn new_db_is_at_latest_version() {
        let conn = open_in_memory().unwrap();
        assert_eq!(user_version(&conn).unwrap(), LATEST_VERSION);
        assert!(has_column(&conn, "tasks", "completed_at"));
    }

    #[test]
    fn spec001_db_is_upgraded_keeping_data() {
        // 版管理を入れる前（仕様001）の DB を再現: 版0、テーブルとデータあり、completed_at なし
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        conn.execute(
            "INSERT INTO posts (body, created_at, classify_status) VALUES ('古いポスト', '2026-09-25T00:00:00Z', 'done')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (post_id, kind, title, action, created_at) VALUES (1, 'issue', '古いタスク', '直す', '2026-09-25T00:00:00Z')",
            [],
        )
        .unwrap();
        assert_eq!(user_version(&conn).unwrap(), 0);
        assert!(!has_column(&conn, "tasks", "completed_at"));

        init(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), LATEST_VERSION);
        assert!(has_column(&conn, "tasks", "completed_at"));
        let timeline = list_timeline(&conn, 10, None).unwrap();
        assert_eq!(timeline[0].post.body, "古いポスト");
        assert_eq!(timeline[0].tasks[0].title, "古いタスク");
    }

    #[test]
    fn db_location_depends_on_build_and_override() {
        let dir = Path::new("C:/data");

        let dev = resolve_db_location(None, true, dir);
        assert_eq!(dev.path, dir.join(DEV_DB_FILE_NAME));
        assert!(dev.dev && !dev.overridden);

        let release = resolve_db_location(None, false, dir);
        assert_eq!(release.path, dir.join(DB_FILE_NAME));
        assert!(!release.dev);

        let custom = resolve_db_location(Some(" D:/test.sqlite "), true, dir);
        assert_eq!(custom.path, PathBuf::from("D:/test.sqlite"));
        assert!(custom.overridden);

        // 空の指定は無視する
        assert_eq!(resolve_db_location(Some("  "), true, dir), dev);
    }

    /// テストごとの一時フォルダ（終わったら消す）
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("x-clone-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dev_db_is_copied_once_from_prod_without_changing_it() {
        let tmp = TempDir::new("devcopy");
        let prod = tmp.0.join(DB_FILE_NAME);
        let dev = tmp.0.join(DEV_DB_FILE_NAME);
        {
            let conn = open(&prod).unwrap();
            insert_post(&conn, "普段使いのポスト").unwrap();
        }
        let prod_modified = std::fs::metadata(&prod).unwrap().modified().unwrap();

        assert!(prepare_dev_db(&dev, &prod).unwrap());
        let conn = open(&dev).unwrap();
        assert_eq!(
            list_timeline(&conn, 10, None).unwrap()[0].post.body,
            "普段使いのポスト"
        );

        // 開発用 DB で書いても普段使いの DB は変わらない
        insert_post(&conn, "開発用のポスト").unwrap();
        drop(conn);
        assert_eq!(
            std::fs::metadata(&prod).unwrap().modified().unwrap(),
            prod_modified
        );
        let prod_conn = open(&prod).unwrap();
        assert_eq!(list_timeline(&prod_conn, 10, None).unwrap().len(), 1);
        drop(prod_conn);

        // 2回目はコピーしない（開発用 DB のデータが残る）
        assert!(!prepare_dev_db(&dev, &prod).unwrap());
        assert_eq!(
            list_timeline(&open(&dev).unwrap(), 10, None).unwrap().len(),
            2
        );
    }

    #[test]
    fn dev_db_is_not_created_when_prod_is_missing() {
        let tmp = TempDir::new("devnoprod");
        let dev = tmp.0.join(DEV_DB_FILE_NAME);
        assert!(!prepare_dev_db(&dev, &tmp.0.join(DB_FILE_NAME)).unwrap());
        assert!(!dev.exists());
    }

    /// 実際の DB の**コピー**を最新の版に上げ、データが残ることを確かめる（人の DB は触らない）。
    /// 実行: `X_CLONE_DB_COPY=<コピーのパス> cargo test -p x-clone real_db_copy -- --ignored --nocapture`
    #[test]
    #[ignore = "実際の DB のコピーを使う"]
    fn real_db_copy_is_upgraded_keeping_data() {
        let path =
            std::env::var("X_CLONE_DB_COPY").expect("X_CLONE_DB_COPY にコピーのパスを指定する");
        let before: (i64, i64) = {
            let conn = Connection::open(&path).unwrap();
            println!("更新前の版: {}", user_version(&conn).unwrap());
            (
                conn.query_row("SELECT COUNT(*) FROM posts", [], |r| r.get(0))
                    .unwrap(),
                conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
                    .unwrap(),
            )
        };

        let conn = open(Path::new(&path)).unwrap();
        let timeline = list_timeline(&conn, 1000, None).unwrap();
        let tasks: usize = timeline.iter().map(|p| p.tasks.len()).sum();
        println!(
            "更新後の版: {} / ポスト {}件（更新前 {}件）/ タスク {}件（更新前 {}件）",
            user_version(&conn).unwrap(),
            timeline.len(),
            before.0,
            tasks,
            before.1
        );
        assert_eq!(user_version(&conn).unwrap(), LATEST_VERSION);
        assert_eq!(timeline.len() as i64, before.0);
        assert_eq!(tasks as i64, before.1);
    }

    #[test]
    fn migration_runs_only_once() {
        let mut conn = open_in_memory().unwrap();
        // もう一度当てても、版2の ALTER TABLE が二重に走らない（走ればエラーになる）
        init(&mut conn).unwrap();
        assert_eq!(user_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn interrupted_pending_posts_become_failed() {
        let conn = open_in_memory().unwrap();
        let pending = insert_post(&conn, "a").unwrap();
        let done = insert_post(&conn, "b").unwrap();
        set_classify_status(&conn, done.id, ClassifyStatus::Done, None).unwrap();

        assert_eq!(fail_interrupted(&conn).unwrap(), 1);
        let p = get_post(&conn, pending.id).unwrap();
        assert_eq!(p.classify_status, ClassifyStatus::Failed);
        assert_eq!(p.classify_error.as_deref(), Some(INTERRUPTED_MESSAGE));
        assert_eq!(
            get_post(&conn, done.id).unwrap().classify_status,
            ClassifyStatus::Done
        );
    }

    fn task(kind: TaskKind, title: &str) -> ClassifiedTask {
        ClassifiedTask {
            kind,
            title: title.to_string(),
            target: String::new(),
            action: title.to_string(),
        }
    }

    #[test]
    fn insert_post_starts_pending() {
        let conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "READMEを書く").unwrap();
        assert_eq!(post.body, "READMEを書く");
        assert_eq!(post.classify_status, ClassifyStatus::Pending);
        assert_eq!(post.classify_error, None);
    }

    #[test]
    fn timeline_is_newest_first_and_pages() {
        let conn = open_in_memory().unwrap();
        let ids: Vec<i64> = (1..=5)
            .map(|i| insert_post(&conn, &format!("post {i}")).unwrap().id)
            .collect();

        let first = list_timeline(&conn, 2, None).unwrap();
        assert_eq!(
            first.iter().map(|p| p.post.id).collect::<Vec<_>>(),
            vec![ids[4], ids[3]]
        );

        let next = list_timeline(&conn, 2, Some(ids[3])).unwrap();
        assert_eq!(
            next.iter().map(|p| p.post.id).collect::<Vec<_>>(),
            vec![ids[2], ids[1]]
        );
    }

    #[test]
    fn replace_tasks_overwrites_previous_tasks() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "READMEを書く、あとCIも設定する").unwrap();

        replace_tasks(&mut conn, post.id, &[task(TaskKind::Issue, "古いタスク")]).unwrap();
        replace_tasks(
            &mut conn,
            post.id,
            &[
                task(TaskKind::Issue, "READMEを書く"),
                task(TaskKind::Issue, "CIを設定する"),
            ],
        )
        .unwrap();

        let pwt = get_post_with_tasks(&conn, post.id).unwrap();
        let titles: Vec<_> = pwt.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["READMEを書く", "CIを設定する"]);
        assert!(pwt.tasks.iter().all(|t| t.status == TaskStatus::Open));
    }

    #[test]
    fn deleting_post_deletes_its_tasks() {
        let mut conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "x").unwrap();
        replace_tasks(&mut conn, post.id, &[task(TaskKind::Idea, "x")]).unwrap();

        conn.execute("DELETE FROM posts WHERE id = ?1", [post.id])
            .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn set_classify_status_records_error() {
        let conn = open_in_memory().unwrap();
        let post = insert_post(&conn, "x").unwrap();
        set_classify_status(&conn, post.id, ClassifyStatus::Failed, Some("boom")).unwrap();

        let post = get_post(&conn, post.id).unwrap();
        assert_eq!(post.classify_status, ClassifyStatus::Failed);
        assert_eq!(post.classify_error.as_deref(), Some("boom"));
    }

    #[test]
    fn missing_post_is_not_found() {
        let conn = open_in_memory().unwrap();
        assert!(matches!(
            get_post(&conn, 999),
            Err(AppError::PostNotFound(999))
        ));
        assert!(matches!(
            set_classify_status(&conn, 999, ClassifyStatus::Done, None),
            Err(AppError::PostNotFound(999))
        ));
    }

    #[test]
    fn data_survives_reopen() {
        let dir = std::env::temp_dir().join(format!("x-clone-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(DB_FILE_NAME);
        let _ = std::fs::remove_file(&path);

        {
            let mut conn = open(&path).unwrap();
            let post = insert_post(&conn, "再起動テスト").unwrap();
            replace_tasks(&mut conn, post.id, &[task(TaskKind::Research, "調べる")]).unwrap();
        }

        let conn = open(&path).unwrap();
        let timeline = list_timeline(&conn, 10, None).unwrap();
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].post.body, "再起動テスト");
        assert_eq!(timeline[0].tasks[0].kind, TaskKind::Research);

        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
