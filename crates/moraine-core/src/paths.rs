use std::path::PathBuf;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct MorainePaths {
    pub data_dir: PathBuf,
    pub history_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl MorainePaths {
    pub fn default_ensure() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| Error::other("could not resolve user data directory"))?
            .join("moraine");
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::other("could not resolve user config directory"))?
            .join("moraine");
        let history_dir = data_dir.join("history");

        std::fs::create_dir_all(&history_dir)?;
        std::fs::create_dir_all(&config_dir)?;

        Ok(Self {
            data_dir,
            history_dir,
            config_dir,
        })
    }

    /// Stable history filename from absolute path (DefaultHasher, not cryptographic).
    pub fn history_file_for(&self, absolute_path: &std::path::Path) -> PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        absolute_path.hash(&mut hasher);
        let id = format!("{:016x}", hasher.finish());
        self.history_dir.join(format!("{id}.json"))
    }
}
