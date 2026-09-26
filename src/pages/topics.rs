//! 話題ページ: 話題の一覧・作成・編集・削除（仕様008 FR-10, FR-11）。
//! 話題を押したときのプレビュー（ポストの一覧など）は B-018 で作る。

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::de::IgnoredAny;
use serde::Serialize;

use crate::models::{CreatedTopic, Topic, MAX_POST_CHARS, MAX_TOPIC_NAME_CHARS};
use crate::store::use_timeline_store;
use crate::tauri::{invoke, NoArgs};

#[derive(Serialize)]
struct CreateTopicArgs {
    name: String,
    description: String,
    body: Option<String>,
}

#[derive(Serialize)]
struct UpdateTopicArgs {
    topic_id: i64,
    name: String,
    description: String,
}

#[derive(Serialize)]
struct TopicIdArgs {
    topic_id: i64,
}

fn name_ok(name: &str) -> bool {
    let n = name.trim().chars().count();
    n > 0 && n <= MAX_TOPIC_NAME_CHARS
}

#[component]
pub fn TopicsPage() -> impl IntoView {
    let store = use_timeline_store();
    let topics = RwSignal::new(None::<Vec<Topic>>);
    let error = RwSignal::new(None::<String>);

    let reload = move || {
        spawn_local(async move {
            match invoke::<_, Vec<Topic>>("list_topics", &NoArgs {}).await {
                Ok(list) => {
                    topics.set(Some(list));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // タイムラインが変わったら取り直す（振り分け直しの結果で件数が変わる）
    Effect::new(move |_| {
        store.posts.track();
        reload();
    });

    // 話題を作る（話題ページから。ポストはしない、Q-028）
    let new_name = RwSignal::new(String::new());
    let new_description = RwSignal::new(String::new());
    let creating = RwSignal::new(false);
    let create = move || {
        let args = CreateTopicArgs {
            name: new_name.get_untracked(),
            description: new_description.get_untracked(),
            body: None,
        };
        creating.set(true);
        spawn_local(async move {
            match invoke::<_, CreatedTopic>("create_topic", &args).await {
                Ok(created) => {
                    new_name.set(String::new());
                    new_description.set(String::new());
                    store.on_topic_created.run(created);
                    reload();
                }
                Err(e) => error.set(Some(e)),
            }
            creating.set(false);
        });
    };
    let can_create = move || name_ok(&new_name.get()) && !creating.get();

    view! {
        <section class="feed-panel topics-page">
            <header class="feed-header">
                <h1>"話題"</h1>
            </header>

            <form
                class="topic-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    if can_create() {
                        create();
                    }
                }
            >
                <p class="topic-form-title">"話題を作る"</p>
                <input
                    type="text"
                    placeholder=format!("名前（{MAX_TOPIC_NAME_CHARS}文字まで）")
                    maxlength=MAX_TOPIC_NAME_CHARS
                    prop:value=move || new_name.get()
                    on:input=move |ev| new_name.set(event_target_value(&ev))
                />
                <textarea
                    placeholder="概要（どんなテーマか。AI が振り分けに使う）"
                    maxlength=MAX_POST_CHARS
                    prop:value=move || new_description.get()
                    on:input=move |ev| new_description.set(event_target_value(&ev))
                ></textarea>
                <div class="topic-form-actions">
                    <span class="hint">"作ると、直近100件のポストから当てはまるものに話題が付きます"</span>
                    <button type="submit" class="post-button" disabled=move || !can_create()>"作成"</button>
                </div>
            </form>

            <Show when=move || error.get().is_some()>
                <p class="error-text banner">{move || error.get().unwrap_or_default()}</p>
            </Show>

            {move || match topics.get() {
                None => view! { <p class="empty">"読み込み中…"</p> }.into_any(),
                Some(list) if list.is_empty() => view! {
                    <p class="empty">"まだ話題がありません。上のフォームか、ホームの「# 話題として投稿」から作れます。"</p>
                }
                .into_any(),
                Some(list) => view! {
                    <ul class="topic-list">
                        {list.into_iter().map(|t| view! { <TopicRow topic=t on_changed=Callback::new(move |_| reload()) error=error /> }).collect_view()}
                    </ul>
                }
                .into_any(),
            }}
        </section>
    }
}

/// 話題の1行: 表示・編集・削除（削除は2回押して確定）
#[component]
fn TopicRow(
    topic: Topic,
    on_changed: Callback<()>,
    error: RwSignal<Option<String>>,
) -> impl IntoView {
    let topic_id = topic.id;
    let editing = RwSignal::new(false);
    let confirm_delete = RwSignal::new(false);
    let name = RwSignal::new(topic.name.clone());
    let description = RwSignal::new(topic.description.clone());

    let save = move || {
        let args = UpdateTopicArgs {
            topic_id,
            name: name.get_untracked(),
            description: description.get_untracked(),
        };
        spawn_local(async move {
            match invoke::<_, Topic>("update_topic", &args).await {
                Ok(_) => {
                    editing.set(false);
                    on_changed.run(());
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };
    let delete = move || {
        spawn_local(async move {
            match invoke::<_, IgnoredAny>("delete_topic", &TopicIdArgs { topic_id }).await {
                Ok(_) => on_changed.run(()),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let (post_count, open_count) = (topic.post_count, topic.open_task_count);
    view! {
        <li class="topic-row">
            <Show
                when=move || editing.get()
                fallback=move || view! {
                    <div class="topic-row-main">
                        <div class="topic-row-head">
                            <span class="topic-row-name">{move || format!("# {}", name.get())}</span>
                            <span class="topic-row-counts">
                                "ポスト " <strong>{post_count}</strong> " / 未完了 " <strong>{open_count}</strong>
                            </span>
                        </div>
                        <p class="topic-row-desc">{move || description.get()}</p>
                        <div class="topic-row-actions">
                            <button type="button" class="secondary-button" on:click=move |_| editing.set(true)>"編集"</button>
                            <Show
                                when=move || confirm_delete.get()
                                fallback=move || view! {
                                    <button type="button" class="secondary-button" on:click=move |_| confirm_delete.set(true)>"削除"</button>
                                }
                            >
                                <span class="hint">"ポストとタスクは残ります。"</span>
                                <button type="button" class="danger-button" on:click=move |_| delete()>"削除する"</button>
                                <button type="button" class="secondary-button" on:click=move |_| confirm_delete.set(false)>"やめる"</button>
                            </Show>
                        </div>
                    </div>
                }
            >
                <div class="topic-row-edit">
                    <input
                        type="text"
                        maxlength=MAX_TOPIC_NAME_CHARS
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                    <textarea
                        maxlength=MAX_POST_CHARS
                        prop:value=move || description.get()
                        on:input=move |ev| description.set(event_target_value(&ev))
                    ></textarea>
                    <div class="topic-row-actions">
                        <button type="button" class="secondary-button" on:click=move |_| editing.set(false)>"キャンセル"</button>
                        <button type="button" class="post-button" disabled=move || !name_ok(&name.get()) on:click=move |_| save()>"保存"</button>
                    </div>
                </div>
            </Show>
        </li>
    }
}
