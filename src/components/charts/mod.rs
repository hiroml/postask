//! 課題ページのグラフ（仕様006）。グラフ用のクレートは使わず自前で描く（Q-025）。
//!
//! 見た目は dataviz スキルの方針に従う: 色は2色だけ（配色の検証スクリプトで確認済み）、
//! 文字は系列の色を使わず黒・灰、ツールチップは数値を強く、系列は短い線の印で示す。

mod bar;
mod line;
mod rect;
mod tooltip;

pub use bar::{BarRow, StackedBarChart};
pub use line::LineChart;

/// 未完了・作成（配色の検証: 白背景に対し色覚の違いでも区別できることを確認済み）
pub const COLOR_OPEN: &str = "#2a78d6";
/// 完了（どちらのグラフでも「完了」は同じ色）
pub const COLOR_DONE: &str = "#eb6834";
