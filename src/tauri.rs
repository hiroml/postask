//! Tauri（`window.__TAURI__`）とのやり取り。`tauri.conf.json` の `withGlobalTauri: true` が前提。

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    async fn listen_raw(
        event: &str,
        handler: &Closure<dyn FnMut(JsValue)>,
    ) -> Result<JsValue, JsValue>;
}

/// コマンドを呼ぶ。エラーはバックエンドが返したメッセージ文字列。
pub async fn invoke<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A) -> Result<R, String> {
    let args = serde_wasm_bindgen::to_value(args).map_err(|e| e.to_string())?;
    let value = invoke_raw(cmd, args)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
    serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())
}

/// 引数のないコマンド用
#[derive(Serialize)]
pub struct NoArgs {}

#[derive(Deserialize)]
struct EventEnvelope<T> {
    payload: T,
}

/// イベントを購読する。アプリが動いている間ずっと受け取るので、解除はしない。
pub async fn listen<T: DeserializeOwned + 'static>(
    event: &str,
    mut handler: impl FnMut(T) + 'static,
) -> Result<(), String> {
    let closure =
        Closure::<dyn FnMut(JsValue)>::new(
            move |raw: JsValue| match serde_wasm_bindgen::from_value::<EventEnvelope<T>>(raw) {
                Ok(env) => handler(env.payload),
                Err(e) => web_sys_log(&format!("イベントを読み取れませんでした: {e}")),
            },
        );
    listen_raw(event, &closure)
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
    closure.forget();
    Ok(())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn web_sys_log(message: &str);
}
