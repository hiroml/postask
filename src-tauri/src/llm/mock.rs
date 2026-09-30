use std::sync::Mutex;

use async_trait::async_trait;

use super::{AiReply, LlmError, LlmProvider};
use crate::models::{Classification, ClassifiedTask, DerivedSuggestion, Task, TopicBrief};

/// テスト用。決まった結果を返すか、決まったエラーで失敗する。
#[derive(Default)]
pub struct MockProvider {
    tasks: Option<Vec<ClassifiedTask>>,
    topic_ids: Vec<i64>,
    reply: Option<AiReply>,
    topic_name: Option<String>,
    matched_post_ids: Option<Vec<i64>>,
    derived: Option<Vec<DerivedSuggestion>>,
    /// `reply` が呼ばれたときの `use_web_search` の記録
    pub web_search_calls: Mutex<Vec<bool>>,
    /// `classify` に渡された話題の id の記録
    pub classify_topic_calls: Mutex<Vec<Vec<i64>>>,
}

impl MockProvider {
    /// 分類でこのタスクを返す
    pub fn tasks(tasks: Vec<ClassifiedTask>) -> Self {
        Self {
            tasks: Some(tasks),
            ..Default::default()
        }
    }

    /// リプライでこの内容を返す
    pub fn replying(reply: AiReply) -> Self {
        Self {
            reply: Some(reply),
            ..Default::default()
        }
    }

    /// すべて失敗する
    pub fn failing() -> Self {
        Self::default()
    }

    /// 分類で、この話題の id も返す
    pub fn with_topic_ids(mut self, ids: Vec<i64>) -> Self {
        self.topic_ids = ids;
        self
    }

    /// 話題の名前の提案でこの名前を返す
    pub fn with_topic_name(mut self, name: &str) -> Self {
        self.topic_name = Some(name.to_string());
        self
    }

    /// 振り分け直しでこのポストの id を返す
    pub fn with_matches(mut self, ids: Vec<i64>) -> Self {
        self.matched_post_ids = Some(ids);
        self
    }

    /// 関連タスクの候補でこれを返す
    pub fn with_derived(mut self, suggestions: Vec<DerivedSuggestion>) -> Self {
        self.derived = Some(suggestions);
        self
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn classify(
        &self,
        _post_body: &str,
        topics: &[TopicBrief],
    ) -> Result<Classification, LlmError> {
        self.classify_topic_calls
            .lock()
            .unwrap()
            .push(topics.iter().map(|t| t.id).collect());
        let tasks = self.tasks.clone().ok_or(LlmError::MissingApiKey)?;
        Ok(Classification {
            tasks,
            topic_ids: self.topic_ids.clone(),
        })
    }

    async fn suggest_topic_name(&self, _body: &str) -> Result<String, LlmError> {
        self.topic_name.clone().ok_or(LlmError::MissingApiKey)
    }

    async fn match_posts_to_topic(
        &self,
        _topic: &TopicBrief,
        _posts: &[(i64, String)],
    ) -> Result<Vec<i64>, LlmError> {
        self.matched_post_ids.clone().ok_or(LlmError::MissingApiKey)
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

    async fn suggest_derived_tasks(
        &self,
        _post_body: &str,
        _tasks: &[Task],
        _reply_body: &str,
    ) -> Result<Vec<DerivedSuggestion>, LlmError> {
        self.derived.clone().ok_or(LlmError::MissingApiKey)
    }
}
