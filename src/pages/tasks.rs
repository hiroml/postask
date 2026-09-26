//! 課題ページ: 絞り込み・数字・グラフ・一覧（仕様006）。集計はバックエンド（get_task_dashboard）が行う。

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;

use crate::components::{BarRow, LineChart, StackedBarChart};
use crate::datetime::format_date;
use crate::models::{KindInfo, PostWithTasks, TaskDashboard, TaskRow, TaskStatus};
use crate::store::{use_timeline_store, SetTaskStatusArgs};
use crate::tauri::invoke;

/// 元のポストを一覧に出すときの文字数（FR-13）
const POST_EXCERPT_CHARS: usize = 30;

#[derive(Serialize)]
struct DashboardArgs {
    period: String,
    kind: Option<String>,
    status: Option<TaskStatus>,
}

fn excerpt(body: &str) -> String {
    let mut s: String = body.chars().take(POST_EXCERPT_CHARS).collect();
    if body.chars().count() > POST_EXCERPT_CHARS {
        s.push('…');
    }
    s
}

fn kind_info(kinds: &[KindInfo], kind: &str) -> (String, String) {
    kinds
        .iter()
        .find(|k| k.kind == kind)
        .map(|k| (k.label.clone(), k.color.clone()))
        .unwrap_or_else(|| (kind.to_string(), "#64748b".to_string()))
}

#[component]
pub fn TasksPage() -> impl IntoView {
    let store = use_timeline_store();
    let kinds = store.kinds;

    // 絞り込み（FR-1）。空文字 = すべて
    let period = RwSignal::new("all".to_string());
    let kind = RwSignal::new(String::new());
    let status = RwSignal::new(String::new());

    let dashboard = RwSignal::new(None::<TaskDashboard>);
    let loading = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    // 古い応答で新しい表示を上書きしないための番号
    let request_seq = StoredValue::new(0u32);

    let reload = move || {
        let args = DashboardArgs {
            period: period.get_untracked(),
            kind: Some(kind.get_untracked()).filter(|k| !k.is_empty()),
            status: match status.get_untracked().as_str() {
                "open" => Some(TaskStatus::Open),
                "done" => Some(TaskStatus::Done),
                _ => None,
            },
        };
        request_seq.update_value(|s| *s += 1);
        let seq = request_seq.get_value();
        loading.set(true);
        spawn_local(async move {
            let result = invoke::<_, TaskDashboard>("get_task_dashboard", &args).await;
            if request_seq.get_value() != seq {
                return;
            }
            match result {
                Ok(d) => {
                    dashboard.set(Some(d));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    };

    // 絞り込みが変わったとき・タイムラインが変わったとき（新しいポストの分類、ホームでの完了など）に取り直す
    Effect::new(move |_| {
        period.track();
        kind.track();
        status.track();
        store.posts.track();
        reload();
    });

    // 一覧の完了チェック（FR-15）。先に表示を変え、結果でホームと集計を更新する
    let toggle = move |task_id: i64, next: TaskStatus| {
        let set_row = move |s: TaskStatus| {
            dashboard.update(|d| {
                if let Some(row) = d
                    .as_mut()
                    .and_then(|d| d.tasks.iter_mut().find(|r| r.task.id == task_id))
                {
                    row.task.status = s;
                }
            })
        };
        set_row(next);
        spawn_local(async move {
            let args = SetTaskStatusArgs {
                task_id,
                status: next,
            };
            match invoke::<_, PostWithTasks>("set_task_status", &args).await {
                Ok(item) => {
                    // ホームに読み込み済みのポストなら更新（タイムラインが変わるので集計も取り直される）
                    if store
                        .posts
                        .with_untracked(|l| l.iter().any(|p| p.post.id == item.post.id))
                    {
                        store.upsert.run(item);
                    } else {
                        reload();
                    }
                }
                Err(e) => {
                    set_row(match next {
                        TaskStatus::Open => TaskStatus::Done,
                        TaskStatus::Done => TaskStatus::Open,
                    });
                    error.set(Some(format!("タスクの状態を保存できませんでした: {e}")));
                }
            }
        });
    };

    let is_default_filter =
        move || period.get() == "all" && kind.get().is_empty() && status.get().is_empty();

    let bar_rows = Signal::derive(move || {
        let ks = kinds.get();
        dashboard
            .get()
            .map(|d| {
                d.by_kind
                    .into_iter()
                    .map(|k| BarRow {
                        label: kind_info(&ks, &k.kind).0,
                        key: k.kind,
                        open: k.open,
                        done: k.done,
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    let trend_points =
        Signal::derive(move || dashboard.get().map(|d| d.trend.points).unwrap_or_default());
    let trend_bucket = Signal::derive(move || {
        dashboard
            .get()
            .map(|d| d.trend.bucket)
            .unwrap_or(crate::models::Bucket::Week)
    });
    let on_select_kind = Callback::new(move |k: String| kind.set(k));

    view! {
        <section class="feed-panel tasks-page">
            <header class="feed-header">
                <h1>"課題"</h1>
            </header>

            // 絞り込み（1行）
            <div class="filter-row">
                <label class="filter">
                    "期間"
                    <select on:change=move |ev| period.set(event_target_value(&ev)) prop:value=move || period.get()>
                        <option value="all">"全期間"</option>
                        <option value="7d">"直近7日"</option>
                        <option value="30d">"直近30日"</option>
                        <option value="90d">"直近90日"</option>
                    </select>
                </label>
                <label class="filter">
                    "種別"
                    <select on:change=move |ev| kind.set(event_target_value(&ev)) prop:value=move || kind.get()>
                        <option value="">"すべて"</option>
                        {move || kinds.get().into_iter().filter(|k| k.creates_task).map(|k| view! {
                            <option value=k.kind.clone() selected=move || kind.get() == k.kind>{k.label.clone()}</option>
                        }).collect_view()}
                    </select>
                </label>
                <label class="filter">
                    "状態"
                    <select on:change=move |ev| status.set(event_target_value(&ev)) prop:value=move || status.get()>
                        <option value="">"すべて"</option>
                        <option value="open">"未完了"</option>
                        <option value="done">"完了"</option>
                    </select>
                </label>
            </div>

            <Show when=move || error.get().is_some()>
                <p class="error-text banner">{move || error.get().unwrap_or_default()}</p>
            </Show>

            // 読み込み中は前の表示を薄くして残す（FR-3）
            <div class="tasks-content" class:stale=move || loading.get() && dashboard.with(|d| d.is_some())>
                {move || {
                    let Some(d) = dashboard.get() else {
                        return view! { <p class="empty">"読み込み中…"</p> }.into_any();
                    };
                    let total = d.totals.open + d.totals.done;
                    if total == 0 {
                        let message = if is_default_filter() {
                            "まだタスクがありません。ホームでポストすると、ここに集計されます。"
                        } else {
                            "条件に合うタスクがありません。"
                        };
                        return view! { <p class="empty">{message}</p> }.into_any();
                    }
                    let rate = d.totals.completion_rate.map(|r| format!("{r}%")).unwrap_or_else(|| "—".to_string());
                    let truncated = d.truncated;
                    let count = d.tasks.len();
                    let rows = d.tasks;
                    view! {
                        <div class="stat-tiles">
                            <div class="stat-tile"><span class="stat-label">"未完了"</span><span class="stat-value">{d.totals.open}</span></div>
                            <div class="stat-tile"><span class="stat-label">"完了"</span><span class="stat-value">{d.totals.done}</span></div>
                            <div class="stat-tile"><span class="stat-label">"完了率"</span><span class="stat-value">{rate}</span></div>
                        </div>

                        <StackedBarChart rows=bar_rows on_select=on_select_kind />
                        <LineChart points=trend_points bucket=trend_bucket />

                        <section class="task-list">
                            <h2 class="chart-title">{format!("課題の一覧（{}件）", total)}</h2>
                            {truncated.then(|| view! { <p class="list-note">{format!("新しい{count}件を表示しています")}</p> })}
                            <table class="task-table">
                                <thead>
                                    <tr><th class="col-check">"完了"</th><th>"種別"</th><th>"タイトル"</th><th>"元のポスト"</th><th>"作成"</th><th>"完了日"</th></tr>
                                </thead>
                                <tbody>
                                    {rows.into_iter().map(|row| view! { <TaskListRow row=row kinds=kinds on_toggle=toggle /> }).collect_view()}
                                </tbody>
                            </table>
                        </section>
                    }
                    .into_any()
                }}
            </div>
        </section>
    }
}

#[component]
fn TaskListRow(
    row: TaskRow,
    kinds: RwSignal<Vec<KindInfo>>,
    on_toggle: impl Fn(i64, TaskStatus) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let t = row.task;
    let (label, color) = kinds.with(|ks| kind_info(ks, &t.kind));
    let done = t.status == TaskStatus::Done;
    let next = if done {
        TaskStatus::Open
    } else {
        TaskStatus::Done
    };
    let id = t.id;
    view! {
        <tr class:done=done>
            <td class="col-check">
                <span class="task-chip-plain" style=format!("--kind-color: {color}")>
                    <button
                        type="button"
                        class="task-check"
                        role="checkbox"
                        aria-checked=if done { "true" } else { "false" }
                        aria-label=if done { "未完了に戻す" } else { "完了にする" }
                        on:click=move |_| on_toggle(id, next)
                    >
                        {if done { "✓" } else { "" }}
                    </button>
                </span>
            </td>
            <td>
                <span class="task-chip-kind" style=format!("background: {color}")>{label}</span>
            </td>
            <td class="col-title">{t.title}</td>
            <td class="col-post" title=row.post_body.clone()>{excerpt(&row.post_body)}</td>
            <td class="col-date">{format_date(&t.created_at)}</td>
            <td class="col-date">{t.completed_at.as_deref().map(format_date).unwrap_or_default()}</td>
        </tr>
    }
}
