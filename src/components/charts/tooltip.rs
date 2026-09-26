use leptos::prelude::*;

/// ツールチップの1行（系列の色・名前・値）
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipRow {
    pub color: &'static str,
    pub label: &'static str,
    pub value: u32,
}

/// ツールチップの中身と位置（グラフの枠の左上からの px）
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipData {
    pub title: String,
    pub rows: Vec<TooltipRow>,
    pub x: f64,
    pub y: f64,
}

/// 値を太く大きく、系列名は小さく。系列は短い線の印で示す。
#[component]
pub fn Tooltip(data: RwSignal<Option<TooltipData>>) -> impl IntoView {
    move || {
        data.get().map(|d| {
            view! {
                <div class="chart-tooltip" style=format!("left: {}px; top: {}px", d.x, d.y) role="status">
                    <div class="chart-tooltip-title">{d.title}</div>
                    {d
                        .rows
                        .into_iter()
                        .map(|r| {
                            view! {
                                <div class="chart-tooltip-row">
                                    <span class="chart-key-line" style=format!("background: {}", r.color)></span>
                                    <strong class="chart-tooltip-value">{r.value}</strong>
                                    <span class="chart-tooltip-label">{r.label}</span>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>
            }
        })
    }
}
