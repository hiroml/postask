use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("データベースエラー: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("{0}")]
    Validation(String),

    #[error("ポストが見つかりません (id={0})")]
    PostNotFound(i64),

    #[error("{0}")]
    Other(String),
}

// Tauri コマンドのエラーはフロントにメッセージ文字列として渡す
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
