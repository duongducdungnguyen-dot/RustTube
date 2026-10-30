use anyhow::Result;
use keyring::Entry;

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

    pub fn save_refresh_token(&self, token: &str) -> Result<()> {
        let entry = Entry::new(&self.service_name, &self.account_name)?;
        entry.set_password(token)?;
        Ok(())
    }

    pub fn get_refresh_token(&self) -> Result<Option<String>> {
        let entry = Entry::new(&self.service_name, &self.account_name)?;
        match entry.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Keyring error: {}", e)),
        }
    }

    pub fn delete_refresh_token(&self) -> Result<()> {
        let entry = Entry::new(&self.service_name, &self.account_name)?;
        match entry.delete_password() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Keyring delete error: {}", e)),
        }
    }
}
