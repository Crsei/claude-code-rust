#![allow(unused)]
//! API key storage and validation
use anyhow::Result;

/// Validate an API key format
pub fn validate_api_key(key: &str) -> bool {
    key.starts_with("sk-ant-") && key.len() > 20
}

/// Store API key to the system keychain
pub fn store_api_key(_key: &str) -> Result<()> {
    #[cfg(feature = "auth")]
    {
        let entry = keyring::Entry::new("claude-code", "api-key")?;
        entry.set_password(_key)?;
        Ok(())
    }
    #[cfg(not(feature = "auth"))]
    {
        anyhow::bail!("Keychain storage requires 'auth' feature")
    }
}

/// Load API key from the system keychain
pub fn load_api_key() -> Result<Option<String>> {
    #[cfg(feature = "auth")]
    {
        let entry = keyring::Entry::new("claude-code", "api-key")?;
        match entry.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    #[cfg(not(feature = "auth"))]
    {
        Ok(None)
    }
}
