use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::models::{now, ClassifiedTask, ClassifyStatus, Post, PostWithTasks, Task};

pub const DB_FILE_NAME: &str = "x-clone.sqlite";

const SCHEMA: &str = r#"
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

/// DB ファイルを開き（無ければ作り）、スキーマを用意する。
pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

const POST_COLUMNS: &str = "id, body, created_at, classify_status, classify_error";
const TASK_COLUMNS: &str = "id, post_id, kind, title, target, action, status, created_at";

fn post_from_row(row: &Row<'_>) -> rusqlite::Result<Post> {
    Ok(Post {
        id: row.get(0)?,
        body: row.get(1)?,
        created_at: row.get(2)?,
        classify_status: row.get(3)?,
        classify_error: row.get(4)?,
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

pub fn get_post_with_tasks(conn: &Connection, post_id: i64) -> AppResult<PostWithTasks> {
    let post = get_post(conn, post_id)?;
    let tasks = tasks_of(conn, post_id)?;
    Ok(PostWithTasks { post, tasks })
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
        .map(|post| {
            let tasks = tasks_of(conn, post.id)?;
            Ok(PostWithTasks { post, tasks })
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::TaskKind;

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
        assert!(pwt.tasks.iter().all(|t| t.status == "open"));
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
