use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

use crate::kind::TaskKind;

/// ポスト本文の最大文字数（前後の空白を除いて数える）。Q-006
pub const MAX_POST_CHARS: usize = 280;

/// DB に文字列として保存する enum に、`as_str` と rusqlite の `ToSql` / `FromSql` を実装する。
macro_rules! sql_text_enum {
    ($ty:ident, $what:literal, { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $ty {
            pub fn as_str(self) -> &'static str {
                match self {
                    $($ty::$variant => $text),+
                }
            }
        }

        impl ToSql for $ty {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $ty {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                match value.as_str()? {
                    $($text => Ok($ty::$variant),)+
                    other => Err(FromSqlError::Other(
                        format!("未知の{}: {other}", $what).into(),
                    )),
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassifyStatus {
    Pending,
    Done,
    Failed,
}

sql_text_enum!(ClassifyStatus, "分類状態", {
    Pending => "pending",
    Done => "done",
    Failed => "failed",
});

/// タスクの状態（仕様002）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Done,
}

sql_text_enum!(TaskStatus, "タスクの状態", {
    Open => "open",
    Done => "done",
});

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Post {
    pub id: i64,
    pub body: String,
    pub created_at: String,
    pub classify_status: ClassifyStatus,
    pub classify_error: Option<String>,
    /// 普段のポストか、話題として投稿したものか（仕様008）
    pub kind: PostKind,
    /// 派生ポストのとき、元のタスクの id（仕様009）
    pub parent_task_id: Option<i64>,
}

/// ポストの種類（仕様008）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostKind {
    /// 普段のポスト（タスクに分類する）
    Post,
    /// 話題として投稿したもの（本文が話題の概要。タスクには分類しない）
    Topic,
}

sql_text_enum!(PostKind, "ポストの種類", {
    Post => "post",
    Topic => "topic",
});

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Task {
    pub id: i64,
    pub post_id: i64,
    pub kind: TaskKind,
    pub title: String,
    pub target: String,
    pub action: String,
    pub status: TaskStatus,
    pub created_at: String,
    /// 完了日時（未完了なら None）。仕様002
    pub completed_at: Option<String>,
    /// このタスクから派生したタスク（子）の数と完了数（仕様009）
    pub children: ChildCount,
}

/// 子のタスクの数（仕様009 FR-5）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ChildCount {
    pub total: u32,
    pub done: u32,
}

/// 派生ポストの元のタスク（仕様009 FR-3）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParentRef {
    pub task_id: i64,
    pub title: String,
}

/// AI が出した関連タスクの候補（仕様009 FR-1b）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedSuggestion {
    pub text: String,
    /// 親にするタスク（そのポストのタスクのどれか）
    pub task_id: i64,
}

/// リプライの状態（仕様003）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyStatus {
    Pending,
    Done,
    Failed,
}

sql_text_enum!(ReplyStatus, "リプライの状態", {
    Pending => "pending",
    Done => "done",
    Failed => "failed",
});

/// リプライの出典（Web 検索の結果のページ）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplySource {
    pub title: String,
    pub url: String,
}

/// ポストへのリプライ（仕様003）。今は AI のみ。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reply {
    pub id: i64,
    pub post_id: i64,
    /// 書いた人。AI は `"ai"`（将来 他人のリプライを入れられるよう文字列にしている）
    pub author: String,
    pub body: String,
    pub sources: Vec<ReplySource>,
    pub model: Option<String>,
    pub status: ReplyStatus,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PostWithTasks {
    pub post: Post,
    pub tasks: Vec<Task>,
    pub replies: Vec<Reply>,
    /// ポストに付いた話題（外したものは含まない。仕様008）
    pub topics: Vec<TopicRef>,
    /// 派生ポストのとき、元のタスク（仕様009）
    pub parent: Option<ParentRef>,
}

/// 話題の名前の最大文字数（仕様008 FR-1）
pub const MAX_TOPIC_NAME_CHARS: usize = 20;
/// 1つのポストに付ける話題の最大数（仕様008 FR-4）
pub const MAX_TOPICS_PER_POST: usize = 3;

/// 話題（一覧用に件数つき。仕様008）
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Topic {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub source_post_id: Option<i64>,
    pub created_at: String,
    pub post_count: u32,
    pub open_task_count: u32,
}

/// ポストに付いた話題（チップの表示用）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TopicRef {
    pub id: i64,
    pub name: String,
}

/// AI に渡す話題（分類・振り分け直し用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicBrief {
    pub id: i64,
    pub name: String,
    pub description: String,
}

/// AI の分類結果: タスクと、当てはまる話題の id
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Classification {
    pub tasks: Vec<ClassifiedTask>,
    pub topic_ids: Vec<i64>,
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

/// 話題の名前の前後の空白を除き、1〜MAX_TOPIC_NAME_CHARS 文字であることを確かめる（仕様008 FR-1）。
pub fn validate_topic_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    if count == 0 {
        return Err("話題の名前を入力してください".to_string());
    }
    if count > MAX_TOPIC_NAME_CHARS {
        return Err(format!(
            "話題の名前は{MAX_TOPIC_NAME_CHARS}文字以内で入力してください（現在{count}文字）"
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
