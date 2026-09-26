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
                <Show when=move || store.is_dev.get()>
                    <span class="dev-badge" title="開発用の DB（x-clone-dev.sqlite）を使っています">
                        "開発版"
                    </span>
                </Show>
            </header>

            <Composer draft=store.draft on_posted=store.on_posted />

            <div class="divider"></div>

            <Show when=move || load_error.get().is_some()>
                <p class="error-text banner">{move || load_error.get().unwrap_or_default()}</p>
            </Show>

            {move || {
                posts
                    .get()
                    .into_iter()
                    .map(|item| {
                        view! {
                            <PostCard
                                item=item
                                kinds=store.kinds
                                on_retry=store.on_retry
                                on_toggle_task=store.on_toggle_task
                                on_ask_ai=store.on_ask_ai
                            />
                        }
                    })
                    .collect_view()
            }}

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
