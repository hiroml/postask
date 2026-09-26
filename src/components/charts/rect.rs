//! 要素の画面上の位置と大きさ（`getBoundingClientRect`）。
//! Leptos が使う web-sys ではこの機能が有効になっていないため、依存を増やさずに自前で呼ぶ。

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    type AnyElement;
    type DomRect;

    #[wasm_bindgen(method, js_name = getBoundingClientRect)]
    fn get_bounding_client_rect(this: &AnyElement) -> DomRect;
    #[wasm_bindgen(method, getter)]
    fn left(this: &DomRect) -> f64;
    #[wasm_bindgen(method, getter)]
    fn top(this: &DomRect) -> f64;
    #[wasm_bindgen(method, getter)]
    fn width(this: &DomRect) -> f64;
}

/// 要素の左端・上端・幅（px）
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
}

pub fn rect_of<E: JsCast>(el: &E) -> Rect {
    let r = el.unchecked_ref::<AnyElement>().get_bounding_client_rect();
    Rect {
        left: r.left(),
        top: r.top(),
        width: r.width(),
    }
}
