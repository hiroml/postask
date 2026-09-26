//! 課題ページの集計（仕様006）。
//!
//! 日ごと・週ごとの区切りをパソコンのタイムゾーンで数えるため（FR-18）、タスクは SQL でまとめて読み、
//! 期間・区切りの計算は Rust で行う。現在時刻とタイムゾーンを引数で受け取り、日付の境目をテストできるようにしている。

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::AppResult;
use crate::kind::{TaskKind, KINDS};
use crate::models::{ParentRef, Task, TaskStatus};

/// 一覧に出す最大件数（FR-16）
pub const MAX_TASK_ROWS: usize = 500;

/// 期間（FR-1）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Period {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "7d")]
    Days7,
    #[serde(rename = "30d")]
    Days30,
    #[serde(rename = "90d")]
    Days90,
}

impl Period {
    fn days(self) -> Option<i64> {
        match self {
            Period::All => None,
            Period::Days7 => Some(7),
            Period::Days30 => Some(30),
            Period::Days90 => Some(90),
        }
    }

    /// 7日・30日は日ごと、90日・全期間は週ごと（FR-9）
    fn bucket(self) -> Bucket {
        match self {
            Period::Days7 | Period::Days30 => Bucket::Day,
            Period::Days90 | Period::All => Bucket::Week,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Day,
    Week,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub open: u32,
    pub done: u32,
    /// 完了 ÷ 対象（%、切り捨て）。対象が0件なら None
    pub completion_rate: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KindCount {
    pub kind: TaskKind,
    pub open: u32,
    pub done: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrendPoint {
    /// 日（週）の最初の日（YYYY-MM-DD）
    pub start: String,
    pub created: u32,
    pub completed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trend {
    pub bucket: Bucket,
    pub points: Vec<TrendPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskRow {
    pub task: Task,
    pub post_body: String,
    /// 派生ポストのタスクなら、元のタスク（仕様009 FR-6）
    pub parent: Option<ParentRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskDashboard {
    pub totals: Totals,
    pub by_kind: Vec<KindCount>,
    pub trend: Trend,
    pub tasks: Vec<TaskRow>,
    /// 一覧が上限（500件）を超えて切り詰めたか
    pub truncated: bool,
}

/// RFC 3339 の日時を、指定のタイムゾーンの日時にする（読めなければ None）
fn parse_in<Tz: TimeZone>(s: &str, tz: &Tz) -> Option<DateTime<Tz>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(tz))
}

fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

fn bucket_of(date: NaiveDate, bucket: Bucket) -> NaiveDate {
    match bucket {
        Bucket::Day => date,
        Bucket::Week => week_start(date),
    }
}

/// 集計する。`now` のタイムゾーンで日付を数える。
pub fn build<Tz: TimeZone>(
    rows: Vec<(Task, String)>,
    period: Period,
    kind: Option<TaskKind>,
    status: Option<TaskStatus>,
    now: DateTime<Tz>,
) -> TaskDashboard {
    let tz = now.timezone();
    let today = now.date_naive();
    // 期間の最初の日（直近7日 = 今日を含む7日間）
    let first_day = period.days().map(|d| today - Duration::days(d - 1));
    let in_period = |date: NaiveDate| first_day.is_none_or(|first| date >= first);

    // 種別・状態の絞り込み
    let matching: Vec<(Task, String, NaiveDate)> = rows
        .into_iter()
        .filter(|(t, _)| t.kind.creates_task())
        .filter(|(t, _)| kind.is_none_or(|k| t.kind == k))
        .filter(|(t, _)| status.is_none_or(|s| t.status == s))
        .filter_map(|(t, body)| {
            let created = parse_in(&t.created_at, &tz)?.date_naive();
            Some((t, body, created))
        })
        .collect();

    // 対象タスク = 作成日が期間内のもの
    let targets: Vec<&(Task, String, NaiveDate)> = matching
        .iter()
        .filter(|(_, _, created)| in_period(*created))
        .collect();

    let count =
        |pred: &dyn Fn(&Task) -> bool| targets.iter().filter(|(t, _, _)| pred(t)).count() as u32;
    let open = count(&|t| t.status == TaskStatus::Open);
    let done = count(&|t| t.status == TaskStatus::Done);
    let total = open + done;
    let totals = Totals {
        open,
        done,
        completion_rate: (total > 0).then(|| done * 100 / total),
    };

    let by_kind = KINDS
        .iter()
        .filter(|k| k.creates_task)
        .map(|k| KindCount {
            kind: k.kind,
            open: count(&|t| t.kind == k.kind && t.status == TaskStatus::Open),
            done: count(&|t| t.kind == k.kind && t.status == TaskStatus::Done),
        })
        .collect();

    let trend = build_trend(&matching, period, first_day, today, &tz);

    // 一覧: 未完了が先（新しい順）、完了は完了日の新しい順（FR-14）
    let mut list: Vec<&(Task, String, NaiveDate)> = targets;
    list.sort_by(|(a, _, _), (b, _, _)| {
        let key = |t: &Task| match t.status {
            TaskStatus::Open => (0, t.created_at.clone()),
            TaskStatus::Done => (1, t.completed_at.clone().unwrap_or_default()),
        };
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0).then(kb.1.cmp(&ka.1)).then(b.id.cmp(&a.id))
    });
    let truncated = list.len() > MAX_TASK_ROWS;
    let tasks = list
        .into_iter()
        .take(MAX_TASK_ROWS)
        .map(|(t, body, _)| TaskRow {
            task: t.clone(),
            post_body: body.clone(),
            parent: None,
        })
        .collect();

    TaskDashboard {
        totals,
        by_kind,
        trend,
        tasks,
        truncated,
    }
}

fn build_trend<Tz: TimeZone>(
    matching: &[(Task, String, NaiveDate)],
    period: Period,
    first_day: Option<NaiveDate>,
    today: NaiveDate,
    tz: &Tz,
) -> Trend {
    let bucket = period.bucket();
    // 全期間は、最初のタスクがある週から
    let Some(first) = first_day.or_else(|| matching.iter().map(|(_, _, c)| *c).min()) else {
        return Trend {
            bucket,
            points: Vec::new(),
        };
    };

    let step = match bucket {
        Bucket::Day => Duration::days(1),
        Bucket::Week => Duration::weeks(1),
    };
    let mut starts = Vec::new();
    let mut d = bucket_of(first, bucket);
    while d <= today {
        starts.push(d);
        d += step;
    }

    let in_range = |date: NaiveDate| date >= first && date <= today;
    let mut points: Vec<TrendPoint> = starts
        .iter()
        .map(|s| TrendPoint {
            start: s.format("%Y-%m-%d").to_string(),
            created: 0,
            completed: 0,
        })
        .collect();
    let index_of = |date: NaiveDate| starts.iter().position(|s| *s == bucket_of(date, bucket));

    for (t, _, created) in matching {
        if in_range(*created) {
            if let Some(i) = index_of(*created) {
                points[i].created += 1;
            }
        }
        if t.status == TaskStatus::Done {
            if let Some(done) = t.completed_at.as_deref().and_then(|s| parse_in(s, tz)) {
                let done = done.date_naive();
                if in_range(done) {
                    if let Some(i) = index_of(done) {
                        points[i].completed += 1;
                    }
                }
            }
        }
    }

    Trend { bucket, points }
}

/// DB から読み、パソコンのタイムゾーンで集計する
pub fn get_task_dashboard(
    conn: &Connection,
    period: Period,
    kind: Option<TaskKind>,
    status: Option<TaskStatus>,
) -> AppResult<TaskDashboard> {
    let mut rows = db::all_tasks_with_post_body(conn)?;
    let children = db::all_children(conn)?;
    for (t, _) in &mut rows {
        t.children = children.get(&t.id).copied().unwrap_or_default();
    }
    let mut dashboard = build(rows, period, kind, status, chrono::Local::now());
    let parents = db::all_parents(conn)?;
    for row in &mut dashboard.tasks {
        row.parent = parents.get(&row.task.id).cloned();
    }
    Ok(dashboard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn jst() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    /// 日本時間 2026-09-26（土）12:00 を「今」とする
    fn now() -> DateTime<FixedOffset> {
        jst().with_ymd_and_hms(2026, 9, 26, 12, 0, 0).unwrap()
    }

    /// 日本時間の日時を RFC 3339（UTC）で
    fn at(y: i32, m: u32, d: u32, h: u32) -> String {
        jst()
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
            .with_timezone(&chrono::Utc)
            .to_rfc3339()
    }

    fn task(id: i64, kind: TaskKind, created: String, completed: Option<String>) -> (Task, String) {
        (
            Task {
                id,
                post_id: id,
                kind,
                title: format!("タスク{id}"),
                target: String::new(),
                action: String::new(),
                status: if completed.is_some() {
                    TaskStatus::Done
                } else {
                    TaskStatus::Open
                },
                created_at: created,
                completed_at: completed,
                children: Default::default(),
            },
            format!("ポスト{id}"),
        )
    }

    fn sample() -> Vec<(Task, String)> {
        vec![
            task(1, TaskKind::Issue, at(2026, 9, 26, 9), None), // 今日
            task(
                2,
                TaskKind::Issue,
                at(2026, 9, 25, 9),
                Some(at(2026, 9, 26, 10)),
            ), // 昨日作成・今日完了
            task(3, TaskKind::Idea, at(2026, 9, 20, 9), None),  // 6日前（7日の範囲内）
            task(
                4,
                TaskKind::Research,
                at(2026, 9, 19, 9),
                Some(at(2026, 9, 21, 9)),
            ), // 7日前（7日の範囲外）
            task(
                5,
                TaskKind::Issue,
                at(2026, 8, 1, 9),
                Some(at(2026, 9, 24, 9)),
            ), // 古い（30日の範囲外）
        ]
    }

    #[test]
    fn all_period_totals_and_rate() {
        let d = build(sample(), Period::All, None, None, now());
        assert_eq!(
            d.totals,
            Totals {
                open: 2,
                done: 3,
                completion_rate: Some(60)
            }
        );
        assert_eq!(d.tasks.len(), 5);
        assert!(!d.truncated);
    }

    #[test]
    fn seven_days_includes_today_and_six_days_before() {
        let d = build(sample(), Period::Days7, None, None, now());
        let ids: Vec<i64> = d.tasks.iter().map(|r| r.task.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&3) && !ids.contains(&4));
        assert_eq!(
            d.totals,
            Totals {
                open: 2,
                done: 1,
                completion_rate: Some(33)
            }
        );
    }

    #[test]
    fn filters_by_kind_and_status() {
        let d = build(
            sample(),
            Period::All,
            Some(TaskKind::Issue),
            Some(TaskStatus::Done),
            now(),
        );
        let ids: Vec<i64> = d.tasks.iter().map(|r| r.task.id).collect();
        assert_eq!(ids, vec![2, 5]);
        // 種別で絞っても、タスクにする種別はすべて並ぶ（0件を含む）
        assert_eq!(d.by_kind.len(), 3);
        let idea = d.by_kind.iter().find(|k| k.kind == TaskKind::Idea).unwrap();
        assert_eq!((idea.open, idea.done), (0, 0));
    }

    #[test]
    fn by_kind_counts_open_and_done() {
        let d = build(sample(), Period::All, None, None, now());
        let issue = d
            .by_kind
            .iter()
            .find(|k| k.kind == TaskKind::Issue)
            .unwrap();
        assert_eq!((issue.open, issue.done), (1, 2));
    }

    #[test]
    fn empty_has_no_rate_and_no_points_for_all() {
        let d = build(vec![], Period::All, None, None, now());
        assert_eq!(d.totals.completion_rate, None);
        assert!(d.trend.points.is_empty());
        let d = build(vec![], Period::Days7, None, None, now());
        assert_eq!(d.trend.points.len(), 7);
    }

    #[test]
    fn daily_trend_for_seven_days() {
        let d = build(sample(), Period::Days7, None, None, now());
        assert_eq!(d.trend.bucket, Bucket::Day);
        assert_eq!(d.trend.points.first().unwrap().start, "2026-09-20");
        assert_eq!(d.trend.points.last().unwrap().start, "2026-09-26");
        let today = d.trend.points.last().unwrap();
        // 今日: タスク1を作成、タスク2を完了
        assert_eq!((today.created, today.completed), (1, 1));
        // 9/21 に完了したタスク4は、作成が範囲外でも完了数には数える
        let p21 = d
            .trend
            .points
            .iter()
            .find(|p| p.start == "2026-09-21")
            .unwrap();
        assert_eq!(p21.completed, 1);
    }

    #[test]
    fn weekly_trend_starts_on_monday_from_first_task() {
        let d = build(sample(), Period::All, None, None, now());
        assert_eq!(d.trend.bucket, Bucket::Week);
        // 最初のタスク（8/1 土）の週の月曜 = 7/27
        assert_eq!(d.trend.points.first().unwrap().start, "2026-07-27");
        // 今週の月曜 = 9/21
        let this_week = d.trend.points.last().unwrap();
        assert_eq!(this_week.start, "2026-09-21");
        // 今週: 作成 1,2 / 完了 2,4,5
        assert_eq!((this_week.created, this_week.completed), (2, 3));
    }

    #[test]
    fn day_boundary_uses_local_time() {
        // 日本時間 9/26 0:30 は UTC では 9/25 15:30。日本時間の「今日」に数える
        let created = jst()
            .with_ymd_and_hms(2026, 9, 26, 0, 30, 0)
            .unwrap()
            .with_timezone(&chrono::Utc)
            .to_rfc3339();
        let d = build(
            vec![task(1, TaskKind::Issue, created, None)],
            Period::Days7,
            None,
            None,
            now(),
        );
        assert_eq!(d.trend.points.last().unwrap().created, 1);
    }

    #[test]
    fn list_puts_open_first_then_done_by_completion() {
        let d = build(sample(), Period::All, None, None, now());
        let ids: Vec<i64> = d.tasks.iter().map(|r| r.task.id).collect();
        // 未完了: 1（9/26）, 3（9/20） / 完了: 2（9/26 完了）, 5（9/24）, 4（9/21）
        assert_eq!(ids, vec![1, 3, 2, 5, 4]);
        assert_eq!(d.tasks[0].post_body, "ポスト1");
    }

    #[test]
    fn list_is_capped() {
        let rows = (0..(MAX_TASK_ROWS as i64 + 3))
            .map(|i| task(i, TaskKind::Issue, at(2026, 9, 26, 9), None))
            .collect();
        let d = build(rows, Period::All, None, None, now());
        assert_eq!(d.tasks.len(), MAX_TASK_ROWS);
        assert!(d.truncated);
        assert_eq!(d.totals.open, MAX_TASK_ROWS as u32 + 3);
    }

    #[test]
    fn reads_from_db_with_post_body() {
        let mut conn = db::open_in_memory().unwrap();
        let post = db::insert_post(&conn, "READMEを書く").unwrap();
        db::replace_tasks(
            &mut conn,
            post.id,
            &[crate::models::ClassifiedTask {
                kind: TaskKind::Issue,
                title: "READMEを書く".into(),
                target: String::new(),
                action: "書く".into(),
            }],
        )
        .unwrap();
        let d = get_task_dashboard(&conn, Period::All, None, None).unwrap();
        assert_eq!(d.totals.open, 1);
        assert_eq!(d.tasks[0].post_body, "READMEを書く");
    }

    #[test]
    fn rows_show_parent_and_children() {
        let mut conn = db::open_in_memory().unwrap();
        let classified = |title: &str| crate::models::ClassifiedTask {
            kind: TaskKind::Issue,
            title: title.into(),
            target: String::new(),
            action: title.into(),
        };
        let post = db::insert_post(&conn, "READMEを書く").unwrap();
        db::replace_tasks(&mut conn, post.id, &[classified("READMEを書く")]).unwrap();
        let parent = db::get_post_with_tasks(&conn, post.id).unwrap().tasks[0].id;
        let child_post = db::insert_derived_post(&conn, "ライセンス", Some(parent)).unwrap();
        db::replace_tasks(
            &mut conn,
            child_post.id,
            &[classified("ライセンスを調べる")],
        )
        .unwrap();

        let d = get_task_dashboard(&conn, Period::All, None, None).unwrap();
        let row = |title: &str| d.tasks.iter().find(|r| r.task.title == title).unwrap();
        assert_eq!(row("READMEを書く").task.children.total, 1);
        assert_eq!(row("READMEを書く").parent, None);
        assert_eq!(
            row("ライセンスを調べる")
                .parent
                .as_ref()
                .map(|p| p.title.as_str()),
            Some("READMEを書く")
        );
    }
}
