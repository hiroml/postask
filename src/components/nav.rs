//! 左のメニュー（ナビゲーター、仕様005）。

use leptos::ev;
use leptos::prelude::*;
use leptos_router::components::A;

use crate::store::use_timeline_store;

/// メニューの1項目
pub struct NavItem {
    pub href: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
    /// 中身があるページか。false のページはメニューで薄く表示し、開くと「準備中」（Q-022）
    pub ready: bool,
}

/// メニューの項目（Q-018: 6ページ、Q-020: 課題と話題は別）
pub const NAV_ITEMS: &[NavItem] = &[
    NavItem {
        href: "/",
        icon: "🏠",
        label: "ホーム",
        ready: true,
    },
    NavItem {
        href: "/tasks",
        icon: "✅",
        label: "課題",
        ready: true,
    },
    NavItem {
        href: "/topics",
        icon: "#",
        label: "話題",
        ready: true,
    },
    NavItem {
        href: "/notifications",
        icon: "🔔",
        label: "通知",
        ready: false,
    },
    NavItem {
        href: "/profile",
        icon: "👤",
        label: "プロフィール",
        ready: false,
    },
    NavItem {
        href: "/settings",
        icon: "⚙",
        label: "設定",
        ready: false,
    },
];

#[component]
pub fn Nav() -> impl IntoView {
    let store = use_timeline_store();

    view! {
        <nav class="nav" aria-label="メニュー">
            <div class="nav-brand">
                <span class="nav-brand-name">"x-clone"</span>
                <Show when=move || store.is_dev.get()>
                    <span class="dev-badge" title="開発用の DB（x-clone-dev.sqlite）を使っています">
                        "開発版"
                    </span>
                </Show>
            </div>

            <ul class="nav-list">
                {NAV_ITEMS
                    .iter()
                    .map(|item| {
                        let class = if item.ready { "nav-item" } else { "nav-item coming-soon" };
                        // ホームは完全一致のときだけ強調（他のページで「/」が一致しないように）
                        let exact = item.href == "/";
                        view! {
                            <li>
                                <A href=item.href exact=exact attr:class=class attr:title=item.label>
                                    <span class="nav-icon" aria-hidden="true">{item.icon}</span>
                                    <span class="nav-label">{item.label}</span>
                                </A>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>

            <AccountMenu />
        </nav>
    }
}

/// 左下のアカウントメニュー（FR-3）。ログアウトは B-016（アカウント）を作るまで出さない。
#[component]
fn AccountMenu() -> impl IntoView {
    let open = RwSignal::new(false);

    // Esc で閉じる（FR-7）
    let handle = window_event_listener(ev::keydown, move |e| {
        if e.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || handle.remove());

    view! {
        <div class="account">
            <Show when=move || open.get()>
                // メニューの外をクリックしたら閉じる
                <div class="account-backdrop" on:click=move |_| open.set(false)></div>
                // 項目をクリックしたら閉じる（移動はリンクが行う）
                <div class="account-popup" role="menu" on:click=move |_| open.set(false)>
                    <A href="/profile" attr:class="account-popup-item">
                        "👤 プロフィール"
                    </A>
                    <A href="/settings" attr:class="account-popup-item">
                        "⚙ 設定"
                    </A>
                </div>
            </Show>
            <button
                type="button"
                class="account-button"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                title="アカウント"
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <span class="account-avatar" aria-hidden="true">"🙂"</span>
                <span class="nav-label account-name">"自分"</span>
                <span class="nav-label account-more" aria-hidden="true">"⋯"</span>
            </button>
        </div>
    }
}
