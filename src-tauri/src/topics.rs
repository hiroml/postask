//! 話題の作成と、既存のポストの振り分け直し（仕様008）。

use std::sync::Mutex;

use rusqlite::Connection;

use crate::classify::lock;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::llm::LlmProvider;
use crate::models::{
    validate_post_body, validate_topic_name, PostWithTasks, Topic, TopicBrief, MAX_POST_CHARS,
    MAX_TOPIC_NAME_CHARS,
};

/// 話題を作ったときに振り分け直す、直近のポストの件数（Q-030）
pub const MATCH_RECENT_POSTS: u32 = 100;

/// 作った話題と、話題として投稿したポスト（入力欄から作ったとき）
#[derive(Debug, Clone, serde::Serialize)]
pub struct CreatedTopic {
    pub topic: Topic,
    pub post: Option<PostWithTasks>,
}

/// 話題を作る（FR-1〜FR-3）。
/// `body` があれば「話題として投稿」（本文を概要にしてポストも保存）、無ければ話題ページから作る。
pub fn create(
    conn: &Mutex<Connection>,
    name: &str,
    description: &str,
    body: Option<&str>,
) -> AppResult<CreatedTopic> {
    let name = validate_topic_name(name).map_err(AppError::Validation)?;
    let body = body
        .map(|b| validate_post_body(b).map_err(AppError::Validation))
        .transpose()?;
    let description = match body {
        Some(b) if description.trim().is_empty() => b.to_string(),
        _ => description.trim().chars().take(MAX_POST_CHARS).collect(),
    };

    let mut c = lock(conn)?;
    // 名前が重複していたらポストも残さないよう、まとめて1つのトランザクションで行う
    let tx = c.transaction()?;
    let post = body.map(|b| db::insert_topic_post(&tx, b)).transpose()?;
    let topic = db::create_topic(&tx, name, &description, post.as_ref().map(|p| p.id))?;
    tx.commit()?;

    let post = post
        .map(|p| db::get_post_with_tasks(&c, p.id))
        .transpose()?;
    let topic = db::get_topic(&c, topic.id)?;
    Ok(CreatedTopic { topic, post })
}

/// 話題として投稿する本文から、話題の名前を提案してもらう（FR-2）。20文字に切り詰める
pub async fn suggest_name(provider: &dyn LlmProvider, body: &str) -> AppResult<String> {
    let body = validate_post_body(body).map_err(AppError::Validation)?;
    let name = provider
        .suggest_topic_name(body)
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(shorten_topic_name(&name))
}

/// 提案された名前を20文字に収める。長すぎるときは、括弧の補足を外してから切り詰める
fn shorten_topic_name(name: &str) -> String {
    let name = name.trim();
    let fits = |s: &str| s.chars().count() <= MAX_TOPIC_NAME_CHARS;
    if fits(name) {
        return name.to_string();
    }
    let without_paren = name.split(['（', '(']).next().unwrap_or(name).trim();
    if !without_paren.is_empty() && fits(without_paren) {
        return without_paren.to_string();
    }
    let base = if without_paren.is_empty() {
        name
    } else {
        without_paren
    };
    base.chars().take(MAX_TOPIC_NAME_CHARS).collect()
}

/// 新しい話題に、直近のポストを振り分け直す（FR-6）。話題が新しく付いたポストを返す。
pub async fn rematch(
    conn: &Mutex<Connection>,
    provider: &dyn LlmProvider,
    topic_id: i64,
) -> AppResult<Vec<PostWithTasks>> {
    let (topic, posts) = {
        let c = lock(conn)?;
        let t = db::get_topic(&c, topic_id)?;
        let brief = TopicBrief {
            id: t.id,
            name: t.name,
            description: t.description,
        };
        (
            brief,
            db::recent_posts_for_matching(&c, MATCH_RECENT_POSTS)?,
        )
    };
    if posts.is_empty() {
        return Ok(Vec::new());
    }

    let ids = provider
        .match_posts_to_topic(&topic, &posts)
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;

    let c = lock(conn)?;
    let mut updated = Vec::new();
    for id in ids {
        // 渡したポスト以外の id は捨てる
        if !posts.iter().any(|(pid, _)| *pid == id) {
            continue;
        }
        if db::add_ai_topic(&c, id, topic_id)? {
            updated.push(db::get_post_with_tasks(&c, id)?);
        }
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_post;
    use crate::kind::TaskKind;
    use crate::llm::mock::MockProvider;
    use crate::models::{ClassifiedTask, PostKind};

    fn conn() -> Mutex<Connection> {
        Mutex::new(db::open_in_memory().unwrap())
    }

    fn issue(title: &str) -> ClassifiedTask {
        ClassifiedTask {
            kind: TaskKind::Issue,
            title: title.to_string(),
            target: String::new(),
            action: title.to_string(),
        }
    }

    fn post(conn: &Mutex<Connection>, body: &str) -> i64 {
        db::insert_post(&lock(conn).unwrap(), body).unwrap().id
    }

    #[test]
    fn create_from_composer_saves_topic_post_without_classifying() {
        let conn = conn();
        let created = create(
            &conn,
            " postask の開発 ",
            "",
            Some("postask の開発: Tauri と Leptos"),
        )
        .unwrap();

        assert_eq!(created.topic.name, "postask の開発");
        assert_eq!(created.topic.description, "postask の開発: Tauri と Leptos");
        let post = created.post.unwrap();
        assert_eq!(post.post.kind, PostKind::Topic);
        assert_eq!(created.topic.source_post_id, Some(post.post.id));
        assert!(post.tasks.is_empty());
    }

    #[test]
    fn create_from_topics_page_has_no_post() {
        let conn = conn();
        let created = create(&conn, "家事", "家のこと", None).unwrap();
        assert!(created.post.is_none());
        assert_eq!(created.topic.source_post_id, None);
        let c = lock(&conn).unwrap();
        assert!(db::list_timeline(&c, 10, None).unwrap().is_empty());
    }

    #[test]
    fn duplicate_or_invalid_name_is_rejected_and_leaves_no_post() {
        let conn = conn();
        create(&conn, "家事", "", None).unwrap();
        assert!(matches!(
            create(&conn, "家事", "", Some("別の本文")),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            create(&conn, "  ", "", None),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            create(&conn, &"あ".repeat(21), "", None),
            Err(AppError::Validation(_))
        ));
        // 失敗したときに話題のポストが残らない
        let c = lock(&conn).unwrap();
        assert!(db::list_timeline(&c, 10, None).unwrap().is_empty());
    }

    #[tokio::test]
    async fn classification_assigns_known_topics_only_up_to_three() {
        let conn = conn();
        let ids: Vec<i64> = (1..=5)
            .map(|i| {
                create(&conn, &format!("話題{i}"), "", None)
                    .unwrap()
                    .topic
                    .id
            })
            .collect();
        let id = post(&conn, "ナビゲーターのページを増やす");
        // 知らない id（999）と重複は捨て、最大3個
        let provider = MockProvider::tasks(vec![issue("ページを増やす")])
            .with_topic_ids(vec![999, ids[0], ids[0], ids[1], ids[2], ids[3]]);

        let result = classify_post(&conn, &provider, id).await.unwrap();

        let got: Vec<i64> = result.topics.iter().map(|t| t.id).collect();
        assert_eq!(got.len(), 3);
        assert!(!got.contains(&999));
        assert_eq!(result.tasks.len(), 1);
    }

    #[tokio::test]
    async fn no_topics_means_no_topic_judgement() {
        let conn = conn();
        let id = post(&conn, "牛乳を買う");
        let provider = MockProvider::tasks(vec![issue("牛乳を買う")]);

        let result = classify_post(&conn, &provider, id).await.unwrap();

        assert!(result.topics.is_empty());
        assert_eq!(
            *provider.classify_topic_calls.lock().unwrap(),
            vec![Vec::<i64>::new()]
        );
    }

    #[tokio::test]
    async fn removed_topic_is_not_reassigned_on_reclassification() {
        let conn = conn();
        let topic = create(&conn, "postask の開発", "", None).unwrap().topic;
        let id = post(&conn, "ナビゲーターのページを増やす");
        let provider =
            MockProvider::tasks(vec![issue("ページを増やす")]).with_topic_ids(vec![topic.id]);
        classify_post(&conn, &provider, id).await.unwrap();

        let after_remove = db::remove_post_topic(&lock(&conn).unwrap(), id, topic.id).unwrap();
        assert!(after_remove.topics.is_empty());

        let again = classify_post(&conn, &provider, id).await.unwrap();
        assert!(again.topics.is_empty());
    }

    #[tokio::test]
    async fn topic_posts_are_not_classified() {
        let conn = conn();
        let created = create(&conn, "家事", "", Some("家のことをまとめる")).unwrap();
        let provider = MockProvider::tasks(vec![issue("まとめる")]);

        let result = classify_post(&conn, &provider, created.post.unwrap().post.id)
            .await
            .unwrap();

        assert!(result.tasks.is_empty());
        assert!(provider.classify_topic_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn rematch_adds_topic_to_matching_recent_posts() {
        let conn = conn();
        let a = post(&conn, "ナビゲーターのページを増やす");
        let b = post(&conn, "牛乳を買う");
        let topic = create(&conn, "postask の開発", "", Some("postask の開発")).unwrap();
        // 話題として投稿したポストや知らない id を返しても付けない
        let provider = MockProvider::failing().with_matches(vec![
            a,
            topic.post.as_ref().unwrap().post.id,
            999,
        ]);

        let updated = rematch(&conn, &provider, topic.topic.id).await.unwrap();

        assert_eq!(
            updated.iter().map(|p| p.post.id).collect::<Vec<_>>(),
            vec![a]
        );
        let c = lock(&conn).unwrap();
        assert_eq!(db::get_post_with_tasks(&c, a).unwrap().topics.len(), 1);
        assert!(db::get_post_with_tasks(&c, b).unwrap().topics.is_empty());
        assert_eq!(db::get_topic(&c, topic.topic.id).unwrap().post_count, 1);
    }

    #[test]
    fn long_name_drops_parenthetical_first() {
        assert_eq!(
            shorten_topic_name("postask の開発（Tauri と Leptos）"),
            "postask の開発"
        );
        assert_eq!(shorten_topic_name("  家事  "), "家事");
        assert_eq!(
            shorten_topic_name(&format!("（{}）", "あ".repeat(30)))
                .chars()
                .count(),
            MAX_TOPIC_NAME_CHARS
        );
    }

    #[tokio::test]
    async fn suggested_name_is_cut_to_twenty_chars() {
        let provider = MockProvider::failing().with_topic_name(&"長".repeat(30));
        let name = suggest_name(&provider, "本文").await.unwrap();
        assert_eq!(name.chars().count(), MAX_TOPIC_NAME_CHARS);
    }

    #[test]
    fn topic_counts_and_delete_keeps_posts() {
        let conn = conn();
        let topic = create(&conn, "postask の開発", "", None).unwrap().topic;
        let id = post(&conn, "READMEを書く");
        {
            let mut c = lock(&conn).unwrap();
            db::replace_tasks(&mut c, id, &[issue("READMEを書く"), issue("CIを設定する")]).unwrap();
            db::set_ai_topics(&mut c, id, &[topic.id]).unwrap();
            let t = db::get_topic(&c, topic.id).unwrap();
            assert_eq!((t.post_count, t.open_task_count), (1, 2));
        }
        let c = lock(&conn).unwrap();
        db::delete_topic(&c, topic.id).unwrap();
        let p = db::get_post_with_tasks(&c, id).unwrap();
        assert!(p.topics.is_empty());
        assert_eq!(p.tasks.len(), 2);
        assert!(db::list_topics(&c).unwrap().is_empty());
    }
}
