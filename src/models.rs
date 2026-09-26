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
    pub completed_at: Option<String>,
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
}

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
