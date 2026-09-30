use leptos::ev::{KeyboardEvent, SubmitEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;

use crate::models::{post_length, CreatedTopic, Post, MAX_POST_CHARS, MAX_TOPIC_NAME_CHARS};
use crate::tauri::invoke;

#[derive(Serialize)]
struct CreatePostArgs {
    body: String,
}

#[derive(Serialize)]
struct BodyArgs {
    body: String,
}

#[derive(Serialize)]
struct CreateTopicArgs {
    name: String,
    description: String,
    body: Option<String>,
}

/// 入力欄のモード（仕様008 FR-2）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Post,
    /// 本文を概要にして話題を作る
    Topic,
}

/// ポストの入力欄（FR-1）。280文字を超えるか空のときは投稿できない。
/// 「# 話題として投稿」に切り替えると、本文を概要にして話題を作る（仕様008）。
#[component]
pub fn Composer(
    /// 入力中の下書き。ページを移動しても残るよう外（store）で持つ（仕様005 FR-6）
    draft: RwSignal<String>,
    on_posted: Callback<Post>,
    on_topic_created: Callback<CreatedTopic>,
) -> impl IntoView {
    let submitting = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let mode = RwSignal::new(Mode::Post);
    // 話題の作成ダイアログ: Some(名前) のとき開いている
    let topic_name = RwSignal::new(None::<String>);
    let topic_hint = RwSignal::new(None::<String>);

    let remaining = move || MAX_POST_CHARS as i64 - post_length(&draft.get()) as i64;
    let can_post = move || {
        let n = post_length(&draft.get());
        n > 0 && n <= MAX_POST_CHARS && !submitting.get()
    };

    let post = move |body: String| {
        spawn_local(async move {
            match invoke::<_, Post>("create_post", &CreatePostArgs { body }).await {
                Ok(post) => {
                    draft.set(String::new());
                    error.set(None);
                    on_posted.run(post);
                }
                Err(e) => error.set(Some(e)),
            }
            submitting.set(false);
        });
    };

    // 話題として投稿: まず AI に名前を提案してもらい、確認のダイアログを開く
    let open_topic_dialog = move |body: String| {
        spawn_local(async move {
            match invoke::<_, String>("suggest_topic_name", &BodyArgs { body }).await {
                Ok(name) => {
                    topic_hint.set(None);
                    topic_name.set(Some(name));
                }
                Err(e) => {
                    topic_hint.set(Some(format!(
                        "名前の提案に失敗しました（{e}）。名前を入力してください"
                    )));
                    topic_name.set(Some(String::new()));
                }
            }
            submitting.set(false);
        });
    };

    let submit = move || {
        let body = draft.get_untracked();
        let n = post_length(&body);
        if n == 0 || n > MAX_POST_CHARS || submitting.get_untracked() {
            return;
        }
        submitting.set(true);
        match mode.get_untracked() {
            Mode::Post => post(body),
            Mode::Topic => open_topic_dialog(body),
        }
    };

    let create_topic = move || {
        let Some(name) = topic_name.get_untracked() else {
            return;
        };
        let body = draft.get_untracked();
        submitting.set(true);
        spawn_local(async move {
            let args = CreateTopicArgs {
                name,
                description: String::new(),
                body: Some(body),
            };
            match invoke::<_, CreatedTopic>("create_topic", &args).await {
                Ok(created) => {
                    draft.set(String::new());
                    error.set(None);
                    topic_name.set(None);
                    mode.set(Mode::Post);
                    on_topic_created.run(created);
                }
                Err(e) => topic_hint.set(Some(e)),
            }
            submitting.set(false);
        });
    };

    let name_len =
        move || topic_name.with(|n| n.as_deref().map(|s| s.trim().chars().count()).unwrap_or(0));
    let name_over = move || name_len() > MAX_TOPIC_NAME_CHARS;
    let can_create = move || {
        let n = name_len();
        n > 0 && n <= MAX_TOPIC_NAME_CHARS && !submitting.get()
    };

    view! {
        <form
            class="composer"
            on:submit=move |ev: SubmitEvent| {
                ev.prevent_default();
                submit();
            }
        >
            <textarea
                placeholder=move || match mode.get() {
                    Mode::Post => "いま何してる？ 何が気になる？",
                    Mode::Topic => "話題の概要を書く（例: postask の開発: Tauri と Leptos で作るタスク管理アプリ）",
                }
                prop:value=move || draft.get()
                on:input=move |ev| draft.set(event_target_value(&ev))
                on:keydown=move |ev: KeyboardEvent| {
                    if ev.ctrl_key() && ev.key() == "Enter" {
                        ev.prevent_default();
                        submit();
                    }
                }
            ></textarea>
            <div class="composer-actions">
                <span class="char-count" class:over=move || remaining() < 0>
                    {remaining}
                </span>
                <span class="hint">"Ctrl+Enter でポスト"</span>
                <select
                    class="composer-mode"
                    aria-label="投稿の種類"
                    on:change=move |ev| {
                        mode.set(if event_target_value(&ev) == "topic" { Mode::Topic } else { Mode::Post })
                    }
                    prop:value=move || if mode.get() == Mode::Topic { "topic" } else { "post" }
                >
                    <option value="post">"ポスト"</option>
                    <option value="topic">"# 話題として投稿"</option>
                </select>
                <button class="post-button" type="submit" disabled=move || !can_post()>
                    {move || match mode.get() {
                        Mode::Post => "ポスト",
                        Mode::Topic => "話題を作る",
                    }}
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="error-text">{move || error.get().unwrap_or_default()}</p>
            </Show>

            // 話題の作成ダイアログ（名前は AI の提案。直せる）
            <Show when=move || topic_name.with(|n| n.is_some())>
                <div class="topic-dialog" role="dialog" aria-label="話題を作成">
                    <p class="topic-dialog-title">"# 話題を作成"</p>
                    <label class="topic-dialog-field">
                        "名前"
                        <input
                            type="text"
                            prop:value=move || topic_name.get().unwrap_or_default()
                            on:input=move |ev| topic_name.set(Some(event_target_value(&ev)))
                        />
                        <span class="char-count" class:over=name_over>
                            {move || format!("{}/{}", name_len(), MAX_TOPIC_NAME_CHARS)}
                        </span>
                    </label>
                    <p class="topic-dialog-desc">"概要: " {move || draft.get()}</p>
                    <Show when=move || topic_hint.get().is_some()>
                        <p class="error-text">{move || topic_hint.get().unwrap_or_default()}</p>
                    </Show>
                    <div class="topic-dialog-actions">
                        <button type="button" class="secondary-button" on:click=move |_| topic_name.set(None)>
                            "キャンセル"
                        </button>
                        <button type="button" class="post-button" disabled=move || !can_create() on:click=move |_| create_topic()>
                            "作成"
                        </button>
                    </div>
                </div>
            </Show>
        </form>
    }
}
