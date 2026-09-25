use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::models::{ClassifyStatus, KindInfo, PostWithTasks, Task};

#[wasm_bindgen]
extern "C" {
    type Date;

    #[wasm_bindgen(constructor)]
    fn new(value: &str) -> Date;
    #[wasm_bindgen(method, js_name = getTime)]
    fn get_time(this: &Date) -> f64;
    #[wasm_bindgen(method, js_name = getMonth)]
    fn get_month(this: &Date) -> f64;
    #[wasm_bindgen(method, js_name = getDate)]
    fn get_date(this: &Date) -> f64;
    #[wasm_bindgen(method, js_name = getHours)]
    fn get_hours(this: &Date) -> f64;
    #[wasm_bindgen(method, js_name = getMinutes)]
    fn get_minutes(this: &Date) -> f64;
}

/// RFC 3339（UTC）をローカル時刻の「MM/DD HH:MM」にする。
fn format_time(rfc3339: &str) -> String {
    let d = Date::new(rfc3339);
    if d.get_time().is_nan() {
        return rfc3339.to_string();
    }
    format!(
        "{:02}/{:02} {:02}:{:02}",
        d.get_month() as u32 + 1,
        d.get_date() as u32,
        d.get_hours() as u32,
        d.get_minutes() as u32
    )
}

/// 1件のポストと、その分類状態・タスク（FR-5, FR-6）。
#[component]
pub fn PostCard(
    item: PostWithTasks,
    kinds: RwSignal<Vec<KindInfo>>,
    on_retry: Callback<i64>,
) -> impl IntoView {
    let PostWithTasks { post, tasks } = item;
    let post_id = post.id;

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
                    .map(|task| view! { <TaskChip task=task kinds=kinds /> })
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
        </article>
    }
}

/// 種別ごとに色分けしたタスクのチップ。表示名と色はバックエンドの kind.rs から受け取る。
#[component]
fn TaskChip(task: Task, kinds: RwSignal<Vec<KindInfo>>) -> impl IntoView {
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
            <span class="task-chip" style=style.clone()>
                <span class="task-chip-kind">{label.clone()}</span>
                <span class="task-chip-title">{task.title.clone()}</span>
            </span>
        </Show>
    }
}
