//! バックエンド（src-tauri/src/models.rs, kind.rs）と同じ JSON 形の型。

use serde::{Deserialize, Serialize};

/// バックエンドの `MAX_POST_CHARS` と同じ値にする（Q-006）
pub const MAX_POST_CHARS: usize = 280;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassifyStatus {
    Pending,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Post {
    pub id: i64,
    pub body: String,
    pub created_at: String,
    pub classify_status: ClassifyStatus,
    pub classify_error: Option<String>,
    pub kind: PostKind,
    #[serde(default)]
    pub parent_task_id: Option<i64>,
}

/// ポストの種類（仕様008）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostKind {
    Post,
    /// 話題として投稿したもの
    Topic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Done,
}

impl TaskStatus {
    pub fn toggled(self) -> Self {
        match self {
            TaskStatus::Open => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Open,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Task {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub status: TaskStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub children: ChildCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyStatus {
    Pending,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReplySource {
    pub title: String,
    pub url: String,
}

/// ポストへのリプライ（仕様003）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Reply {
    pub author: String,
    pub body: String,
    pub sources: Vec<ReplySource>,
    pub status: ReplyStatus,
    pub error: Option<String>,
}

/// AI のリプライの `author`（バックエンドの db::AI_AUTHOR と同じ）
pub const AI_AUTHOR: &str = "ai";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PostWithTasks {
    pub post: Post,
    pub tasks: Vec<Task>,
    pub replies: Vec<Reply>,
    pub topics: Vec<TopicRef>,
    #[serde(default)]
    pub parent: Option<ParentRef>,
}

/// ポストに付いた話題（仕様008）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TopicRef {
    pub id: i64,
    pub name: String,
}

/// 話題（一覧用、仕様008）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Topic {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub post_count: u32,
    pub open_task_count: u32,
}

/// 話題を作った結果（入力欄から作ったときは話題のポストも付く）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CreatedTopic {
    pub topic: Topic,
    pub post: Option<PostWithTasks>,
}

/// 話題の名前の最大文字数（バックエンドと同じ）
pub const MAX_TOPIC_NAME_CHARS: usize = 20;

impl PostWithTasks {
    pub fn ai_reply(&self) -> Option<&Reply> {
        self.replies.iter().find(|r| r.author == AI_AUTHOR)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KindInfo {
    pub kind: String,
    pub label: String,
    pub color: String,
    pub creates_task: bool,
}

/// 課題ページの集計（仕様006。バックエンドの dashboard.rs と同じ形）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskDashboard {
    pub totals: Totals,
    pub by_kind: Vec<KindCount>,
    pub trend: Trend,
    pub tasks: Vec<TaskRow>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Totals {
    pub open: u32,
    pub done: u32,
    pub completion_rate: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KindCount {
    pub kind: String,
    pub open: u32,
    pub done: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Day,
    Week,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Trend {
    pub bucket: Bucket,
    pub points: Vec<TrendPoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TrendPoint {
    pub start: String,
    pub created: u32,
    pub completed: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TaskRow {
    pub task: Task,
    pub post_body: String,
    #[serde(default)]
    pub parent: Option<ParentRef>,
}

/// アプリの情報（仕様004）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AppInfo {
    pub dev: bool,
    pub db_path: String,
}

/// 前後の空白を除いた文字数（バックエンドの検証と同じ数え方）
pub fn post_length(body: &str) -> usize {
    body.trim().chars().count()
}

/// タスクの子（派生したタスク）の数と完了数（仕様009）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct ChildCount {
    pub total: u32,
    pub done: u32,
}

impl ChildCount {
    /// 子があって、すべて完了している
    pub fn all_done(self) -> bool {
        self.total > 0 && self.done == self.total
    }
}

/// 派生ポスト・派生タスクの元のタスク（仕様009）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ParentRef {
    pub task_id: i64,
    pub title: String,
}

/// AI が出した関連タスクの候補（仕様009）
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DerivedSuggestion {
    pub text: String,
    pub task_id: i64,
}
