//! アプリ全体で共有するタイムラインの状態と操作（仕様005 FR-6）。
//! ページを移動しても消えないよう、ページの外（App）で1つだけ作り、context で配る。

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::de::IgnoredAny;
use serde::Serialize;

use crate::models::{
    post_length, AppInfo, ClassifyStatus, CreatedTopic, DerivedSuggestion, KindInfo, ParentRef,
    Post, PostWithTasks, Reply, ReplyStatus, Task, TaskStatus, AI_AUTHOR, MAX_POST_CHARS,
};
use crate::tauri::{invoke, listen, NoArgs};

/// バックエンドの commands::POST_UPDATED_EVENT と同じ名前（分類・AI リプライの完了で届く）
const POST_UPDATED_EVENT: &str = "post-updated";
const PAGE_SIZE: u32 = 50;

#[derive(Serialize)]
struct ListTimelineArgs {
    limit: u32,
    before_id: Option<i64>,
}

/// post_id だけを渡すコマンド用（retry_classification, request_ai_reply）
#[derive(Serialize)]
struct PostIdArgs {
    post_id: i64,
}

#[derive(Serialize)]
struct RemoveTopicArgs {
    post_id: i64,
    topic_id: i64,
}

#[derive(Serialize)]
struct CreateDerivedPostArgs {
    body: String,
    parent_task_id: i64,
}

/// タスクから派生ポストを書く入力欄（仕様009 FR-1）。開けるのは一度に1つ
#[derive(Debug, Clone, PartialEq)]
pub struct DeriveDraft {
    pub post_id: i64,
    pub task_id: i64,
    pub task_title: String,
    pub text: String,
    pub sending: bool,
    pub error: Option<String>,
}

/// AI の関連タスクの候補の状態（仕様009 FR-1b）
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestState {
    Loading,
    Failed(String),
    Ready,
}

/// 候補1件。文は投稿前に直せる。既定はチェックなし
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub text: String,
    pub task_id: i64,
    pub checked: bool,
}

/// AI リプライの下の候補の欄。開けるのは一度に1つ
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestPanel {
    pub post_id: i64,
    pub state: SuggestState,
    pub candidates: Vec<Candidate>,
    pub posting: bool,
    pub error: Option<String>,
}

impl Candidate {
    /// 投稿できる文か（空でなく、280文字以内）
    pub fn is_valid(&self) -> bool {
        let n = post_length(&self.text);
        n > 0 && n <= MAX_POST_CHARS
    }
}

#[derive(Serialize)]
pub struct SetTaskStatusArgs {
    pub task_id: i64,
    pub status: TaskStatus,
}

#[derive(Clone, Copy)]
pub struct TimelineStore {
    pub posts: RwSignal<Vec<PostWithTasks>>,
    pub kinds: RwSignal<Vec<KindInfo>>,
    pub has_more: RwSignal<bool>,
    pub load_error: RwSignal<Option<String>>,
    /// 開発版かどうか（仕様004）
    pub is_dev: RwSignal<bool>,
    /// 入力中の下書き（ページを移動しても残す）
    pub draft: RwSignal<String>,
    pub load_more: Callback<()>,
    /// 取得・更新した `PostWithTasks` を画面の状態に反映する（無ければ追加）。課題ページの完了チェック用
    pub upsert: Callback<PostWithTasks>,
    pub on_posted: Callback<Post>,
    pub on_retry: Callback<i64>,
    /// (タスク ID, 切り替え後の状態)
    pub on_toggle_task: Callback<(i64, TaskStatus)>,
    pub on_ask_ai: Callback<i64>,
    /// 話題を作ったとき（入力欄から作ったときは話題のポストを先頭に出す。仕様008）
    pub on_topic_created: Callback<CreatedTopic>,
    /// (ポスト ID, 話題 ID): ポストから話題を外す（仕様008 FR-9）
    pub on_remove_topic: Callback<(i64, i64)>,
    /// 派生ポストの入力欄（仕様009）
    pub derive_draft: RwSignal<Option<DeriveDraft>>,
    /// (ポスト ID, タスク ID, タスクのタイトル): 派生ポストの入力欄を開く
    pub on_open_derive: Callback<(i64, i64, String)>,
    pub on_submit_derive: Callback<()>,
    /// AI の関連タスクの候補の欄（仕様009 FR-1b）
    pub suggest_panel: RwSignal<Option<SuggestPanel>>,
    /// ポスト ID: 候補を作る（再試行も同じ）
    pub on_suggest: Callback<i64>,
    /// チェックした候補を派生ポストとして投稿する
    pub on_post_suggestions: Callback<()>,
}

impl TimelineStore {
    /// 状態を作り、起動時の読み込み（開発版かどうか・種別・イベント購読・タイムライン）を始める。
    fn new() -> Self {
        let posts = RwSignal::new(Vec::<PostWithTasks>::new());
        let kinds = RwSignal::new(Vec::<KindInfo>::new());
        let has_more = RwSignal::new(false);
        let load_error = RwSignal::new(None::<String>);
        let is_dev = RwSignal::new(false);
        let draft = RwSignal::new(String::new());

        // 同じ id があれば置き換え、無ければ追加して新しい順に並べる
        let upsert = move |item: PostWithTasks| {
            posts.update(|list| {
                if let Some(existing) = list.iter_mut().find(|p| p.post.id == item.post.id) {
                    *existing = item;
                } else {
                    list.push(item);
                    list.sort_by_key(|p| std::cmp::Reverse(p.post.id));
                }
            })
        };

        let load_page = move |before_id: Option<i64>| {
            spawn_local(async move {
                let args = ListTimelineArgs {
                    limit: PAGE_SIZE,
                    before_id,
                };
                match invoke::<_, Vec<PostWithTasks>>("list_timeline", &args).await {
                    Ok(items) => {
                        has_more.set(items.len() as u32 == PAGE_SIZE);
                        items.into_iter().for_each(upsert);
                    }
                    Err(e) => load_error.set(Some(e)),
                }
            });
        };

        spawn_local(async move {
            if let Ok(info) = invoke::<_, AppInfo>("app_info", &NoArgs {}).await {
                is_dev.set(info.dev);
            }
        });

        // 起動時: 種別の読み込み → イベント購読 → タイムライン読み込み（購読を先にして取りこぼしを防ぐ）
        spawn_local(async move {
            match invoke::<_, Vec<KindInfo>>("list_kinds", &NoArgs {}).await {
                Ok(k) => kinds.set(k),
                Err(e) => load_error.set(Some(e)),
            }
            if let Err(e) = listen::<PostWithTasks>(POST_UPDATED_EVENT, upsert).await {
                load_error.set(Some(format!("分類結果の受信を開始できませんでした: {e}")));
            }
            load_page(None);
        });

        let load_more = Callback::new(move |_: ()| {
            load_page(posts.with_untracked(|p| p.last().map(|x| x.post.id)))
        });

        // 投稿直後は「分類中」で先頭に出す。分類完了イベントが先に届いていた場合は上書きしない。
        // 派生ポストなら、元のタスクのタイトルを画面にあるポストから探して「↳ 〇〇 から」を出す（仕様009）
        let on_posted = Callback::new(move |post: Post| {
            posts.update(|list| {
                if !list.iter().any(|p| p.post.id == post.id) {
                    let parent = post.parent_task_id.and_then(|task_id| {
                        list.iter()
                            .flat_map(|p| p.tasks.iter())
                            .find(|t| t.id == task_id)
                            .map(|t| ParentRef {
                                task_id,
                                title: t.title.clone(),
                            })
                    });
                    list.insert(
                        0,
                        PostWithTasks {
                            post,
                            tasks: Vec::new(),
                            replies: Vec::new(),
                            topics: Vec::new(),
                            parent,
                        },
                    );
                }
            })
        });

        let set_status = move |post_id: i64, status: ClassifyStatus, error: Option<String>| {
            posts.update(|list| {
                if let Some(p) = list.iter_mut().find(|p| p.post.id == post_id) {
                    p.post.classify_status = status;
                    p.post.classify_error = error;
                }
            })
        };

        let on_retry = Callback::new(move |post_id: i64| {
            set_status(post_id, ClassifyStatus::Pending, None);
            spawn_local(async move {
                if let Err(e) =
                    invoke::<_, IgnoredAny>("retry_classification", &PostIdArgs { post_id }).await
                {
                    set_status(post_id, ClassifyStatus::Failed, Some(e));
                }
            });
        });

        // タスクの状態を書き換え、書き換える前のタスクを返す
        let replace_task = move |task_id: i64, f: &dyn Fn(&mut Task)| -> Option<Task> {
            let mut previous = None;
            posts.update(|list| {
                if let Some(t) = list
                    .iter_mut()
                    .flat_map(|p| p.tasks.iter_mut())
                    .find(|t| t.id == task_id)
                {
                    previous = Some(t.clone());
                    f(t);
                }
            });
            previous
        };

        // 完了・未完了の切り替え（仕様002）。先に表示を変え、保存に失敗したら元に戻す
        let on_toggle_task = Callback::new(move |(task_id, status): (i64, TaskStatus)| {
            let Some(previous) = replace_task(task_id, &|t| {
                t.status = status;
                if status == TaskStatus::Open {
                    t.completed_at = None;
                }
            }) else {
                return;
            };
            spawn_local(async move {
                let args = SetTaskStatusArgs { task_id, status };
                match invoke::<_, PostWithTasks>("set_task_status", &args).await {
                    Ok(item) => upsert(item),
                    Err(e) => {
                        replace_task(task_id, &|t| *t = previous.clone());
                        load_error.set(Some(format!("タスクの状態を保存できませんでした: {e}")));
                    }
                }
            });
        });

        // AI のリプライを画面上で置き換える（無ければ追加）
        let set_ai_reply = move |post_id: i64, status: ReplyStatus, error: Option<String>| {
            posts.update(|list| {
                if let Some(p) = list.iter_mut().find(|p| p.post.id == post_id) {
                    p.replies.retain(|r| r.author != AI_AUTHOR);
                    p.replies.push(Reply {
                        author: AI_AUTHOR.to_string(),
                        body: String::new(),
                        sources: Vec::new(),
                        status,
                        error,
                    });
                }
            })
        };

        // AI にリプライを頼む（仕様003）。すぐ「考え中」を出し、結果は post-updated イベントで届く
        let on_ask_ai = Callback::new(move |post_id: i64| {
            set_ai_reply(post_id, ReplyStatus::Pending, None);
            spawn_local(async move {
                if let Err(e) =
                    invoke::<_, IgnoredAny>("request_ai_reply", &PostIdArgs { post_id }).await
                {
                    set_ai_reply(post_id, ReplyStatus::Failed, Some(e));
                }
            });
        });

        // 話題を作ったとき: 話題のポストがあれば先頭に出す（振り分け直しの結果は post-updated で届く）
        let on_topic_created = Callback::new(move |created: CreatedTopic| {
            if let Some(item) = created.post {
                upsert(item);
            }
        });

        // ポストから話題を外す（仕様008 FR-9）
        let on_remove_topic = Callback::new(move |(post_id, topic_id): (i64, i64)| {
            spawn_local(async move {
                let args = RemoveTopicArgs { post_id, topic_id };
                match invoke::<_, PostWithTasks>("remove_post_topic", &args).await {
                    Ok(item) => upsert(item),
                    Err(e) => load_error.set(Some(format!("話題を外せませんでした: {e}"))),
                }
            });
        });

        // ---- 派生ポスト（仕様009） ----
        let derive_draft = RwSignal::new(None::<DeriveDraft>);
        let suggest_panel = RwSignal::new(None::<SuggestPanel>);

        let on_open_derive =
            Callback::new(move |(post_id, task_id, task_title): (i64, i64, String)| {
                derive_draft.set(Some(DeriveDraft {
                    post_id,
                    task_id,
                    task_title,
                    text: String::new(),
                    sending: false,
                    error: None,
                }))
            });

        let on_submit_derive = Callback::new(move |_: ()| {
            let Some(draft) = derive_draft.get_untracked() else {
                return;
            };
            let n = post_length(&draft.text);
            if draft.sending || n == 0 || n > MAX_POST_CHARS {
                return;
            }
            derive_draft.update(|d| {
                if let Some(d) = d {
                    d.sending = true;
                    d.error = None;
                }
            });
            spawn_local(async move {
                let args = CreateDerivedPostArgs {
                    body: draft.text.clone(),
                    parent_task_id: draft.task_id,
                };
                match invoke::<_, Post>("create_post", &args).await {
                    Ok(post) => {
                        on_posted.run(post);
                        derive_draft.set(None);
                    }
                    Err(e) => derive_draft.update(|d| {
                        if let Some(d) = d.as_mut().filter(|d| d.task_id == draft.task_id) {
                            d.sending = false;
                            d.error = Some(e);
                        }
                    }),
                }
            });
        });

        let on_suggest = Callback::new(move |post_id: i64| {
            suggest_panel.set(Some(SuggestPanel {
                post_id,
                state: SuggestState::Loading,
                candidates: Vec::new(),
                posting: false,
                error: None,
            }));
            spawn_local(async move {
                let result = invoke::<_, Vec<DerivedSuggestion>>(
                    "suggest_derived_tasks",
                    &PostIdArgs { post_id },
                )
                .await;
                suggest_panel.update(|panel| {
                    // 待っている間に閉じた・別のポストで開き直した場合は捨てる
                    let Some(p) = panel
                        .as_mut()
                        .filter(|p| p.post_id == post_id && p.state == SuggestState::Loading)
                    else {
                        return;
                    };
                    match result {
                        Ok(list) => {
                            p.state = SuggestState::Ready;
                            p.candidates = list
                                .into_iter()
                                .map(|s| Candidate {
                                    text: s.text,
                                    task_id: s.task_id,
                                    checked: false,
                                })
                                .collect();
                        }
                        Err(e) => p.state = SuggestState::Failed(e),
                    }
                });
            });
        });

        // チェックした候補を1件ずつ投稿する。失敗したものだけを残してエラーを出す
        let on_post_suggestions = Callback::new(move |_: ()| {
            let Some(panel) = suggest_panel.get_untracked() else {
                return;
            };
            let chosen: Vec<Candidate> = panel
                .candidates
                .iter()
                .filter(|c| c.checked && c.is_valid())
                .cloned()
                .collect();
            if panel.posting || chosen.is_empty() {
                return;
            }
            suggest_panel.update(|p| {
                if let Some(p) = p {
                    p.posting = true;
                    p.error = None;
                }
            });
            spawn_local(async move {
                let mut failed = Vec::new();
                let mut last_error = None;
                for c in chosen {
                    let args = CreateDerivedPostArgs {
                        body: c.text.clone(),
                        parent_task_id: c.task_id,
                    };
                    match invoke::<_, Post>("create_post", &args).await {
                        Ok(post) => on_posted.run(post),
                        Err(e) => {
                            last_error = Some(e);
                            failed.push(c);
                        }
                    }
                }
                let post_id = panel.post_id;
                if failed.is_empty() {
                    suggest_panel.update(|p| {
                        if p.as_ref().is_some_and(|p| p.post_id == post_id) {
                            *p = None;
                        }
                    });
                    return;
                }
                suggest_panel.update(|p| {
                    if let Some(p) = p.as_mut().filter(|p| p.post_id == post_id) {
                        p.posting = false;
                        p.error = Some(format!(
                            "{}件を投稿できませんでした: {}",
                            failed.len(),
                            last_error.unwrap_or_default()
                        ));
                        p.candidates = failed;
                    }
                });
            });
        });
        Self {
            posts,
            kinds,
            has_more,
            load_error,
            is_dev,
            draft,
            load_more,
            upsert: Callback::new(upsert),
            on_posted,
            on_retry,
            on_toggle_task,
            on_ask_ai,
            on_topic_created,
            on_remove_topic,
            derive_draft,
            on_open_derive,
            on_submit_derive,
            suggest_panel,
            on_suggest,
            on_post_suggestions,
        }
    }
}

/// アプリの起動時に1回だけ呼ぶ
pub fn provide_timeline_store() {
    provide_context(TimelineStore::new());
}

pub fn use_timeline_store() -> TimelineStore {
    expect_context::<TimelineStore>()
}
