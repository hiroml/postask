use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

use crate::kind::TaskKind;

/// ポスト本文の最大文字数（前後の空白を除いて数える）。Q-006
pub const MAX_POST_CHARS: usize = 280;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassifyStatus {
    Pending,
    Done,
    Failed,
}

impl ClassifyStatus {
    fn as_str(self) -> &'static str {
        match self {
            ClassifyStatus::Pending => "pending",
            ClassifyStatus::Done => "done",
            ClassifyStatus::Failed => "failed",
        }
    }
}

impl ToSql for ClassifyStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for ClassifyStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "pending" => Ok(ClassifyStatus::Pending),
            "done" => Ok(ClassifyStatus::Done),
            "failed" => Ok(ClassifyStatus::Failed),
            other => Err(FromSqlError::Other(
                format!("未知の分類状態: {other}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Post {
    pub id: i64,
    pub body: String,
    pub created_at: String,
    pub classify_status: ClassifyStatus,
    pub classify_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Task {
    pub id: i64,
    pub post_id: i64,
    pub kind: TaskKind,
    pub title: String,
    pub target: String,
    pub action: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PostWithTasks {
    pub post: Post,
    pub tasks: Vec<Task>,
}

/// AI がポストから読み取ったタスク1件。`note` も含む。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifiedTask {
    pub kind: TaskKind,
    pub title: String,
    pub target: String,
    pub action: String,
}

/// ポスト本文の前後の空白を除き、1〜MAX_POST_CHARS 文字であることを確かめる。
pub fn validate_post_body(body: &str) -> Result<&str, String> {
    let trimmed = body.trim();
    let count = trimmed.chars().count();
    if count == 0 {
        return Err("ポストの本文を入力してください".to_string());
    }
    if count > MAX_POST_CHARS {
        return Err(format!(
            "ポストは{MAX_POST_CHARS}文字以内で入力してください（現在{count}文字）"
        ));
    }
    Ok(trimmed)
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_is_trimmed() {
        assert_eq!(validate_post_body("  READMEを書く \n"), Ok("READMEを書く"));
    }

    #[test]
    fn empty_or_whitespace_only_is_rejected() {
        assert!(validate_post_body("").is_err());
        assert!(validate_post_body(" \n\t　").is_err());
    }

    #[test]
    fn limit_counts_characters_not_bytes() {
        let ok = "あ".repeat(MAX_POST_CHARS);
        assert!(validate_post_body(&ok).is_ok());

        let too_long = "あ".repeat(MAX_POST_CHARS + 1);
        assert!(validate_post_body(&too_long).is_err());
    }
}
