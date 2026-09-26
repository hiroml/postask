//! 中身がまだ無いページ（仕様005 FR-5、Q-022）。予定と関連するバックログ ID を表示する。

use leptos::prelude::*;

#[component]
pub fn ComingSoonPage(
    title: &'static str,
    /// このページでできるようになる予定のこと（1行ずつ）
    plans: &'static [&'static str],
) -> impl IntoView {
    view! {
        <section class="feed-panel">
            <header class="feed-header">
                <h1>{title}</h1>
            </header>
            <div class="coming-soon-body">
                <p class="coming-soon-lead">"🚧 準備中です"</p>
                <p>"このページでは、次のことができるようになる予定です。"</p>
                <ul class="coming-soon-plans">
                    {plans.iter().map(|p| view! { <li>{*p}</li> }).collect_view()}
                </ul>
            </div>
        </section>
    }
}
