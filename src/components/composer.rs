use leptos::ev::{KeyboardEvent, SubmitEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;

use crate::models::{post_length, Post, MAX_POST_CHARS};
use crate::tauri::invoke;

#[derive(Serialize)]
struct CreatePostArgs {
    body: String,
}

/// ポストの入力欄（FR-1）。280文字を超えるか空のときは投稿できない。
#[component]
pub fn Composer(
    /// 入力中の下書き。ページを移動しても残るよう外（store）で持つ（仕様005 FR-6）
    draft: RwSignal<String>,
    on_posted: Callback<Post>,
) -> impl IntoView {
    let submitting = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let remaining = move || MAX_POST_CHARS as i64 - post_length(&draft.get()) as i64;
    let can_post = move || {
        let n = post_length(&draft.get());
        n > 0 && n <= MAX_POST_CHARS && !submitting.get()
    };

    let submit = move || {
        let body = draft.get_untracked();
        let n = post_length(&body);
        if n == 0 || n > MAX_POST_CHARS || submitting.get_untracked() {
            return;
        }
        submitting.set(true);
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

    view! {
        <form
            class="composer"
            on:submit=move |ev: SubmitEvent| {
                ev.prevent_default();
                submit();
            }
        >
            <textarea
                placeholder="いま何してる？ 何が気になる？"
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
                <button class="post-button" type="submit" disabled=move || !can_post()>
                    "ポスト"
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="error-text">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </form>
    }
}
