use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::use_navigate;
use leptos_router::path;
use leptos_router::NavigateOptions;

use crate::components::Nav;
use crate::pages::{ComingSoonPage, HomePage, TasksPage, TopicsPage};
use crate::store::provide_timeline_store;

/// 知らないパスのときにホームへ移動する。
/// `leptos_router` の `<Redirect>` は描画中に場所（URL）を書き換えて止まってしまうため、
/// 描画が終わってから移動する。
#[component]
fn GoHome() -> impl IntoView {
    let navigate = use_navigate();
    spawn_local(async move {
        navigate(
            "/",
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });
}

#[component]
pub fn App() -> impl IntoView {
    // タイムラインの状態はページの外で持つ（仕様005 FR-6）
    provide_timeline_store();

    view! {
        <Router>
            <div class="app-layout">
                <Nav />
                <main class="page-area">
                    // 知らないパスはホームへ
                    <Routes fallback=|| view! { <GoHome /> }>
                        <Route path=path!("/") view=HomePage />
                        <Route path=path!("/tasks") view=TasksPage />
                        <Route path=path!("/topics") view=TopicsPage />
                        <Route
                            path=path!("/notifications")
                            view=|| view! {
                                <ComingSoonPage
                                    title="通知"
                                    plans=&["AI リプライの完了などのお知らせと、通知する内容の設定（B-026）"]
                                />
                            }
                        />
                        <Route
                            path=path!("/profile")
                            view=|| view! {
                                <ComingSoonPage
                                    title="プロフィール"
                                    plans=&[
                                        "最近の関心・取り組んでいる課題の要約（B-015）",
                                        "アイコンと名前の設定（B-024）",
                                    ]
                                />
                            }
                        />
                        <Route
                            path=path!("/settings")
                            view=|| view! {
                                <ComingSoonPage
                                    title="設定"
                                    plans=&[
                                        "AI: API キーとモデルの設定（B-006）",
                                        "通知: 通知する内容の設定（B-026）",
                                        "表示: アイコン・名前（B-024）",
                                        "データ: バックアップ・書き出し（B-031）",
                                    ]
                                />
                            }
                        />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}
