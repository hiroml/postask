mod app;
mod components;
mod datetime;
mod models;
mod pages;
mod store;
mod tauri;

use app::*;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! {
            <App/>
        }
    })
}
