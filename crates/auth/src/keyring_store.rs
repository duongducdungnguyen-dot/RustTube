use anyhow::Result;
use keyring::Entry;

pub trait TokenStorage: Send + Sync {
    fn save_refresh_token(&self, token: &str) -> Result<()>;
    fn get_refresh_token(&self) -> Result<Option<String>>;
    fn delete_refresh_token(&self) -> Result<()>;
}

pub struct SecureTokenStore {
    service_name: String,
    account_name: String,
}

impl SecureTokenStore {
    pub fn new(service_name: &str, account_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            account_name: account_name.to_string(),
        }
    }
}

impl TokenStorage for SecureTokenStore {
    fn save_refresh_token(&self, token: &str) -> Result<()> {
        let entry = Entry::new(&self.service_name, &self.account_name)?;
        entry.set_password(token)?;
        Ok(())
    }

    fn get_refresh_token(&self) -> Result<Option<String>> {
        let entry = Entry::new(&self.service_name, &self.account_name)?;
        match entry.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Keyring error: {}", e)),
        }
    }

    fn delete_refresh_token(&self) -> Result<()> {
        let entry = Entry::new(&self.service_name, &self.account_name)?;
        match entry.delete_password() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Keyring delete error: {}", e)),
        }
    }
}

#[derive(Default)]
pub struct MemoryTokenStore {
    token: std::sync::RwLock<Option<String>>,
}

impl MemoryTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_token(token: &str) -> Self {
        Self {
            token: std::sync::RwLock::new(Some(token.to_string())),
        }
    }
}

impl TokenStorage for MemoryTokenStore {
    fn save_refresh_token(&self, token: &str) -> Result<()> {
        let mut lock = self.token.write().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        *lock = Some(token.to_string());
        Ok(())
    }

    fn get_refresh_token(&self) -> Result<Option<String>> {
        let lock = self.token.read().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        Ok(lock.clone())
    }

    fn delete_refresh_token(&self) -> Result<()> {
        let mut lock = self.token.write().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        *lock = None;
        Ok(())
    }
}
