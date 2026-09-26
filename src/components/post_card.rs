use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use leptos::task::spawn_local;

use crate::datetime::format_time;
use crate::models::{
    ClassifyStatus, KindInfo, PostWithTasks, Reply, ReplyStatus, Task, TaskStatus,
};
use crate::tauri::invoke;

/// 1件のポストと、その分類状態・タスク（FR-5, FR-6）。
#[component]
pub fn PostCard(
    item: PostWithTasks,
    kinds: RwSignal<Vec<KindInfo>>,
    on_retry: Callback<i64>,
    /// (タスク ID, 切り替え後の状態)
    on_toggle_task: Callback<(i64, TaskStatus)>,
    /// AI にリプライを頼む（聞き直し・再試行も同じ）。引数はポスト ID
    on_ask_ai: Callback<i64>,
) -> impl IntoView {
    let ai_reply = item.ai_reply().cloned();
    let PostWithTasks { post, tasks, .. } = item;
    let post_id = post.id;

    // 分類済みでタスクがあるポストだけ AI に聞ける（仕様003 FR-4）。
    // 生成中は出さず、失敗時はリプライ側の「再試行」があるので出さない
    let can_ask = post.classify_status == ClassifyStatus::Done && !tasks.is_empty();
    let ask_button = match (&ai_reply, can_ask) {
        (_, false) => None,
        (Some(r), true) if r.status != ReplyStatus::Done => None,
        (reply, true) => {
            let label = if reply.is_some() {
                "聞き直す"
            } else {
                "🤖 AI に聞く"
            };
            Some(view! {
                <button type="button" class="ask-ai-button" on:click=move |_| on_ask_ai.run(post_id)>
                    {label}
                </button>
            })
        }
    };

    let status_view = match post.classify_status {
        ClassifyStatus::Pending => view! {
            <p class="classify-status pending">"⏳ 分類中…"</p>
        }
        .into_any(),
        ClassifyStatus::Failed => {
            let detail = post.classify_error.unwrap_or_default();
            view! {
                <div class="classify-status failed">
                    <span>"⚠ 分類に失敗しました"</span>
                    <button type="button" class="retry-button" on:click=move |_| on_retry.run(post_id)>
                        "再試行"
                    </button>
                    <small class="error-detail">{detail}</small>
                </div>
            }
            .into_any()
        }
        ClassifyStatus::Done => view! {
            <div class="task-chips">
                {tasks
                    .into_iter()
                    .map(|task| view! { <TaskChip task=task kinds=kinds on_toggle=on_toggle_task /> })
                    .collect_view()}
            </div>
        }
        .into_any(),
    };

    view! {
        <article class="post-card">
            <div class="post-meta">{format_time(&post.created_at)}</div>
            <p class="post-body">{post.body}</p>
            {status_view}
            {ai_reply.map(|reply| view! { <AiReplyView reply=reply post_id=post_id on_ask_ai=on_ask_ai /> })}
            {ask_button.map(|b| view! { <div class="ask-ai">{b}</div> })}
        </article>
    }
}

/// 出典の URL を既定のブラウザで開く（tauri-plugin-opener）
fn open_in_browser(url: String) {
    #[derive(serde::Serialize)]
    struct OpenUrlArgs {
        url: String,
    }
    spawn_local(async move {
        if let Err(e) =
            invoke::<_, serde::de::IgnoredAny>("plugin:opener|open_url", &OpenUrlArgs { url }).await
        {
            web_sys_error(&format!("URL を開けませんでした: {e}"));
        }
    });
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn web_sys_error(message: &str);
}

/// AI のリプライ（仕様003 FR-5, FR-6, FR-10）
#[component]
fn AiReplyView(reply: Reply, post_id: i64, on_ask_ai: Callback<i64>) -> impl IntoView {
    let content = match reply.status {
        ReplyStatus::Pending => view! {
            <p class="reply-status">"🤖 AI が考え中…"</p>
        }
        .into_any(),
        ReplyStatus::Failed => {
            let detail = reply.error.unwrap_or_default();
            view! {
                <div class="reply-status failed">
                    <span>"⚠ リプライを作れませんでした"</span>
                    <button type="button" class="retry-button" on:click=move |_| on_ask_ai.run(post_id)>
                        "再試行"
                    </button>
                    <small class="error-detail">{detail}</small>
                </div>
            }
            .into_any()
        }
        ReplyStatus::Done => {
            let sources = (!reply.sources.is_empty()).then(|| {
                view! {
                    <div class="reply-sources">
                        <span class="reply-sources-title">"出典"</span>
                        <ul>
                            {reply
                                .sources
                                .into_iter()
                                .map(|s| {
                                    let url = s.url.clone();
                                    view! {
                                        <li>
                                            <a
                                                href=s.url.clone()
                                                title=s.url.clone()
                                                on:click=move |ev| {
                                                    ev.prevent_default();
                                                    open_in_browser(url.clone());
                                                }
                                            >
                                                {s.title}
                                            </a>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    </div>
                }
            });
            view! {
                <p class="reply-body">{reply.body}</p>
                {sources}
            }
            .into_any()
        }
    };

    view! {
        <div class="ai-reply">
            <div class="reply-author">"🤖 AI"</div>
            {content}
        </div>
    }
}

/// 種別ごとに色分けしたタスクのチップ。表示名と色はバックエンドの kind.rs から受け取る。
/// 左端のチェックボックスで完了・未完了を切り替える（仕様002）。
#[component]
fn TaskChip(
    task: Task,
    kinds: RwSignal<Vec<KindInfo>>,
    on_toggle: Callback<(i64, TaskStatus)>,
) -> impl IntoView {
    let task_id = task.id;
    let status = task.status;
    let done = status == TaskStatus::Done;
    let completed_title = task
        .completed_at
        .as_deref()
        .filter(|_| done)
        .map(|at| format!("完了: {}", format_time(at)));
    let kind = task.kind;
    let info = {
        let kind = kind.clone();
        move || kinds.with(|ks| ks.iter().find(|k| k.kind == kind).cloned())
    };

    let visible = {
        let info = info.clone();
        move || info().map(|k| k.creates_task).unwrap_or(true)
    };
    let label = {
        let info = info.clone();
        move || info().map(|k| k.label).unwrap_or_else(|| kind.clone())
    };
    let style = move || {
        let color = info()
            .map(|k| k.color)
            .unwrap_or_else(|| "#64748b".to_string());
        format!("--kind-color: {color}")
    };

    view! {
        <Show when=visible>
            <span
                class="task-chip"
                class:done=done
                style=style.clone()
                title=completed_title.clone()
            >
                <button
                    type="button"
                    class="task-check"
                    role="checkbox"
                    aria-checked=if done { "true" } else { "false" }
                    aria-label=if done { "未完了に戻す" } else { "完了にする" }
                    on:click=move |_| on_toggle.run((task_id, status.toggled()))
                >
                    {if done { "✓" } else { "" }}
                </button>
                <span class="task-chip-kind">{label.clone()}</span>
                <span class="task-chip-title">{task.title.clone()}</span>
            </span>
        </Show>
    }
}
