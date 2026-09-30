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

    #[error("Operation failed: {0}")]
    General(String),
}

impl From<String> for NagisaError {
    fn from(s: String) -> Self {
        if s.contains("PASSWORD_REQUIRED") {
            NagisaError::PasswordRequired
        } else if s.contains("SIGNED_PDF_MUTATION_ERROR") || s.contains("署名") && s.contains("保護") {
            NagisaError::SignedPdfMutationBlocked(s)
        } else if s.contains("timed out") || s.contains("Timeout") {
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
