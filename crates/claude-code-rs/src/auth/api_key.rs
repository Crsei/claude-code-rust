//! API key validation and storage
//!
//! Supports:
//! - Format validation (sk-ant-* prefix)
//! - System keychain storage
//! - Environment variable (`ANTHROPIC_API_KEY`)

use anyhow::Result;

/// Validate an API key format.
///
/// Valid keys start with `sk-ant-` and are longer than 20 characters.
pub fn validate_api_key(key: &str) -> bool {
    key.starts_with("sk-ant-") && key.len() > 20
}

/// Store API key to the system keychain.
pub fn store_api_key(key: &str) -> Result<()> {
    let entry = keyring::Entry::new("cc-rust", "api-key")?;
    entry.set_password(key)?;
    Ok(())
}

/// Load API key from the system keychain.
///
/// Returns `Ok(None)` if no key is stored.
pub fn load_api_key() -> Result<Option<String>> {
    let entry = keyring::Entry::new("cc-rust", "api-key")?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Remove API key from the system keychain.
///
/// Used by the `/logout` command.
pub fn remove_api_key() -> Result<()> {
    let entry = keyring::Entry::new("cc-rust", "api-key")?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // already gone
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_api_key() {
        assert!(validate_api_key("sk-ant-api03-abcdefghijklmnop"));
        assert!(validate_api_key("sk-ant-xxxxxxxxxxxxxxxxxxxx1"));
    }

    #[test]
    fn test_invalid_api_key() {
        assert!(!validate_api_key("sk-ant-short"));
        assert!(!validate_api_key("wrong-prefix-abcdefghijklmnop"));
        assert!(!validate_api_key(""));
    }
}
