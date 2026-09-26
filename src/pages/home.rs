//! ホーム: ポストの入力欄とタイムライン（仕様001〜003）。状態は store にあり、ここは表示だけ。

use leptos::prelude::*;

use crate::components::{Composer, PostCard};
use crate::store::use_timeline_store;

#[component]
pub fn HomePage() -> impl IntoView {
    let store = use_timeline_store();
    let posts = store.posts;
    let load_error = store.load_error;

    view! {
        <section class="feed-panel">
            <header class="feed-header">
                <h1>"ホーム"</h1>
            </header>

            <Composer draft=store.draft on_posted=store.on_posted on_topic_created=store.on_topic_created />

            <div class="divider"></div>

            <Show when=move || load_error.get().is_some()>
                <p class="error-text banner">{move || load_error.get().unwrap_or_default()}</p>
            </Show>

            // ポストごとに作り、そのポストが変わったときだけ作り直す
            // （他のポストの分類完了などで、開いている入力欄が消えないように。仕様009）
            <For
                each=move || posts.with(|l| l.iter().map(|p| p.post.id).collect::<Vec<_>>())
                key=|id| *id
                let:post_id
            >
                {
                    let item = Memo::new(move |_| {
                        posts.with(|l| l.iter().find(|p| p.post.id == post_id).cloned())
                    });
                    move || {
                        item.get().map(|item| {
                            view! {
                                <PostCard
                                    item=item
                                    kinds=store.kinds
                                    on_retry=store.on_retry
                                    on_toggle_task=store.on_toggle_task
                                    on_ask_ai=store.on_ask_ai
                                    on_remove_topic=store.on_remove_topic
                                />
                            }
                        })
                    }
                }
            </For>

            <Show when=move || posts.with(|p| p.is_empty())>
                <p class="empty">"まだポストがありません。思ったことを書いてみましょう。"</p>
            </Show>

            <Show when=move || store.has_more.get()>
                <button type="button" class="load-more" on:click=move |_| store.load_more.run(())>
                    "さらに読み込む"
                </button>
            </Show>
        </section>
    }
}
