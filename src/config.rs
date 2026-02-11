use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where config lives: ~/.config/matrixtui/
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("matrixtui")
}

/// Where data lives: ~/.local/share/matrixtui/
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("matrixtui")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAccount {
    pub homeserver: String,
    pub user_id: String,
    /// Stored session token — avoids re-login
    pub access_token: String,
    pub device_id: String,
}

fn default_room_sort() -> String {
    "unread".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub accounts: Vec<SavedAccount>,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default = "default_room_sort")]
    pub room_sort: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_dir().join("config.json");
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.json");
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn add_account(&mut self, account: SavedAccount) {
        // Replace existing entry for same user_id, or add new
        if let Some(existing) = self
            .accounts
            .iter_mut()
            .find(|a| a.user_id == account.user_id)
        {
            *existing = account;
        } else {
            self.accounts.push(account);
        }
    }

    pub fn remove_account(&mut self, user_id: &str) {
        self.accounts.retain(|a| a.user_id != user_id);
    }
}
