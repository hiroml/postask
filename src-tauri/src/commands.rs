//! フロントから `invoke` で呼ぶ Tauri コマンド。

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, State};

use crate::classify::{self, lock};
use crate::dashboard::{self, Period, TaskDashboard};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::kind::{KindInfo, TaskKind, KINDS};
use crate::llm::LlmProvider;
use crate::models::{
    validate_post_body, validate_topic_name, ClassifyStatus, Post, PostWithTasks, TaskStatus,
    Topic, MAX_POST_CHARS,
};
use crate::reply;
use crate::topics::{self, CreatedTopic};

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
    pub info: AppInfo,
}

/// アプリの情報（仕様004）。開発版かどうかを画面に出すために使う
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppInfo {
    pub dev: bool,
    pub db_path: String,
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

// ---- 話題（仕様008） ----

/// 話題として投稿する本文から、話題の名前を提案する（FR-2）
#[tauri::command(rename_all = "snake_case")]
pub async fn suggest_topic_name(state: State<'_, AppState>, body: String) -> AppResult<String> {
    let llm = Arc::clone(&state.llm);
    topics::suggest_name(llm.as_ref(), &body).await
}

/// 話題を作る（FR-1〜FR-3）。`body` があれば「話題として投稿」、無ければ話題ページから。
/// 作ったあと、直近のポストの振り分け直しを裏で行い、話題が付いたポストを `post-updated` で知らせる（FR-6）。
#[tauri::command(rename_all = "snake_case")]
pub fn create_topic(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    description: String,
    body: Option<String>,
) -> AppResult<CreatedTopic> {
    let created = topics::create(&state.db, &name, &description, body.as_deref())?;
    let topic_id = created.topic.id;
    let db = Arc::clone(&state.db);
    let llm = Arc::clone(&state.llm);
    tauri::async_runtime::spawn(async move {
        match topics::rematch(&db, llm.as_ref(), topic_id).await {
            Ok(updated) => updated.into_iter().for_each(|p| emit_post_updated(&app, p)),
            Err(e) => eprintln!("話題 {topic_id} の振り分け直しに失敗しました: {e}"),
        }
    });
    Ok(created)
}

#[tauri::command]
pub fn list_topics(state: State<'_, AppState>) -> AppResult<Vec<Topic>> {
    let c = lock(&state.db)?;
    db::list_topics(&c)
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_topic(
    state: State<'_, AppState>,
    topic_id: i64,
    name: String,
    description: String,
) -> AppResult<Topic> {
    let name = validate_topic_name(&name).map_err(AppError::Validation)?;
    let description: String = description.trim().chars().take(MAX_POST_CHARS).collect();
    let c = lock(&state.db)?;
    db::update_topic(&c, topic_id, name, &description)
}

/// 話題を削除する。ポストとタスクは残る（FR-11）
#[tauri::command(rename_all = "snake_case")]
pub fn delete_topic(state: State<'_, AppState>, topic_id: i64) -> AppResult<()> {
    let c = lock(&state.db)?;
    db::delete_topic(&c, topic_id)
}

/// ポストから話題を外す（FR-9）。外した話題は AI が付け直さない
#[tauri::command(rename_all = "snake_case")]
pub fn remove_post_topic(
    state: State<'_, AppState>,
    post_id: i64,
    topic_id: i64,
) -> AppResult<PostWithTasks> {
    let c = lock(&state.db)?;
    db::remove_post_topic(&c, post_id, topic_id)
}

/// 課題ページの集計（仕様006）。期間・種別・状態で絞り込む（None = すべて）。
#[tauri::command(rename_all = "snake_case")]
pub fn get_task_dashboard(
    state: State<'_, AppState>,
    period: Period,
    kind: Option<TaskKind>,
    status: Option<TaskStatus>,
) -> AppResult<TaskDashboard> {
    let c = lock(&state.db)?;
    dashboard::get_task_dashboard(&c, period, kind, status)
}

/// 種別の表示名と色（kind.rs の定義）を返す。
#[tauri::command]
pub fn list_kinds() -> Vec<KindInfo> {
    KINDS.to_vec()
}

/// 開発版かどうかと、使っている DB のパスを返す（仕様004）。
#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    state.info.clone()
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
