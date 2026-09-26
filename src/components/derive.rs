//! 関連タスクの派生（仕様009）: タスクから書く入力欄と、AI リプライの下の候補の欄。
//! 状態は store にあり（タイムラインの更新で消えないように）、ここは表示だけ。

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;

use crate::models::{post_length, MAX_POST_CHARS};
use crate::store::{use_timeline_store, SuggestState};

/// ポストの下に開く、派生ポストの入力欄（FR-1, FR-4）。そのポストで開いているときだけ出す。
#[component]
pub fn DeriveComposer(post_id: i64) -> impl IntoView {
    let store = use_timeline_store();
    let draft = store.derive_draft;
    // 入力のたびに作り直さないよう、開閉と元のタスクだけを見る
    let open = Memo::new(move |_| {
        draft.with(|d| {
            d.as_ref()
                .filter(|d| d.post_id == post_id)
                .map(|d| (d.task_id, d.task_title.clone()))
        })
    });

    move || {
        open.get().map(|(_, title)| {
            let text =
                move || draft.with(|d| d.as_ref().map(|d| d.text.clone()).unwrap_or_default());
            let count =
                move || draft.with(|d| d.as_ref().map(|d| post_length(&d.text)).unwrap_or(0));
            let over = move || count() > MAX_POST_CHARS;
            let sending = move || draft.with(|d| d.as_ref().is_some_and(|d| d.sending));
            let error = move || draft.with(|d| d.as_ref().and_then(|d| d.error.clone()));
            let cannot_post = move || sending() || count() == 0 || over();
            let close = move || draft.set(None);

            let textarea = NodeRef::<html::Textarea>::new();
            Effect::new(move |_| {
                if let Some(el) = textarea.get() {
                    let _ = el.focus();
                }
            });

            view! {
                <div class="derive-composer">
                    <p class="derive-composer-title">{format!("↳ 「{title}」に関連するタスク")}</p>
                    <textarea
                        node_ref=textarea
                        rows="2"
                        placeholder="関連するやることを書く（Esc で閉じる）"
                        prop:value=text
                        on:input=move |ev| {
                            let v = event_target_value(&ev);
                            draft.update(|d| {
                                if let Some(d) = d {
                                    d.text = v;
                                }
                            })
                        }
                        on:keydown=move |ev: KeyboardEvent| {
                            if ev.key() == "Escape" {
                                close();
                            } else if ev.ctrl_key() && ev.key() == "Enter" {
                                ev.prevent_default();
                                store.on_submit_derive.run(());
                            }
                        }
                    ></textarea>
                    <div class="derive-actions">
                        <span class="char-count" class:over=over>
                            {move || format!("{}/{}", count(), MAX_POST_CHARS)}
                        </span>
                        <button type="button" class="secondary-button" on:click=move |_| close()>
                            "キャンセル"
                        </button>
                        <button
                            type="button"
                            class="post-button"
                            disabled=cannot_post
                            on:click=move |_| store.on_submit_derive.run(())
                        >
                            {move || if sending() { "投稿中…" } else { "ポスト" }}
                        </button>
                    </div>
                    {move || error().map(|e| view! { <p class="error-text">{e}</p> })}
                </div>
            }
        })
    }
}

/// AI リプライの下の「↳ ここから関連タスクを作る」と候補の欄（FR-1b）。
/// `tasks` は (タスク ID, タイトル)。候補の親の表示に使う。
#[component]
pub fn DeriveSuggestions(post_id: i64, tasks: Vec<(i64, String)>) -> impl IntoView {
    let store = use_timeline_store();
    let panel = store.suggest_panel;
    // 候補の文やチェックの変更で作り直さないよう、状態と件数だけを見る
    let shape = Memo::new(move |_| {
        panel.with(|p| {
            p.as_ref()
                .filter(|p| p.post_id == post_id)
                .map(|p| (p.state.clone(), p.candidates.len()))
        })
    });
    let tasks = StoredValue::new(tasks);
    let close = move || panel.set(None);

    move || {
        match shape.get() {
        None => view! {
            <button type="button" class="derive-suggest-button" on:click=move |_| store.on_suggest.run(post_id)>
                "↳ ここから関連タスクを作る"
            </button>
        }
        .into_any(),
        Some((SuggestState::Loading, _)) => view! {
            <p class="reply-status">"🤖 関連タスクの候補を作っています…"</p>
        }
        .into_any(),
        Some((SuggestState::Failed(e), _)) => view! {
            <div class="reply-status failed">
                <span>"⚠ 候補を作れませんでした"</span>
                <button type="button" class="retry-button" on:click=move |_| store.on_suggest.run(post_id)>
                    "再試行"
                </button>
                <button type="button" class="secondary-button" on:click=move |_| close()>
                    "閉じる"
                </button>
                <small class="error-detail">{e}</small>
            </div>
        }
        .into_any(),
        Some((SuggestState::Ready, 0)) => view! {
            <div class="derive-suggestions">
                <p class="hint">"関連タスクの候補はありませんでした。"</p>
                <button type="button" class="secondary-button" on:click=move |_| close()>
                    "閉じる"
                </button>
            </div>
        }
        .into_any(),
        Some((SuggestState::Ready, n)) => {
            let checked_count = move || {
                panel.with(|p| {
                    p.as_ref()
                        .map(|p| p.candidates.iter().filter(|c| c.checked && c.is_valid()).count())
                        .unwrap_or(0)
                })
            };
            let posting = move || panel.with(|p| p.as_ref().is_some_and(|p| p.posting));
            let error = move || panel.with(|p| p.as_ref().and_then(|p| p.error.clone()));
            let cannot_post = move || posting() || checked_count() == 0;
            view! {
                <div class="derive-suggestions" role="group" aria-label="関連タスクの候補">
                    <p class="derive-suggestions-title">"↳ 関連タスクの候補（投稿するものにチェック）"</p>
                    <ul>
                        {(0..n).map(|i| view! { <CandidateRow index=i tasks=tasks /> }).collect_view()}
                    </ul>
                    {move || error().map(|e| view! { <p class="error-text">{e}</p> })}
                    <div class="derive-actions">
                        <button type="button" class="secondary-button" on:click=move |_| close()>
                            "キャンセル"
                        </button>
                        <button
                            type="button"
                            class="post-button"
                            disabled=cannot_post
                            on:click=move |_| store.on_post_suggestions.run(())
                        >
                            {move || {
                                if posting() {
                                    "投稿中…".to_string()
                                } else {
                                    format!("{}件を投稿", checked_count())
                                }
                            }}
                        </button>
                    </div>
                </div>
            }
            .into_any()
        }
    }
    }
}

/// 候補1件: チェック・直せる文・親のタスク
#[component]
fn CandidateRow(index: usize, tasks: StoredValue<Vec<(i64, String)>>) -> impl IntoView {
    let panel = use_timeline_store().suggest_panel;
    let get = move |f: &dyn Fn(&crate::store::Candidate) -> String| {
        panel.with(|p| {
            p.as_ref()
                .and_then(|p| p.candidates.get(index))
                .map(f)
                .unwrap_or_default()
        })
    };
    let update = move |f: &dyn Fn(&mut crate::store::Candidate)| {
        panel.update(|p| {
            if let Some(c) = p.as_mut().and_then(|p| p.candidates.get_mut(index)) {
                f(c);
            }
        })
    };
    let checked = move || {
        panel.with(|p| {
            p.as_ref()
                .and_then(|p| p.candidates.get(index))
                .is_some_and(|c| c.checked)
        })
    };
    let parent = move || {
        let task_id = panel.with(|p| {
            p.as_ref()
                .and_then(|p| p.candidates.get(index))
                .map(|c| c.task_id)
        });
        tasks.with_value(|ts| {
            ts.iter()
                .find(|(id, _)| Some(*id) == task_id)
                .map(|(_, title)| format!("← {title}"))
                .unwrap_or_default()
        })
    };

    view! {
        <li class="derive-candidate" class:checked=checked>
            <input
                type="checkbox"
                aria-label="この候補を投稿する"
                prop:checked=checked
                on:change=move |ev| {
                    let on = event_target_checked(&ev);
                    update(&|c| c.checked = on)
                }
            />
            <input
                type="text"
                class="derive-candidate-text"
                maxlength=MAX_POST_CHARS
                prop:value=move || get(&|c| c.text.clone())
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    update(&|c| c.text = v.clone())
                }
            />
            <span class="derive-candidate-parent">{parent}</span>
        </li>
    }
}
