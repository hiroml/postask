//! 種別ごとの課題数: 横向きの積み上げ棒（未完了＋完了）。仕様006 FR-5〜FR-8, FR-12

use leptos::ev::{FocusEvent, MouseEvent};
use leptos::html::Div;
use leptos::prelude::*;

use super::rect::rect_of;
use super::tooltip::{Tooltip, TooltipData, TooltipRow};
use super::{COLOR_DONE, COLOR_OPEN};

#[derive(Debug, Clone, PartialEq)]
pub struct BarRow {
    /// 種別（絞り込みに使う）
    pub key: String,
    /// 表示名
    pub label: String,
    pub open: u32,
    pub done: u32,
}

#[component]
pub fn StackedBarChart(
    rows: Signal<Vec<BarRow>>,
    /// 棒をクリックしたとき（種別で絞り込む）
    on_select: Callback<String>,
) -> impl IntoView {
    let show_table = RwSignal::new(false);
    let tooltip = RwSignal::new(None::<TooltipData>);
    let frame = NodeRef::<Div>::new();

    // マウスの位置（枠の左上からの px）
    let position = move |ev: &MouseEvent| -> (f64, f64) {
        frame
            .get()
            .map(|el| {
                let r = rect_of(&el);
                (
                    ev.client_x() as f64 - r.left + 12.0,
                    ev.client_y() as f64 - r.top + 12.0,
                )
            })
            .unwrap_or((0.0, 0.0))
    };

    view! {
        <section class="chart-card">
            <div class="chart-header">
                <h2 class="chart-title">"種別ごとの課題数"</h2>
                <div class="chart-legend">
                    <span class="chart-legend-item">
                        <span class="chart-key-rect" style=format!("background: {COLOR_OPEN}")></span>
                        "未完了"
                    </span>
                    <span class="chart-legend-item">
                        <span class="chart-key-rect" style=format!("background: {COLOR_DONE}")></span>
                        "完了"
                    </span>
                </div>
                <button type="button" class="chart-table-toggle" on:click=move |_| show_table.update(|v| *v = !*v)>
                    {move || if show_table.get() { "グラフで見る" } else { "表で見る" }}
                </button>
            </div>

            <Show
                when=move || !show_table.get()
                fallback=move || {
                    view! {
                        <table class="chart-table">
                            <thead>
                                <tr><th>"種別"</th><th>"未完了"</th><th>"完了"</th><th>"合計"</th></tr>
                            </thead>
                            <tbody>
                                {move || rows.get().into_iter().map(|r| view! {
                                    <tr>
                                        <td>{r.label}</td>
                                        <td>{r.open}</td>
                                        <td>{r.done}</td>
                                        <td>{r.open + r.done}</td>
                                    </tr>
                                }).collect_view()}
                            </tbody>
                        </table>
                    }
                }
            >
                <div class="bar-chart" node_ref=frame on:mouseleave=move |_| tooltip.set(None)>
                    {move || {
                        let rows = rows.get();
                        let max = rows.iter().map(|r| r.open + r.done).max().unwrap_or(0).max(1) as f64;
                        rows.into_iter()
                            .map(|r| {
                                let pct = |n: u32| n as f64 / max * 100.0;
                                let (open_w, done_w) = (pct(r.open), pct(r.done));
                                let key = r.key.clone();
                                let key_for_keys = r.key.clone();
                                let label = r.label.clone();
                                let (label_o, label_d, label_f) = (label.clone(), label.clone(), label.clone());
                                let (open, done) = (r.open, r.done);
                                let done_is_zero = done == 0;
                                let has_gap = open > 0 && done > 0;
                                let one = move |title: String, row: TooltipRow, ev: &MouseEvent| {
                                    let (x, y) = position(ev);
                                    tooltip.set(Some(TooltipData { title, rows: vec![row], x, y }));
                                };
                                view! {
                                    <div
                                        class="bar-row"
                                        role="button"
                                        tabindex="0"
                                        title=format!("クリックで「{label}」に絞り込む")
                                        on:click=move |_| on_select.run(key.clone())
                                        on:keydown=move |ev| {
                                            if ev.key() == "Enter" || ev.key() == " " {
                                                ev.prevent_default();
                                                on_select.run(key_for_keys.clone());
                                            }
                                        }
                                        // キーボードで選んだときも値を出す（FR-7）
                                        on:focus=move |_: FocusEvent| {
                                            tooltip.set(Some(TooltipData {
                                                title: label_f.clone(),
                                                rows: vec![
                                                    TooltipRow { color: COLOR_OPEN, label: "未完了", value: open },
                                                    TooltipRow { color: COLOR_DONE, label: "完了", value: done },
                                                ],
                                                x: 120.0,
                                                y: 8.0,
                                            }))
                                        }
                                        on:blur=move |_: FocusEvent| tooltip.set(None)
                                    >
                                        <span class="bar-label">{label.clone()}</span>
                                        <span class="bar-track">
                                            <span
                                                class="bar-seg"
                                                class:end=done_is_zero
                                                style=format!("width: {open_w}%; background: {COLOR_OPEN}")
                                                on:mousemove=move |ev| one(label_o.clone(), TooltipRow { color: COLOR_OPEN, label: "未完了", value: open }, &ev)
                                            ></span>
                                            <span
                                                class="bar-seg end"
                                                class:gap=has_gap
                                                style=format!("width: {done_w}%; background: {COLOR_DONE}")
                                                on:mousemove=move |ev| one(label_d.clone(), TooltipRow { color: COLOR_DONE, label: "完了", value: done }, &ev)
                                            ></span>
                                        </span>
                                        <span class="bar-value">
                                            "未完了 " <strong>{open}</strong> " / 完了 " <strong>{done}</strong>
                                        </span>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                    <Tooltip data=tooltip />
                </div>
            </Show>
        </section>
    }
}
