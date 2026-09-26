//! AI リプライの処理（仕様003）。依頼で「生成中」を保存し、裏で AI を呼んで結果を保存する。

use std::sync::Mutex;

use rusqlite::Connection;

use crate::classify::lock;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::kind::TaskKind;
use crate::llm::LlmProvider;
use crate::models::{ClassifyStatus, PostWithTasks, Task};

/// 裏で AI に渡す内容
pub struct ReplyJob {
    pub post_id: i64,
    post_body: String,
    tasks: Vec<Task>,
    use_web_search: bool,
}

/// 依頼を受け付ける。分類済みでタスクがあるポストだけを対象にし、リプライを「生成中」にする。
/// 調査（research）のタスクがあれば Web 検索を使う（Q-017）。
pub fn start(
    conn: &Mutex<Connection>,
    provider: &dyn LlmProvider,
    post_id: i64,
) -> AppResult<ReplyJob> {
    let mut c = lock(conn)?;
    let pwt = db::get_post_with_tasks(&c, post_id)?;
    if pwt.post.classify_status != ClassifyStatus::Done || pwt.tasks.is_empty() {
        return Err(AppError::Validation(
            "分類が終わり、タスクがあるポストだけ AI に聞けます".to_string(),
        ));
    }
    db::start_ai_reply(&mut c, post_id, &provider.reply_model())?;

    let use_web_search = pwt.tasks.iter().any(|t| t.kind == TaskKind::Research);
    Ok(ReplyJob {
        post_id,
        post_body: pwt.post.body,
        tasks: pwt.tasks,
        use_web_search,
    })
}

/// AI にリプライを書かせて保存し、最新の状態を返す。
/// AI の失敗はエラーにせず、リプライを failed にして返す。DB のロックは AI の応答待ちの間は持たない。
pub async fn generate(
    conn: &Mutex<Connection>,
    provider: &dyn LlmProvider,
    job: ReplyJob,
) -> AppResult<PostWithTasks> {
    let result = provider
        .reply(&job.post_body, &job.tasks, job.use_web_search)
        .await;

    let c = lock(conn)?;
    match &result {
        Ok(reply) => db::finish_ai_reply(&c, job.post_id, Ok((&reply.body, &reply.sources)))?,
        Err(e) => db::finish_ai_reply(&c, job.post_id, Err(&e.to_string()))?,
    }
    db::get_post_with_tasks(&c, job.post_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::mock::MockProvider;
    use crate::llm::AiReply;
    use crate::models::{ClassifiedTask, ReplySource, ReplyStatus};

    fn setup(body: &str, kinds: &[TaskKind]) -> (Mutex<Connection>, i64) {
        let mut conn = db::open_in_memory().unwrap();
        let id = db::insert_post(&conn, body).unwrap().id;
        let tasks: Vec<_> = kinds
            .iter()
            .map(|&kind| ClassifiedTask {
                kind,
                title: body.to_string(),
                target: String::new(),
                action: body.to_string(),
            })
            .collect();
        db::replace_tasks(&mut conn, id, &tasks).unwrap();
        db::set_classify_status(&conn, id, ClassifyStatus::Done, None).unwrap();
        (Mutex::new(conn), id)
    }

    fn ai_reply(body: &str, sources: Vec<ReplySource>) -> AiReply {
        AiReply {
            body: body.to_string(),
            sources,
        }
    }

    #[tokio::test]
    async fn reply_is_saved_as_done() {
        let (conn, id) = setup("ログイン画面のバリデーションを直す", &[TaskKind::Issue]);
        let provider = MockProvider::replying(ai_reply("方針です", vec![]));

        let job = start(&conn, &provider, id).unwrap();
        {
            let c = lock(&conn).unwrap();
            let r = &db::get_post_with_tasks(&c, id).unwrap().replies[0];
            assert_eq!(r.status, ReplyStatus::Pending);
            assert_eq!(r.model.as_deref(), Some("mock"));
        }
        let result = generate(&conn, &provider, job).await.unwrap();

        assert_eq!(result.replies.len(), 1);
        assert_eq!(result.replies[0].status, ReplyStatus::Done);
        assert_eq!(result.replies[0].body, "方針です");
    }

    #[tokio::test]
    async fn web_search_only_for_research() {
        let provider = MockProvider::replying(ai_reply("x", vec![]));

        let (conn, id) = setup("直す", &[TaskKind::Issue, TaskKind::Idea]);
        generate(&conn, &provider, start(&conn, &provider, id).unwrap())
            .await
            .unwrap();

        let (conn, id) = setup("調べる", &[TaskKind::Issue, TaskKind::Research]);
        generate(&conn, &provider, start(&conn, &provider, id).unwrap())
            .await
            .unwrap();

        assert_eq!(
            *provider.web_search_calls.lock().unwrap(),
            vec![false, true]
        );
    }

    #[tokio::test]
    async fn failure_marks_reply_failed_and_keeps_post_and_tasks() {
        let (conn, id) = setup("直す", &[TaskKind::Issue]);
        let provider = MockProvider::failing();

        let job = start(&conn, &provider, id).unwrap();
        let result = generate(&conn, &provider, job).await.unwrap();

        assert_eq!(result.replies[0].status, ReplyStatus::Failed);
        assert!(result.replies[0]
            .error
            .as_deref()
            .unwrap()
            .contains("API キー"));
        assert_eq!(result.post.body, "直す");
        assert_eq!(result.tasks.len(), 1);
    }

    #[tokio::test]
    async fn asking_again_replaces_previous_reply() {
        let (conn, id) = setup("直す", &[TaskKind::Issue]);
        let first = MockProvider::replying(ai_reply("1回目", vec![]));
        generate(&conn, &first, start(&conn, &first, id).unwrap())
            .await
            .unwrap();

        let second = MockProvider::replying(ai_reply("2回目", vec![]));
        let result = generate(&conn, &second, start(&conn, &second, id).unwrap())
            .await
            .unwrap();

        assert_eq!(result.replies.len(), 1);
        assert_eq!(result.replies[0].body, "2回目");
    }

    #[tokio::test]
    async fn posts_without_tasks_cannot_ask() {
        let provider = MockProvider::replying(ai_reply("x", vec![]));

        let (conn, note_only) = setup("今日は眠い", &[]);
        assert!(matches!(
            start(&conn, &provider, note_only),
            Err(AppError::Validation(_))
        ));

        let conn = Mutex::new(db::open_in_memory().unwrap());
        let pending = db::insert_post(&lock(&conn).unwrap(), "分類中").unwrap().id;
        assert!(matches!(
            start(&conn, &provider, pending),
            Err(AppError::Validation(_))
        ));
    }
}
