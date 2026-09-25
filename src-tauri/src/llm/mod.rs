//! LLM プロバイダの抽象化（ADR-002）。Claude 以外（ローカル LLM など）もこのトレイトを実装して差し替える。

pub mod claude;
#[cfg(test)]
pub mod mock;

use async_trait::async_trait;

use crate::models::{ClassifiedTask, ReplySource, Task};

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API キーが設定されていません（.env の ANTHROPIC_API_KEY を確認してください）")]
    MissingApiKey,

    #[error("AI との通信に失敗しました: {0}")]
    Network(String),

    #[error("AI の API がエラーを返しました（HTTP {status}）: {message}")]
    Api { status: u16, message: String },

    #[error("AI が回答を断りました")]
    Refused,

    #[error("AI の応答が途中で終わりました（stop_reason: {0}）")]
    Incomplete(String),

    #[error("AI の応答を読み取れませんでした: {0}")]
    InvalidResponse(String),
}

/// AI のリプライ（仕様003）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiReply {
    pub body: String,
    /// Web 検索を使ったときの出典（重複なし）
    pub sources: Vec<ReplySource>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// ポスト本文を読み、含まれるタスクを返す（`note` も含めて返してよい）。
    async fn classify(&self, post_body: &str) -> Result<Vec<ClassifiedTask>, LlmError>;

    /// リプライに使うモデル名（記録用）。
    fn reply_model(&self) -> String;

    /// ポストとそのタスクに対して、助言のリプライを書く（仕様003）。
    /// `use_web_search` が true でも、検索に対応しないプロバイダは検索なしで答えてよい。
    async fn reply(
        &self,
        post_body: &str,
        tasks: &[Task],
        use_web_search: bool,
    ) -> Result<AiReply, LlmError>;
}
