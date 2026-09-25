use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::models::{now, ClassifiedTask, ClassifyStatus, Post, PostWithTasks, Task, TaskStatus};

pub const DB_FILE_NAME: &str = "x-clone.sqlite";

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

const POST_COLUMNS: &str = "id, body, created_at, classify_status, classify_error";
const TASK_COLUMNS: &str =
    "id, post_id, kind, title, target, action, status, created_at, completed_at";

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

pub const INTERRUPTED_MESSAGE: &str = "アプリの終了により分類が中断されました";

/// 起動時に呼ぶ。前回の終了時に分類中だったポストを failed にして、再試行できるようにする。
pub fn fail_interrupted(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE posts SET classify_status = ?1, classify_error = ?2 WHERE classify_status = ?3",
        params![
            ClassifyStatus::Failed,
            INTERRUPTED_MESSAGE,
            ClassifyStatus::Pending
        ],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::TaskKind;

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
