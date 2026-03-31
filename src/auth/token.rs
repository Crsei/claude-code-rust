#![allow(unused)]
//! OAuth token management and refresh
use anyhow::Result;

/// Token storage location
pub fn token_file_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("credentials.json")
}

/// Stored token data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
}

/// Load stored token from disk
pub fn load_token() -> Result<Option<StoredToken>> {
    let path = token_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let token: StoredToken = serde_json::from_str(&content)?;
    Ok(Some(token))
}

/// Save token to disk
pub fn save_token(token: &StoredToken) -> Result<()> {
    let path = token_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(token)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Check if a stored token has expired
pub fn is_token_expired(token: &StoredToken) -> bool {
    if let Some(expires_at) = token.expires_at {
        let now = chrono::Utc::now().timestamp();
        now >= expires_at - 300 // 5 minute buffer
    } else {
        false
    }
}
