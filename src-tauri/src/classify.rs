//! ポストの分類処理。AI に問い合わせ、結果をタスクとして保存する。

use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::llm::LlmProvider;
use crate::models::{
    ClassifiedTask, ClassifyStatus, PostKind, PostWithTasks, TopicBrief, MAX_TOPICS_PER_POST,
};

pub fn lock(conn: &Mutex<Connection>) -> AppResult<MutexGuard<'_, Connection>> {
    conn.lock()
        .map_err(|_| AppError::Other("データベースのロックに失敗しました".to_string()))
}

/// AI の結果から、保存するタスクだけを残す（`note` と空のタイトルを除く）。
fn tasks_to_save(tasks: Vec<ClassifiedTask>) -> Vec<ClassifiedTask> {
    tasks
        .into_iter()
        .filter(|t| t.kind.creates_task() && !t.title.trim().is_empty())
        .collect()
}

/// 分類のときに AI に渡す話題の最大数（仕様008 NFR-1）
pub const MAX_TOPICS_FOR_AI: u32 = 50;

/// AI が選んだ話題のうち、渡した話題にあるものだけを、重複なく最大3個残す（仕様008 FR-4）。
/// 知らない id は捨てるだけで、分類の失敗にはしない（NFR-2）。
fn topics_to_save(ids: &[i64], topics: &[TopicBrief]) -> Vec<i64> {
    let mut out = Vec::new();
    for id in ids {
        if topics.iter().any(|t| t.id == *id) && !out.contains(id) {
            out.push(*id);
        }
    }
    out.truncate(MAX_TOPICS_PER_POST);
    out
}

/// ポストを分類して結果を保存し、最新の状態を返す。
///
/// AI の失敗はエラーにせず、ポストを `failed` にして返す（ポスト自体は残す）。
/// DB のロックは AI の応答待ちの間は持たない。
pub async fn classify_post(
    conn: &Mutex<Connection>,
    provider: &dyn LlmProvider,
    post_id: i64,
) -> AppResult<PostWithTasks> {
    let (body, topics) = {
        let c = lock(conn)?;
        let post = db::get_post(&c, post_id)?;
        // 話題として投稿したものはタスクに分類しない（仕様008 FR-3）
        if post.kind == PostKind::Topic {
            return db::get_post_with_tasks(&c, post_id);
        }
        db::set_classify_status(&c, post_id, ClassifyStatus::Pending, None)?;
        (post.body, db::topics_for_ai(&c, MAX_TOPICS_FOR_AI)?)
    };

    let result = provider.classify(&body, &topics).await;

    let mut c = lock(conn)?;
    match result {
        Ok(classification) => {
            db::replace_tasks(&mut c, post_id, &tasks_to_save(classification.tasks))?;
            let topic_ids = topics_to_save(&classification.topic_ids, &topics);
            db::set_ai_topics(&mut c, post_id, &topic_ids)?;
            db::set_classify_status(&c, post_id, ClassifyStatus::Done, None)?;
        }
        Err(e) => {
            db::set_classify_status(&c, post_id, ClassifyStatus::Failed, Some(&e.to_string()))?;
        }
    }
    db::get_post_with_tasks(&c, post_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::TaskKind;
    use crate::llm::mock::MockProvider;

    fn task(kind: TaskKind, title: &str) -> ClassifiedTask {
        ClassifiedTask {
            kind,
            title: title.to_string(),
            target: String::new(),
            action: title.to_string(),
        }
    }

    fn setup(body: &str) -> (Mutex<Connection>, i64) {
        let conn = db::open_in_memory().unwrap();
        let id = db::insert_post(&conn, body).unwrap().id;
        (Mutex::new(conn), id)
    }

    #[tokio::test]
    async fn multiple_tasks_are_saved() {
        let (conn, id) = setup("READMEを書く、あとCIも設定する");
        let provider = MockProvider::tasks(vec![
            task(TaskKind::Issue, "READMEを書く"),
            task(TaskKind::Issue, "CIを設定する"),
        ]);

        let result = classify_post(&conn, &provider, id).await.unwrap();

        assert_eq!(result.post.classify_status, ClassifyStatus::Done);
        assert_eq!(result.tasks.len(), 2);
        assert!(result.tasks.iter().all(|t| t.kind == TaskKind::Issue));
    }

    #[tokio::test]
    async fn note_only_post_has_no_tasks_but_remains() {
        let (conn, id) = setup("今日は眠い");
        let provider = MockProvider::tasks(vec![task(TaskKind::Note, "今日は眠い")]);

        let result = classify_post(&conn, &provider, id).await.unwrap();

        assert_eq!(result.post.classify_status, ClassifyStatus::Done);
        assert_eq!(result.post.body, "今日は眠い");
        assert!(result.tasks.is_empty());
    }

    #[tokio::test]
    async fn blank_titles_are_dropped() {
        let (conn, id) = setup("x");
        let provider = MockProvider::tasks(vec![
            task(TaskKind::Idea, "  "),
            task(TaskKind::Idea, "付箋っぽく表示する"),
        ]);

        let result = classify_post(&conn, &provider, id).await.unwrap();
        assert_eq!(result.tasks.len(), 1);
    }

    #[tokio::test]
    async fn failure_marks_post_failed_and_keeps_it() {
        let (conn, id) = setup("ログイン画面のバリデーションを直す");

        let result = classify_post(&conn, &MockProvider::failing(), id)
            .await
            .unwrap();

        assert_eq!(result.post.classify_status, ClassifyStatus::Failed);
        assert!(result
            .post
            .classify_error
            .as_deref()
            .unwrap()
            .contains("API キー"));
        assert!(result.tasks.is_empty());
    }

    #[tokio::test]
    async fn retry_after_failure_succeeds_and_clears_error() {
        let (conn, id) = setup("ログイン画面のバリデーションを直す");
        classify_post(&conn, &MockProvider::failing(), id)
            .await
            .unwrap();

        let provider = MockProvider::tasks(vec![task(
            TaskKind::Issue,
            "ログイン画面のバリデーションを直す",
        )]);
        let result = classify_post(&conn, &provider, id).await.unwrap();

        assert_eq!(result.post.classify_status, ClassifyStatus::Done);
        assert_eq!(result.post.classify_error, None);
        assert_eq!(result.tasks.len(), 1);
    }

    #[tokio::test]
    async fn missing_post_is_an_error() {
        let (conn, _) = setup("x");
        let provider = MockProvider::tasks(vec![]);
        assert!(matches!(
            classify_post(&conn, &provider, 999).await,
            Err(AppError::PostNotFound(999))
        ));
    }
}
