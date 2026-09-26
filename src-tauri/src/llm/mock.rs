use std::sync::Mutex;

use async_trait::async_trait;

use super::{AiReply, LlmError, LlmProvider};
use crate::models::{ClassifiedTask, Task};

/// テスト用。決まった結果を返すか、決まったエラーで失敗する。
pub struct MockProvider {
    tasks: Option<Vec<ClassifiedTask>>,
    reply: Option<AiReply>,
    /// `reply` が呼ばれたときの `use_web_search` の記録
    pub web_search_calls: Mutex<Vec<bool>>,
}

impl MockProvider {
    fn new(tasks: Option<Vec<ClassifiedTask>>, reply: Option<AiReply>) -> Self {
        Self {
            tasks,
            reply,
            web_search_calls: Mutex::new(Vec::new()),
        }
    }

    /// 分類でこのタスクを返す
    pub fn tasks(tasks: Vec<ClassifiedTask>) -> Self {
        Self::new(Some(tasks), None)
    }

    /// リプライでこの内容を返す
    pub fn replying(reply: AiReply) -> Self {
        Self::new(None, Some(reply))
    }

    /// 分類もリプライも失敗する
    pub fn failing() -> Self {
        Self::new(None, None)
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn classify(&self, _post_body: &str) -> Result<Vec<ClassifiedTask>, LlmError> {
        self.tasks.clone().ok_or(LlmError::MissingApiKey)
    }

    fn reply_model(&self) -> String {
        "mock".to_string()
    }

    async fn reply(
        &self,
        _post_body: &str,
        _tasks: &[Task],
        use_web_search: bool,
    ) -> Result<AiReply, LlmError> {
        self.web_search_calls.lock().unwrap().push(use_web_search);
        self.reply.clone().ok_or(LlmError::MissingApiKey)
    }
}
