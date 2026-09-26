//! `.env` と環境変数から設定を読む（Q-004）。

/// 分類に使うモデルの既定値（Q-007）
pub const DEFAULT_MODEL: &str = "claude-haiku-4-5";
/// リプライに使うモデルの既定値（Q-016）
pub const DEFAULT_REPLY_MODEL: &str = "claude-haiku-4-5";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// 未設定なら None。分類・リプライは「API キーが設定されていません」で失敗する。
    pub api_key: Option<String>,
    /// 分類に使うモデル（`CLAUDE_MODEL`）
    pub model: String,
    /// リプライに使うモデル（`CLAUDE_REPLY_MODEL`、仕様003）
    pub reply_model: String,
}

impl Config {
    /// `.env` を読み込んでから環境変数を見る。
    /// `.env` はカレントディレクトリから親へさかのぼって探すので、
    /// `cargo tauri dev`（src-tauri で起動）でもプロジェクト直下の `.env` が読まれる。
    pub fn load() -> Self {
        let _ = dotenvy::dotenv();
        Self::from_values(
            std::env::var("ANTHROPIC_API_KEY").ok(),
            std::env::var("CLAUDE_MODEL").ok(),
            std::env::var("CLAUDE_REPLY_MODEL").ok(),
        )
    }

    fn from_values(
        api_key: Option<String>,
        model: Option<String>,
        reply_model: Option<String>,
    ) -> Self {
        let non_empty =
            |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Self {
            api_key: non_empty(api_key),
            model: non_empty(model).unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            reply_model: non_empty(reply_model).unwrap_or_else(|| DEFAULT_REPLY_MODEL.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_haiku_without_key() {
        let c = Config::from_values(None, None, None);
        assert_eq!(c.api_key, None);
        assert_eq!(c.model, DEFAULT_MODEL);
        assert_eq!(c.reply_model, DEFAULT_REPLY_MODEL);
    }

    #[test]
    fn blank_values_are_treated_as_unset() {
        let c = Config::from_values(Some("  ".into()), Some("".into()), Some(" ".into()));
        assert_eq!(c.api_key, None);
        assert_eq!(c.model, DEFAULT_MODEL);
        assert_eq!(c.reply_model, DEFAULT_REPLY_MODEL);
    }

    #[test]
    fn models_can_be_overridden_separately() {
        let c = Config::from_values(
            Some("sk-test".into()),
            Some("claude-opus-5".into()),
            Some("claude-sonnet-5".into()),
        );
        assert_eq!(c.api_key.as_deref(), Some("sk-test"));
        assert_eq!(c.model, "claude-opus-5");
        assert_eq!(c.reply_model, "claude-sonnet-5");
    }
}
