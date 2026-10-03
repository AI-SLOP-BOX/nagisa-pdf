use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Structured error type for Nagisa PDF operations.
/// Serializes to JSON for the frontend with code and message.
#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "details")]
pub enum NagisaError {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("PDF parsing error: {0}")]
    PdfParse(String),

    #[error("PDF is password protected")]
    PasswordRequired,

    #[error("Invalid password provided")]
    InvalidPassword,

    #[error("Cryptographic signature present: direct mutations would corrupt signatures ({0})")]
    SignedPdfMutationBlocked(String),

    #[error("External tool missing: {0}")]
    ExternalToolMissing(String),

    #[error("External process execution error: {0}")]
    ExternalProcessError(String),

    #[error("Operation timeout: {0}")]
    Timeout(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Session not found or expired: {0}")]
    SessionNotFound(String),

    #[error("Operation failed: {0}")]
    General(String),
}

impl From<String> for NagisaError {
    fn from(s: String) -> Self {
        if s.contains("PASSWORD_REQUIRED") {
            NagisaError::PasswordRequired
        } else if s.contains("SIGNED_PDF_MUTATION_ERROR")
            || (s.contains("署名") && s.contains("保護"))
        {
            NagisaError::SignedPdfMutationBlocked(s)
        } else if s.contains("timed out")
            || s.contains("Timeout")
            || s.contains("タイムアウト")
            || s.contains("制限時間")
        {
            NagisaError::Timeout(s)
        } else {
            NagisaError::General(s)
        }
    }
}

impl From<&str> for NagisaError {
    fn from(s: &str) -> Self {
        NagisaError::from(s.to_string())
    }
}

impl From<std::io::Error> for NagisaError {
    fn from(e: std::io::Error) -> Self {
        NagisaError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for NagisaError {
    fn from(e: serde_json::Error) -> Self {
        NagisaError::General(format!("JSON serialization error: {e}"))
    }
}

impl From<tokio::task::JoinError> for NagisaError {
    fn from(e: tokio::task::JoinError) -> Self {
        NagisaError::General(format!("Task execution panicked or cancelled: {e}"))
    }
}

impl From<lopdf::Error> for NagisaError {
    fn from(e: lopdf::Error) -> Self {
        NagisaError::PdfParse(format!("PDF parsing failed: {e}"))
    }
}

impl<T> From<std::sync::PoisonError<T>> for NagisaError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        NagisaError::General(format!("Internal lock poisoned: {e}"))
    }
}

impl NagisaError {
    /// 人間可読メッセージ（Display と同一）。テストやログでの内容検証用。
    /// IPC 越しには `details` フィールドを使うこと。
    pub fn message(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_to_tauri_ipc_shape() {
        // Tauri は Err を JSON 化してフロントへ渡す。フロントの
        // parseNagisaError が読む { type, details } 形を保証する。
        let e = NagisaError::InvalidParameter("bad page".to_string());
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "InvalidParameter");
        assert_eq!(v["details"], "bad page");
        let unit = serde_json::to_value(&NagisaError::PasswordRequired).unwrap();
        assert_eq!(unit["type"], "PasswordRequired");
    }

    #[test]
    fn string_sentinels_classify_correctly() {
        assert!(matches!(
            NagisaError::from("PASSWORD_REQUIRED".to_string()),
            NagisaError::PasswordRequired
        ));
        assert!(matches!(
            NagisaError::from("SIGNED_PDF_MUTATION_ERROR: x".to_string()),
            NagisaError::SignedPdfMutationBlocked(_)
        ));
        assert!(matches!(
            NagisaError::from("plain failure".to_string()),
            NagisaError::General(_)
        ));
    }
}
