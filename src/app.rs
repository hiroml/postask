use leptos::prelude::*;

use crate::pages::HomePage;
use crate::store::provide_timeline_store;

#[component]
pub fn App() -> impl IntoView {
    // タイムラインの状態はページの外で持つ（仕様005 FR-6）
    provide_timeline_store();

    view! {
        <main class="page-shell">
            <HomePage />
        </main>
    }
}
