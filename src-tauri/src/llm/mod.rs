//! LLM プロバイダの抽象化（ADR-002）。Claude 以外（ローカル LLM など）もこのトレイトを実装して差し替える。

pub mod claude;
#[cfg(test)]
pub mod mock;

use async_trait::async_trait;

use crate::models::ClassifiedTask;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API キーが設定されていません（.env の ANTHROPIC_API_KEY を確認してください）")]
    MissingApiKey,

    #[error("AI との通信に失敗しました: {0}")]
    Network(String),

    #[error("AI の API がエラーを返しました（HTTP {status}）: {message}")]
    Api { status: u16, message: String },

    #[error("AI が分類を断りました")]
    Refused,

    #[error("AI の応答が途中で終わりました（stop_reason: {0}）")]
    Incomplete(String),

    #[error("AI の応答を読み取れませんでした: {0}")]
    InvalidResponse(String),
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// ポスト本文を読み、含まれるタスクを返す（`note` も含めて返してよい）。
    async fn classify(&self, post_body: &str) -> Result<Vec<ClassifiedTask>, LlmError>;
}
