use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::de::IgnoredAny;
use serde::Serialize;

use crate::components::{Composer, PostCard};
use crate::models::{
    ClassifyStatus, KindInfo, Post, PostWithTasks, Reply, ReplyStatus, Task, TaskStatus, AI_AUTHOR,
};
use crate::tauri::{invoke, listen, NoArgs};

/// バックエンドの commands::POST_UPDATED_EVENT と同じ名前（分類・AI リプライの完了で届く）
const POST_UPDATED_EVENT: &str = "post-updated";
const PAGE_SIZE: u32 = 50;

#[derive(Serialize)]
struct ListTimelineArgs {
    limit: u32,
    before_id: Option<i64>,
}

#[derive(Serialize)]
/// post_id だけを渡すコマンド用（retry_classification, request_ai_reply）
struct PostIdArgs {
    post_id: i64,
}

#[derive(Serialize)]
struct SetTaskStatusArgs {
    task_id: i64,
    status: TaskStatus,
}

#[component]
pub fn App() -> impl IntoView {
    let posts = RwSignal::new(Vec::<PostWithTasks>::new());
    let kinds = RwSignal::new(Vec::<KindInfo>::new());
    let has_more = RwSignal::new(false);
    let load_error = RwSignal::new(None::<String>);

    // 同じ id があれば置き換え、無ければ追加して新しい順に並べる
    let upsert = move |item: PostWithTasks| {
        posts.update(|list| {
            if let Some(existing) = list.iter_mut().find(|p| p.post.id == item.post.id) {
                *existing = item;
            } else {
                list.push(item);
                list.sort_by_key(|p| std::cmp::Reverse(p.post.id));
            }
        })
    };

    let load_page = move |before_id: Option<i64>| {
        spawn_local(async move {
            let args = ListTimelineArgs {
                limit: PAGE_SIZE,
                before_id,
            };
            match invoke::<_, Vec<PostWithTasks>>("list_timeline", &args).await {
                Ok(items) => {
                    has_more.set(items.len() as u32 == PAGE_SIZE);
                    items.into_iter().for_each(upsert);
                }
                Err(e) => load_error.set(Some(e)),
            }
        });
    };

    // 起動時: 種別の読み込み → イベント購読 → タイムライン読み込み（購読を先にして取りこぼしを防ぐ）
    spawn_local(async move {
        match invoke::<_, Vec<KindInfo>>("list_kinds", &NoArgs {}).await {
            Ok(k) => kinds.set(k),
            Err(e) => load_error.set(Some(e)),
        }
        if let Err(e) = listen::<PostWithTasks>(POST_UPDATED_EVENT, upsert).await {
            load_error.set(Some(format!("分類結果の受信を開始できませんでした: {e}")));
        }
        load_page(None);
    });

    // 投稿直後は「分類中」で先頭に出す。分類完了イベントが先に届いていた場合は上書きしない
    let on_posted = Callback::new(move |post: Post| {
        posts.update(|list| {
            if !list.iter().any(|p| p.post.id == post.id) {
                list.insert(
                    0,
                    PostWithTasks {
                        post,
                        tasks: Vec::new(),
                        replies: Vec::new(),
                    },
                );
            }
        })
    });

    let set_status = move |post_id: i64, status: ClassifyStatus, error: Option<String>| {
        posts.update(|list| {
            if let Some(p) = list.iter_mut().find(|p| p.post.id == post_id) {
                p.post.classify_status = status;
                p.post.classify_error = error;
            }
        })
    };

    let on_retry = Callback::new(move |post_id: i64| {
        set_status(post_id, ClassifyStatus::Pending, None);
        spawn_local(async move {
            if let Err(e) =
                invoke::<_, IgnoredAny>("retry_classification", &PostIdArgs { post_id }).await
            {
                set_status(post_id, ClassifyStatus::Failed, Some(e));
            }
        });
    });

    // タスクの状態を書き換え、書き換える前のタスクを返す
    let replace_task = move |task_id: i64, f: &dyn Fn(&mut Task)| -> Option<Task> {
        let mut previous = None;
        posts.update(|list| {
            if let Some(t) = list
                .iter_mut()
                .flat_map(|p| p.tasks.iter_mut())
                .find(|t| t.id == task_id)
            {
                previous = Some(t.clone());
                f(t);
            }
        });
        previous
    };

    // 完了・未完了の切り替え（仕様002）。先に表示を変え、保存に失敗したら元に戻す
    let on_toggle_task = Callback::new(move |(task_id, status): (i64, TaskStatus)| {
        let Some(previous) = replace_task(task_id, &|t| {
            t.status = status;
            if status == TaskStatus::Open {
                t.completed_at = None;
            }
        }) else {
            return;
        };
        spawn_local(async move {
            let args = SetTaskStatusArgs { task_id, status };
            match invoke::<_, PostWithTasks>("set_task_status", &args).await {
                Ok(item) => upsert(item),
                Err(e) => {
                    replace_task(task_id, &|t| *t = previous.clone());
                    load_error.set(Some(format!("タスクの状態を保存できませんでした: {e}")));
                }
            }
        });
    });

    // AI のリプライを画面上で置き換える（無ければ追加）
    let set_ai_reply = move |post_id: i64, status: ReplyStatus, error: Option<String>| {
        posts.update(|list| {
            if let Some(p) = list.iter_mut().find(|p| p.post.id == post_id) {
                p.replies.retain(|r| r.author != AI_AUTHOR);
                p.replies.push(Reply {
                    author: AI_AUTHOR.to_string(),
                    body: String::new(),
                    sources: Vec::new(),
                    status,
                    error,
                });
            }
        })
    };

    // AI にリプライを頼む（仕様003）。すぐ「考え中」を出し、結果は post-updated イベントで届く
    let on_ask_ai = Callback::new(move |post_id: i64| {
        set_ai_reply(post_id, ReplyStatus::Pending, None);
        spawn_local(async move {
            if let Err(e) =
                invoke::<_, IgnoredAny>("request_ai_reply", &PostIdArgs { post_id }).await
            {
                set_ai_reply(post_id, ReplyStatus::Failed, Some(e));
            }
        });
    });

    view! {
        <main class="page-shell">
            <section class="feed-panel">
                <header class="feed-header">
                    <h1>"ホーム"</h1>
                </header>

                <Composer on_posted=on_posted />

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
                                    kinds=kinds
                                    on_retry=on_retry
                                    on_toggle_task=on_toggle_task
                                    on_ask_ai=on_ask_ai
                                />
                            }
                        })
                        .collect_view()
                }}

                <Show when=move || posts.with(|p| p.is_empty())>
                    <p class="empty">"まだポストがありません。思ったことを書いてみましょう。"</p>
                </Show>

                <Show when=move || has_more.get()>
                    <button
                        type="button"
                        class="load-more"
                        on:click=move |_| {
                            load_page(posts.with_untracked(|p| p.last().map(|x| x.post.id)))
                        }
                    >
                        "さらに読み込む"
                    </button>
                </Show>
            </section>
        </main>
    }
}
