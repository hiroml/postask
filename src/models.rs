//! バックエンド（src-tauri/src/models.rs, kind.rs）と同じ JSON 形の型。

use serde::Deserialize;

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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Task {
    pub id: i64,
    pub kind: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PostWithTasks {
    pub post: Post,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KindInfo {
    pub kind: String,
    pub label: String,
    pub color: String,
    pub creates_task: bool,
}

/// 前後の空白を除いた文字数（バックエンドの検証と同じ数え方）
pub fn post_length(body: &str) -> usize {
    body.trim().chars().count()
}
