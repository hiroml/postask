use leptos::task::spawn_local;
use leptos::{ev::SubmitEvent, prelude::*};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct User {
    id: String,
    username: String,
    password: String,
    name: String,
    bio: String,
    followers: usize,
    following: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Post {
    id: String,
    author_id: String,
    author_name: String,
    handle: String,
    content: String,
    created_at: String,
    likes: usize,
    retweets: usize,
    replies: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileData {
    user: User,
    posts: Vec<Post>,
}

#[derive(Serialize, Deserialize)]
struct LoginArgs {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct CreatePostArgs {
    author_id: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct ToggleArgs {
    post_id: String,
    user_id: String,
}

#[derive(Serialize, Deserialize)]
struct ProfileArgs {
    user_id: String,
}

#[component]
pub fn App() -> impl IntoView {
    let (current_user, set_current_user) = signal(None::<User>);
    let (login_name, set_login_name) = signal(String::new());
    let (login_password, set_login_password) = signal(String::new());
    let (login_error, set_login_error) = signal(String::new());
    let (draft, set_draft) = signal(String::new());
    let (posts, set_posts) = signal(Vec::<Post>::new());
    let (profile_data, set_profile_data) = signal(None::<ProfileData>);
    let (active_view, set_active_view) = signal("home".to_string());

    let refresh_posts = {
        let set_posts = set_posts;
        move || {
            spawn_local(async move {
                let result = invoke("get_posts", JsValue::NULL).await;
                if let Ok(parsed) = serde_wasm_bindgen::from_value::<Vec<Post>>(result) {
                    set_posts.set(parsed);
                }
            });
        }
    };

    let load_profile = {
        let set_profile_data = set_profile_data;
        move |user_id: String| {
            let args = serde_wasm_bindgen::to_value(&ProfileArgs { user_id }).unwrap();
            spawn_local(async move {
                let result = invoke("get_profile", args).await;
                if let Ok(parsed) = serde_wasm_bindgen::from_value::<ProfileData>(result) {
                    set_profile_data.set(Some(parsed));
                }
            });
        }
    };

    let login = move |ev: SubmitEvent| {
        ev.prevent_default();
        let username = login_name.get_untracked();
        let password = login_password.get_untracked();
        if username.trim().is_empty() || password.trim().is_empty() {
            set_login_error.set("ユーザー名とパスワードを入力してください".to_string());
            return;
        }

        let args = serde_wasm_bindgen::to_value(&LoginArgs {
            username: username.trim().to_string(),
            password: password.trim().to_string(),
        })
        .unwrap();

        spawn_local(async move {
            let result = invoke("login", args).await;
            match serde_wasm_bindgen::from_value::<User>(result) {
                Ok(user) => {
                    set_login_error.set(String::new());
                    set_current_user.set(Some(user.clone()));
                    set_active_view.set("home".to_string());
                    load_profile(user.id.clone());
                    refresh_posts();
                }
                Err(_) => {
                    set_login_error.set("ログインに失敗しました".to_string());
                }
            }
        });
    };

    let submit_post = move |ev: SubmitEvent| {
        ev.prevent_default();
        let content = draft.get_untracked();
        let trimmed = content.trim();
        let Some(current) = current_user.get_untracked() else {
            return;
        };

        if trimmed.is_empty() {
            return;
        }

        let args = serde_wasm_bindgen::to_value(&CreatePostArgs {
            author_id: current.id.clone(),
            content: trimmed.to_string(),
        })
        .unwrap();

        spawn_local(async move {
            let result = invoke("create_post", args).await;
            if serde_wasm_bindgen::from_value::<Post>(result).is_ok() {
                set_draft.set(String::new());
                refresh_posts();
                load_profile(current.id.clone());
            }
        });
    };

    let toggle_like = {
        let current_user = current_user;
        move |post_id: String| {
            let Some(user) = current_user.get_untracked() else {
                return;
            };
            let args = serde_wasm_bindgen::to_value(&ToggleArgs {
                post_id,
                user_id: user.id.clone(),
            })
            .unwrap();

            spawn_local(async move {
                let result = invoke("toggle_like", args).await;
                if serde_wasm_bindgen::from_value::<Post>(result).is_ok() {
                    refresh_posts();
                    load_profile(user.id.clone());
                }
            });
        }
    };

    let toggle_retweet = {
        let current_user = current_user;
        move |post_id: String| {
            let Some(user) = current_user.get_untracked() else {
                return;
            };
            let args = serde_wasm_bindgen::to_value(&ToggleArgs {
                post_id,
                user_id: user.id.clone(),
            })
            .unwrap();

            spawn_local(async move {
                let result = invoke("toggle_retweet", args).await;
                if serde_wasm_bindgen::from_value::<Post>(result).is_ok() {
                    refresh_posts();
                    load_profile(user.id.clone());
                }
            });
        }
    };

    let render_home = {
        let posts = posts;
        let current_user = current_user;
        let draft = draft;
        let toggle_like = toggle_like;
        let toggle_retweet = toggle_retweet;
        move || {
            let user = current_user.get().unwrap();
            view! {
                <main class="page-shell">
                    <aside class="sidebar">
                        <div class="brand">"X Clone"</div>
                        <nav class="nav">
                            <button class="nav-item active" type="button" on:click=move |_| set_active_view.set("home".to_string())>
                                "ホーム"
                            </button>
                            <button class="nav-item" type="button" on:click=move |_| {
                                set_active_view.set("profile".to_string());
                                load_profile(user.id.clone());
                            }>
                                "プロフィール"
                            </button>
                        </nav>
                        <button class="tweet-button" type="button">"投稿する"</button>
                    </aside>

                    <section class="feed-panel">
                        <header class="feed-header">
                            <h1>"ホーム"</h1>
                        </header>

                        <form class="composer" on:submit=submit_post>
                            <div class="composer-avatar">{user.name.chars().next().unwrap_or('Y')}</div>
                            <div class="composer-body">
                                <textarea
                                    placeholder="いまどうしてる？"
                                    prop:value=move || draft.get()
                                    on:input=move |ev| set_draft.set(event_target_value(&ev))
                                ></textarea>
                                <div class="composer-actions">
                                    <span class="char-count">{move || draft.get().chars().count()}</span>
                                    <button class="post-button" type="submit" disabled=move || draft.get().trim().is_empty()>
                                        "投稿"
                                    </button>
                                </div>
                            </div>
                        </form>

                        <div class="divider"></div>

                        <For each=move || posts.get() key=|post| post.id.clone() let:post>
                            { let post_id = post.id.clone();
                              let like_id = post_id.clone();
                              let retweet_id = post_id.clone();
                              let favorite_id = post_id.clone();
                              let post_replies = post.replies;
                              let post_retweets = post.retweets;
                              let post_likes = post.likes;
                              view! {
                                <article class="post-card">
                                    <div class="avatar">{post.author_name.chars().next().unwrap_or('A')}</div>
                                    <div class="post-content">
                                        <div class="post-header">
                                            <div class="user-meta">
                                                <strong>{post.author_name}</strong>
                                                <span>{post.handle}</span>
                                                <span>{" • "}</span>
                                                <span>{post.created_at}</span>
                                            </div>
                                        </div>
                                        <p>{post.content}</p>
                                        <div class="post-stats">
                                            <button type="button" class="stat-button" on:click=move |_| {
                                                toggle_like(like_id.clone());
                                            }>{"💬 "}{post_replies}</button>
                                            <button type="button" class="stat-button" on:click=move |_| {
                                                toggle_retweet(retweet_id.clone());
                                            }>{"🔁 "}{post_retweets}</button>
                                            <button type="button" class="stat-button" on:click=move |_| {
                                                toggle_like(favorite_id.clone());
                                            }>{"♥ "}{post_likes}</button>
                                        </div>
                                    </div>
                                </article>
                              }
                            }
                        </For>
                    </section>

                    <aside class="trends-panel">
                        <div class="search-box">"検索"</div>
                        <div class="trends-card">
                            <h2>"いま話題"</h2>
                            <ul>
                                <li><span>#Rust</span><small>"2.4K posts"</small></li>
                                <li><span>#Tauri</span><small>"1.9K posts"</small></li>
                                <li><span>#Leptos</span><small>"810 posts"</small></li>
                            </ul>
                        </div>
                    </aside>
                </main>
            }
        }
    };

    let render_profile = {
        let profile_data = profile_data;
        let current_user = current_user;
        let toggle_like = toggle_like;
        let toggle_retweet = toggle_retweet;
        move || {
            let user = current_user.get().unwrap();
            let profile = profile_data.get().unwrap_or_else(|| ProfileData {
                user: user.clone(),
                posts: Vec::new(),
            });
            let profile_posts = profile.posts.clone();
            let profile_count = profile_posts.len();

            view! {
                <main class="page-shell profile-shell">
                    <aside class="sidebar">
                        <div class="brand">"X Clone"</div>
                        <nav class="nav">
                            <button class="nav-item" type="button" on:click=move |_| set_active_view.set("home".to_string())>
                                "ホーム"
                            </button>
                            <button class="nav-item active" type="button" on:click=move |_| {
                                set_active_view.set("profile".to_string());
                                load_profile(user.id.clone());
                            }>
                                "プロフィール"
                            </button>
                        </nav>
                    </aside>

                    <section class="feed-panel profile-panel">
                        <header class="feed-header">
                            <h1>{profile.user.name.clone()}</h1>
                        </header>

                        <div class="profile-header-card">
                            <div class="profile-banner"></div>
                            <div class="profile-main">
                                <div class="profile-avatar">{profile.user.name.chars().next().unwrap_or('Y')}</div>
                                <div class="profile-info">
                                    <h2>{profile.user.name}</h2>
                                    <p>{format!("@{}", profile.user.username)}</p>
                                    <p>{profile.user.bio}</p>
                                    <div class="profile-stats">
                                        <span>{profile.user.following} "フォロー中"</span>
                                        <span>{profile.user.followers} "フォロワー"</span>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div class="divider"></div>

                        <For each=move || profile_posts.clone() key=|post| post.id.clone() let:post>
                            { let post_id = post.id.clone();
                              let like_id = post_id.clone();
                              let retweet_id = post_id.clone();
                              let favorite_id = post_id.clone();
                              let post_replies = post.replies;
                              let post_retweets = post.retweets;
                              let post_likes = post.likes;
                              view! {
                                <article class="post-card">
                                    <div class="avatar">{post.author_name.chars().next().unwrap_or('A')}</div>
                                    <div class="post-content">
                                        <div class="post-header">
                                            <div class="user-meta">
                                                <strong>{post.author_name}</strong>
                                                <span>{post.handle}</span>
                                                <span>{" • "}</span>
                                                <span>{post.created_at}</span>
                                            </div>
                                        </div>
                                        <p>{post.content}</p>
                                        <div class="post-stats">
                                            <button type="button" class="stat-button" on:click=move |_| {
                                                toggle_like(like_id.clone());
                                            }>{"💬 "}{post_replies}</button>
                                            <button type="button" class="stat-button" on:click=move |_| {
                                                toggle_retweet(retweet_id.clone());
                                            }>{"🔁 "}{post_retweets}</button>
                                            <button type="button" class="stat-button" on:click=move |_| {
                                                toggle_like(favorite_id.clone());
                                            }>{"♥ "}{post_likes}</button>
                                        </div>
                                    </div>
                                </article>
                              }
                            }
                        </For>
                    </section>

                    <aside class="trends-panel">
                        <div class="trends-card">
                            <h2>"プロフィール"</h2>
                            <ul>
                                <li><span>"投稿数"</span><small>{profile_count}</small></li>
                                <li><span>"フォロー中"</span><small>{profile.user.following}</small></li>
                                <li><span>"フォロワー"</span><small>{profile.user.followers}</small></li>
                            </ul>
                        </div>
                    </aside>
                </main>
            }
        }
    };

    let render_logged_in = move || {
        let active = active_view.get();
        if active == "profile" {
            render_profile().into_any()
        } else {
            render_home().into_any()
        }
    };

    view! {
        <Show
            when=move || current_user.get().is_some()
            fallback=move || {
                view! {
                    <main class="login-shell">
                        <div class="login-card">
                            <h1>"X Clone"</h1>
                            <p>"ログインして投稿を始めましょう"</p>
                            <form class="login-form" on:submit=login>
                                <input
                                    placeholder="ユーザー名"
                                    prop:value=move || login_name.get()
                                    on:input=move |ev| set_login_name.set(event_target_value(&ev))
                                />
                                <input
                                    type="password"
                                    placeholder="パスワード"
                                    prop:value=move || login_password.get()
                                    on:input=move |ev| set_login_password.set(event_target_value(&ev))
                                />
                                <button type="submit">"ログイン"</button>
                            </form>
                            <p class="error-text">{move || login_error.get()}</p>
                        </div>
                    </main>
                }
            }
        >
            {render_logged_in}
        </Show>
    }
}
