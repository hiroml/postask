//! 推移: 作成数と完了数の2本の折れ線（縦軸は1本）。仕様006 FR-9〜FR-12

use leptos::ev::{KeyboardEvent, PointerEvent};
use leptos::html::Div;
use leptos::prelude::*;

use super::rect::rect_of;
use super::tooltip::{Tooltip, TooltipData, TooltipRow};
use super::{COLOR_DONE, COLOR_OPEN};
use crate::datetime::short_day;
use crate::models::{Bucket, TrendPoint};

// SVG の座標（viewBox）
const W: f64 = 640.0;
const H: f64 = 220.0;
const ML: f64 = 36.0;
const MR: f64 = 52.0;
const MT: f64 = 12.0;
const MB: f64 = 28.0;
const PW: f64 = W - ML - MR;
const PH: f64 = H - MT - MB;
/// これ以下の点の数なら、各点に丸（8px）を描く
const MAX_MARKERS: usize = 14;
/// 横軸の目盛りの最大数
const MAX_X_LABELS: usize = 6;

fn x_of(i: usize, n: usize) -> f64 {
    if n <= 1 {
        ML + PW / 2.0
    } else {
        ML + i as f64 * PW / (n - 1) as f64
    }
}

/// 縦軸の上限。0・半分・上限の3本の目盛りが整数になるよう4の倍数にする
fn y_max(points: &[TrendPoint]) -> u32 {
    let max = points
        .iter()
        .map(|p| p.created.max(p.completed))
        .max()
        .unwrap_or(0);
    (max.div_ceil(4) * 4).max(4)
}

fn y_of(v: u32, max: u32) -> f64 {
    MT + PH - v as f64 / max as f64 * PH
}

fn polyline(points: &[TrendPoint], max: u32, value: impl Fn(&TrendPoint) -> u32) -> String {
    let n = points.len();
    points
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{:.1},{:.1}", x_of(i, n), y_of(value(p), max)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn point_label(p: &TrendPoint, bucket: Bucket) -> String {
    match bucket {
        Bucket::Day => short_day(&p.start),
        Bucket::Week => format!("{}〜", short_day(&p.start)),
    }
}

#[component]
pub fn LineChart(points: Signal<Vec<TrendPoint>>, bucket: Signal<Bucket>) -> impl IntoView {
    let show_table = RwSignal::new(false);
    let hover = RwSignal::new(None::<usize>);
    let tooltip = RwSignal::new(None::<TooltipData>);
    let frame = NodeRef::<Div>::new();

    // 選んでいる点のツールチップを出す（位置は枠の幅に合わせて換算）
    let show = move |i: usize| {
        let pts = points.get_untracked();
        let Some(p) = pts.get(i) else { return };
        hover.set(Some(i));
        let width = frame.get().map(|el| rect_of(&el).width).unwrap_or(W);
        let x = x_of(i, pts.len()) / W * width;
        tooltip.set(Some(TooltipData {
            title: point_label(p, bucket.get_untracked()),
            rows: vec![
                TooltipRow {
                    color: COLOR_OPEN,
                    label: "作成",
                    value: p.created,
                },
                TooltipRow {
                    color: COLOR_DONE,
                    label: "完了",
                    value: p.completed,
                },
            ],
            x: x + 12.0,
            y: 8.0,
        }));
    };
    let hide = move || {
        hover.set(None);
        tooltip.set(None);
    };

    // 縦の線は一番近い日（週）に合わせる（FR-10）
    let on_move = move |ev: PointerEvent| {
        let n = points.get_untracked().len();
        let Some(el) = frame.get() else { return };
        if n == 0 {
            return;
        }
        let r = rect_of(&el);
        let sx = (ev.client_x() as f64 - r.left) / r.width * W;
        let i = if n == 1 {
            0
        } else {
            (((sx - ML) / (PW / (n - 1) as f64)).round().max(0.0) as usize).min(n - 1)
        };
        show(i);
    };

    // キーボード: 左右キーで点を移動（FR-7 相当）
    let on_key = move |ev: KeyboardEvent| {
        let n = points.get_untracked().len();
        if n == 0 {
            return;
        }
        let cur = hover.get_untracked().unwrap_or(n - 1);
        let next = match ev.key().as_str() {
            "ArrowLeft" => cur.saturating_sub(1),
            "ArrowRight" => (cur + 1).min(n - 1),
            _ => return,
        };
        ev.prevent_default();
        show(next);
    };

    let title = move || match bucket.get() {
        Bucket::Day => "推移（日ごと）",
        Bucket::Week => "推移（週ごと）",
    };

    view! {
        <section class="chart-card">
            <div class="chart-header">
                <h2 class="chart-title">{title}</h2>
                <div class="chart-legend">
                    <span class="chart-legend-item">
                        <span class="chart-key-line" style=format!("background: {COLOR_OPEN}")></span>
                        "作成"
                    </span>
                    <span class="chart-legend-item">
                        <span class="chart-key-line" style=format!("background: {COLOR_DONE}")></span>
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
                            <thead><tr><th>"期間"</th><th>"作成"</th><th>"完了"</th></tr></thead>
                            <tbody>
                                {move || {
                                    let b = bucket.get();
                                    points.get().into_iter().map(|p| view! {
                                        <tr><td>{point_label(&p, b)}</td><td>{p.created}</td><td>{p.completed}</td></tr>
                                    }).collect_view()
                                }}
                            </tbody>
                        </table>
                    }
                }
            >
                <div class="line-chart" node_ref=frame>
                    {move || {
                        let pts = points.get();
                        let n = pts.len();
                        if n == 0 {
                            return view! { <p class="chart-empty">"データがありません"</p> }.into_any();
                        }
                        let max = y_max(&pts);
                        let ticks = [0, max / 2, max];
                        let label_step = n.div_ceil(MAX_X_LABELS).max(1);
                        let last = &pts[n - 1];
                        // 線の端のラベル（重なるときは上下にずらす）
                        let (mut y_created, mut y_done) = (y_of(last.created, max), y_of(last.completed, max));
                        if (y_created - y_done).abs() < 12.0 {
                            if y_created <= y_done {
                                y_created -= 6.0;
                                y_done += 6.0;
                            } else {
                                y_created += 6.0;
                                y_done -= 6.0;
                            }
                        }
                        let markers = n <= MAX_MARKERS;
                        view! {
                            <svg
                                viewBox=format!("0 0 {W} {H}")
                                class="line-svg"
                                tabindex="0"
                                role="img"
                                aria-label="作成数と完了数の推移。左右キーで日（週）を移動して値を表示"
                                on:pointermove=on_move
                                on:pointerleave=move |_| hide()
                                on:keydown=on_key
                                on:focus=move |_| show(n - 1)
                                on:blur=move |_| hide()
                            >
                                // 目盛りと補助線（控えめな灰色）
                                {ticks.iter().map(|&t| {
                                    let y = y_of(t, max);
                                    view! {
                                        <line x1=ML x2=W - MR y1=y y2=y class="chart-grid" />
                                        <text x=ML - 6.0 y=y + 4.0 class="chart-axis-label" text-anchor="end">{t}</text>
                                    }
                                }).collect_view()}
                                {pts.iter().enumerate().filter(|(i, _)| i % label_step == 0 || *i == n - 1).map(|(i, p)| view! {
                                    <text x=x_of(i, n) y=H - 8.0 class="chart-axis-label" text-anchor="middle">{short_day(&p.start)}</text>
                                }).collect_view()}

                                // 縦の線（選んでいる点）
                                {move || hover.get().map(|i| {
                                    let x = x_of(i, n);
                                    view! { <line x1=x x2=x y1=MT y2=MT + PH class="chart-crosshair" /> }
                                })}

                                <polyline points=polyline(&pts, max, |p| p.created) fill="none" stroke=COLOR_OPEN stroke-width="2" stroke-linejoin="round" />
                                <polyline points=polyline(&pts, max, |p| p.completed) fill="none" stroke=COLOR_DONE stroke-width="2" stroke-linejoin="round" />

                                {markers.then(|| pts.iter().enumerate().map(|(i, p)| view! {
                                    <circle cx=x_of(i, n) cy=y_of(p.created, max) r="4" fill=COLOR_OPEN class="chart-marker" />
                                    <circle cx=x_of(i, n) cy=y_of(p.completed, max) r="4" fill=COLOR_DONE class="chart-marker" />
                                }).collect_view())}

                                // 選んでいる点は大きめの丸と白い縁取り
                                {move || hover.get().and_then(|i| pts.get(i).cloned().map(|p| {
                                    let x = x_of(i, n);
                                    view! {
                                        <circle cx=x cy=y_of(p.created, max) r="5" fill=COLOR_OPEN class="chart-marker-active" />
                                        <circle cx=x cy=y_of(p.completed, max) r="5" fill=COLOR_DONE class="chart-marker-active" />
                                    }
                                }))}

                                // 線の端に直接ラベル（FR-11）
                                <text x=x_of(n - 1, n) + 8.0 y=y_created + 4.0 class="chart-end-label">"作成"</text>
                                <text x=x_of(n - 1, n) + 8.0 y=y_done + 4.0 class="chart-end-label">"完了"</text>
                            </svg>
                        }
                        .into_any()
                    }}
                    <Tooltip data=tooltip />
                </div>
            </Show>
        </section>
    }
}
