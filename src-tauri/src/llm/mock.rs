use async_trait::async_trait;

use super::{LlmError, LlmProvider};
use crate::models::ClassifiedTask;

/// テスト用。決まったタスクを返すか、決まったエラーで失敗する。
pub enum MockProvider {
    Tasks(Vec<ClassifiedTask>),
    Fail,
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn classify(&self, _post_body: &str) -> Result<Vec<ClassifiedTask>, LlmError> {
        match self {
            MockProvider::Tasks(tasks) => Ok(tasks.clone()),
            MockProvider::Fail => Err(LlmError::MissingApiKey),
        }
    }
}
