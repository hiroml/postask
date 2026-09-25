//! フロントから `invoke` で呼ぶ Tauri コマンド。

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

use crate::classify::{self, lock};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::kind::{KindInfo, KINDS};
use crate::llm::LlmProvider;
use crate::models::{validate_post_body, ClassifyStatus, Post, PostWithTasks, TaskStatus};
use crate::reply;

/// ポストの状態が裏の処理で変わったとき（分類・AI リプライの完了。成功・失敗とも）に送るイベント。
/// payload は `PostWithTasks`（仕様003 で `post-classified` から改名）。
pub const POST_UPDATED_EVENT: &str = "post-updated";

fn emit_post_updated(app: &AppHandle, result: PostWithTasks) {
    if let Err(e) = app.emit(POST_UPDATED_EVENT, result) {
        eprintln!("イベントの送信に失敗しました: {e}");
    }
}

const MAX_TIMELINE_LIMIT: u32 = 200;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub llm: Arc<dyn LlmProvider>,
}

fn create_post_in_db(conn: &Mutex<Connection>, body: &str) -> AppResult<Post> {
    let body = validate_post_body(body).map_err(AppError::Validation)?;
    let c = lock(conn)?;
    db::insert_post(&c, body)
}

/// 裏で分類を実行し、終わったらイベントでフロントに知らせる。
fn spawn_classification(app: AppHandle, state: &AppState, post_id: i64) {
    let db = Arc::clone(&state.db);
    let llm = Arc::clone(&state.llm);
    tauri::async_runtime::spawn(async move {
        match classify::classify_post(&db, llm.as_ref(), post_id).await {
            Ok(result) => emit_post_updated(&app, result),
            Err(e) => eprintln!("ポスト {post_id} の分類結果を保存できませんでした: {e}"),
        }
    });
}

/// ポストを保存してすぐ返す。分類は裏で始める（FR-2, FR-3）。
#[tauri::command(rename_all = "snake_case")]
pub fn create_post(app: AppHandle, state: State<'_, AppState>, body: String) -> AppResult<Post> {
    let post = create_post_in_db(&state.db, &body)?;
    spawn_classification(app, &state, post.id);
    Ok(post)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_timeline(
    state: State<'_, AppState>,
    limit: u32,
    before_id: Option<i64>,
) -> AppResult<Vec<PostWithTasks>> {
    let c = lock(&state.db)?;
    db::list_timeline(&c, limit.clamp(1, MAX_TIMELINE_LIMIT), before_id)
}

/// 分類をやり直す（FR-6）。状態をすぐ pending にしてから裏で分類する。
#[tauri::command(rename_all = "snake_case")]
pub fn retry_classification(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: i64,
) -> AppResult<()> {
    {
        let c = lock(&state.db)?;
        db::set_classify_status(&c, post_id, ClassifyStatus::Pending, None)?;
    }
    spawn_classification(app, &state, post_id);
    Ok(())
}

/// タスクを完了・未完了にする（仕様002）。画面の更新用に、そのタスクを含むポスト全体を返す。
#[tauri::command(rename_all = "snake_case")]
pub fn set_task_status(
    state: State<'_, AppState>,
    task_id: i64,
    status: TaskStatus,
) -> AppResult<PostWithTasks> {
    let c = lock(&state.db)?;
    db::set_task_status(&c, task_id, status)
}

/// AI にリプライを頼む（仕様003）。聞き直し・再試行も同じ。
/// 受け付けたらリプライを「生成中」にしてすぐ返し、生成は裏で行う。終わったら `post-updated` で知らせる。
#[tauri::command(rename_all = "snake_case")]
pub fn request_ai_reply(app: AppHandle, state: State<'_, AppState>, post_id: i64) -> AppResult<()> {
    let job = reply::start(&state.db, state.llm.as_ref(), post_id)?;
    let db = Arc::clone(&state.db);
    let llm = Arc::clone(&state.llm);
    tauri::async_runtime::spawn(async move {
        match reply::generate(&db, llm.as_ref(), job).await {
            Ok(result) => emit_post_updated(&app, result),
            Err(e) => eprintln!("ポスト {post_id} のリプライを保存できませんでした: {e}"),
        }
    });
    Ok(())
}

/// 種別の表示名と色（kind.rs の定義）を返す。
#[tauri::command]
pub fn list_kinds() -> Vec<KindInfo> {
    KINDS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MAX_POST_CHARS;

    fn conn() -> Mutex<Connection> {
        Mutex::new(db::open_in_memory().unwrap())
    }

    #[test]
    fn create_post_trims_and_saves_pending() {
        let conn = conn();
        let post = create_post_in_db(&conn, "  READMEを書く  ").unwrap();
        assert_eq!(post.body, "READMEを書く");
        assert_eq!(post.classify_status, ClassifyStatus::Pending);
    }

    #[test]
    fn create_post_rejects_empty_and_too_long() {
        let conn = conn();
        assert!(matches!(
            create_post_in_db(&conn, "   "),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            create_post_in_db(&conn, &"a".repeat(MAX_POST_CHARS + 1)),
            Err(AppError::Validation(_))
        ));
        assert!(create_post_in_db(&conn, &"a".repeat(MAX_POST_CHARS)).is_ok());

        let c = lock(&conn).unwrap();
        assert_eq!(db::list_timeline(&c, 10, None).unwrap().len(), 1);
    }

    #[test]
    fn kinds_are_serialized_without_ai_description() {
        let json = serde_json::to_value(list_kinds()).unwrap();
        assert_eq!(json[0]["kind"], "issue");
        assert_eq!(json[0]["label"], "課題");
        assert!(json[0].get("description").is_none());
    }
}
