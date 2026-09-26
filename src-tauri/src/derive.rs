//! 関連タスクの候補（仕様009 FR-1b）。AI リプライをもとに、派生ポストにする文の候補を作る。

use std::sync::Mutex;

use rusqlite::Connection;

use crate::classify::lock;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::llm::LlmProvider;
use crate::models::{DerivedSuggestion, ReplyStatus, Task, MAX_POST_CHARS};

/// 候補の最大件数
pub const MAX_SUGGESTIONS: usize = 5;

/// AI の候補から、使えるものだけを残す。
/// 親がそのポストのタスクでないもの・空の文は捨て、文は280文字に切り詰め、5件までにする。
fn suggestions_to_keep(
    suggestions: Vec<DerivedSuggestion>,
    tasks: &[Task],
) -> Vec<DerivedSuggestion> {
    suggestions
        .into_iter()
        .filter(|s| tasks.iter().any(|t| t.id == s.task_id))
        .filter_map(|s| {
            let text: String = s.text.trim().chars().take(MAX_POST_CHARS).collect();
            (!text.trim().is_empty()).then_some(DerivedSuggestion {
                text: text.trim().to_string(),
                task_id: s.task_id,
            })
        })
        .take(MAX_SUGGESTIONS)
        .collect()
}

/// ポストの本文・タスク・AI リプライから、関連タスクの候補を作る。
/// AI のリプライが完了していないポスト・タスクの無いポストはエラー。
pub async fn suggest(
    conn: &Mutex<Connection>,
    provider: &dyn LlmProvider,
    post_id: i64,
) -> AppResult<Vec<DerivedSuggestion>> {
    let post = db::get_post_with_tasks(&*lock(conn)?, post_id)?;
    let reply = post
        .replies
        .iter()
        .find(|r| r.author == db::AI_AUTHOR && r.status == ReplyStatus::Done)
        .ok_or_else(|| {
            AppError::Validation("AI のリプライが無いポストでは候補を作れません".to_string())
        })?;
    if post.tasks.is_empty() {
        return Err(AppError::Validation(
            "タスクの無いポストでは候補を作れません".to_string(),
        ));
    }

    let suggestions = provider
        .suggest_derived_tasks(&post.post.body, &post.tasks, &reply.body)
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(suggestions_to_keep(suggestions, &post.tasks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::TaskKind;
    use crate::llm::mock::MockProvider;
    use crate::models::{ClassifiedTask, ClassifyStatus};

    /// タスク2件と、完了した AI リプライのあるポスト
    fn setup(with_reply: bool) -> (Mutex<Connection>, i64, Vec<i64>) {
        let mut conn = db::open_in_memory().unwrap();
        let id = db::insert_post(&conn, "ログイン画面のバリデーションを直す、あとCIも")
            .unwrap()
            .id;
        let task = |title: &str| ClassifiedTask {
            kind: TaskKind::Issue,
            title: title.to_string(),
            target: String::new(),
            action: title.to_string(),
        };
        db::replace_tasks(
            &mut conn,
            id,
            &[task("バリデーションを直す"), task("CIを設定する")],
        )
        .unwrap();
        db::set_classify_status(&conn, id, ClassifyStatus::Done, None).unwrap();
        if with_reply {
            db::start_ai_reply(&mut conn, id, "mock").unwrap();
            db::finish_ai_reply(&conn, id, Ok(("まず今の仕様を確認しましょう", &[]))).unwrap();
        }
        let ids = db::get_post_with_tasks(&conn, id)
            .unwrap()
            .tasks
            .iter()
            .map(|t| t.id)
            .collect();
        (Mutex::new(conn), id, ids)
    }

    fn s(text: &str, task_id: i64) -> DerivedSuggestion {
        DerivedSuggestion {
            text: text.to_string(),
            task_id,
        }
    }

    #[tokio::test]
    async fn suggestions_are_limited_to_the_posts_tasks() {
        let (conn, id, tasks) = setup(true);
        let provider = MockProvider::default().with_derived(vec![
            s("今のバリデーションの仕様を確認する", tasks[0]),
            s("よそのタスクに付いた候補", 9999),
            s("  ", tasks[0]),
            s(&"長".repeat(300), tasks[1]),
        ]);
        let got = suggest(&conn, &provider, id).await.unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], s("今のバリデーションの仕様を確認する", tasks[0]));
        assert_eq!(got[1].task_id, tasks[1]);
        assert_eq!(got[1].text.chars().count(), MAX_POST_CHARS);
    }

    #[tokio::test]
    async fn at_most_five_suggestions() {
        let (conn, id, tasks) = setup(true);
        let many = (0..8).map(|i| s(&format!("候補{i}"), tasks[0])).collect();
        let provider = MockProvider::default().with_derived(many);
        assert_eq!(
            suggest(&conn, &provider, id).await.unwrap().len(),
            MAX_SUGGESTIONS
        );
    }

    #[tokio::test]
    async fn post_without_ai_reply_is_an_error() {
        let (conn, id, tasks) = setup(false);
        let provider = MockProvider::default().with_derived(vec![s("x", tasks[0])]);
        assert!(matches!(
            suggest(&conn, &provider, id).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn ai_failure_is_an_error() {
        let (conn, id, _) = setup(true);
        assert!(matches!(
            suggest(&conn, &MockProvider::failing(), id).await,
            Err(AppError::Other(_))
        ));
    }

    #[tokio::test]
    async fn missing_post_is_not_found() {
        let (conn, _, _) = setup(true);
        assert!(matches!(
            suggest(&conn, &MockProvider::failing(), 999).await,
            Err(AppError::PostNotFound(999))
        ));
    }
}
