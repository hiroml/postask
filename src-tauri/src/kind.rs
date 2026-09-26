//! タスク種別の定義。種別を増やすときはこのファイルだけを変更する（Q-005）。
//!
//! 表示名・色もここで持ち、フロントは `list_kinds` コマンドで受け取って表示する。

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Issue,
    Idea,
    Research,
    Note,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct KindInfo {
    pub kind: TaskKind,
    /// 画面に出す名前
    pub label: &'static str,
    /// チップの色
    pub color: &'static str,
    /// AI に渡す説明（分類の基準）
    #[serde(skip)]
    pub description: &'static str,
    /// false の種別はタスクとして保存しない
    pub creates_task: bool,
    /// AI リプライでの答え方（仕様003）。タスクにしない種別では使わない
    #[serde(skip)]
    pub reply_guide: &'static str,
}

pub const KINDS: &[KindInfo] = &[
    KindInfo {
        kind: TaskKind::Issue,
        label: "課題",
        color: "#dc2626",
        description: "やるべきこと・直すべきこと・作るもの。「〇〇を直す」「〇〇を書く」「〇〇を設定する」など、行動すれば完了するもの。",
        creates_task: true,
        reply_guide: "解決の方針と、最初にやること（3〜5ステップ）を示す。",
    },
    KindInfo {
        kind: TaskKind::Idea,
        label: "思い付き",
        color: "#ca8a04",
        description: "アイデア・提案・「〇〇したら面白いかも」のような、まだやると決まっていない思い付き。",
        creates_task: true,
        reply_guide: "実現のアイデアと、小さく試す方法を示す。",
    },
    KindInfo {
        kind: TaskKind::Research,
        label: "調査",
        color: "#2563eb",
        description: "調べたいこと・比較したいこと・知りたいこと。「〇〇を調べる」「〇〇の違いを知りたい」など。",
        creates_task: true,
        reply_guide: "調べ方（検索キーワード、見るべき情報源、比較の観点）と、分かっている範囲の概要を示す。Web 検索の結果があれば要点をまとめる。",
    },
    KindInfo {
        kind: TaskKind::Note,
        label: "メモ",
        color: "#64748b",
        description: "感想・つぶやき・記録など、行動につながらないもの。タスクにはしない。",
        creates_task: false,
        reply_guide: "",
    },
];

impl TaskKind {
    pub fn info(self) -> &'static KindInfo {
        KINDS
            .iter()
            .find(|k| k.kind == self)
            .expect("すべての TaskKind は KINDS に定義されている")
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TaskKind::Issue => "issue",
            TaskKind::Idea => "idea",
            TaskKind::Research => "research",
            TaskKind::Note => "note",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        KINDS.iter().map(|k| k.kind).find(|k| k.as_str() == s)
    }

    pub fn creates_task(self) -> bool {
        self.info().creates_task
    }
}

impl ToSql for TaskKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for TaskKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        TaskKind::parse(s).ok_or_else(|| FromSqlError::Other(format!("未知の種別: {s}").into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_info_and_roundtrips() {
        for info in KINDS {
            assert_eq!(info.kind.info().label, info.label);
            assert_eq!(TaskKind::parse(info.kind.as_str()), Some(info.kind));
        }
    }

    #[test]
    fn note_does_not_create_task() {
        assert!(!TaskKind::Note.creates_task());
        assert!(TaskKind::Issue.creates_task());
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert_eq!(TaskKind::parse("bug"), None);
    }
}
