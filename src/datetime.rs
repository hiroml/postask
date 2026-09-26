//! 日時の表示（パソコンのタイムゾーンで表示する）。

use wasm_bindgen::prelude::*;

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

fn parse(rfc3339: &str) -> Option<Date> {
    let d = Date::new(rfc3339);
    (!d.get_time().is_nan()).then_some(d)
}

/// RFC 3339（UTC）をローカル時刻の「MM/DD HH:MM」にする。
pub fn format_time(rfc3339: &str) -> String {
    let Some(d) = parse(rfc3339) else {
        return rfc3339.to_string();
    };
    format!(
        "{:02}/{:02} {:02}:{:02}",
        d.get_month() as u32 + 1,
        d.get_date() as u32,
        d.get_hours() as u32,
        d.get_minutes() as u32
    )
}

/// RFC 3339（UTC）をローカルの「MM/DD」にする。
pub fn format_date(rfc3339: &str) -> String {
    let Some(d) = parse(rfc3339) else {
        return rfc3339.to_string();
    };
    format!("{:02}/{:02}", d.get_month() as u32 + 1, d.get_date() as u32)
}

/// 「YYYY-MM-DD」を「M/D」にする（グラフの目盛り用）。
pub fn short_day(ymd: &str) -> String {
    let mut parts = ymd.split('-').skip(1).filter_map(|p| p.parse::<u32>().ok());
    match (parts.next(), parts.next()) {
        (Some(m), Some(d)) => format!("{m}/{d}"),
        _ => ymd.to_string(),
    }
}
