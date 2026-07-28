use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct MorainePaths {
    pub data_dir: PathBuf,
    pub history_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl MorainePaths {
    pub fn default_ensure() -> Result<Self> {
        let users = moraine_platform::UserPaths::discover();
        Self::from_user_paths_ensure(&users)
    }

    pub fn from_user_paths_ensure(users: &moraine_platform::UserPaths) -> Result<Self> {
        let data_dir = users.data_dir.join("moraine");
        let config_dir = users.config_dir.join("moraine");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_domain_paths_from_injected_platform_user_paths() {
        let dir = tempfile::tempdir().unwrap();
        let users = moraine_platform::UserPaths {
            data_dir: dir.path().join("data"),
            config_dir: dir.path().join("config"),
            cache_dir: dir.path().join("cache"),
            runtime_dir: dir.path().join("runtime"),
        };

        let paths = MorainePaths::from_user_paths_ensure(&users).unwrap();

        assert_eq!(paths.data_dir, users.data_dir.join("moraine"));
        assert_eq!(paths.config_dir, users.config_dir.join("moraine"));
        assert_eq!(paths.history_dir, users.data_dir.join("moraine/history"));
        assert!(paths.history_dir.is_dir());
        assert!(paths.config_dir.is_dir());
    }
}
