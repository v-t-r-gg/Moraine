//! Rebuildable user-level registry of known project roots.
//!
//! Run bundles remain canonical inside each project. This file stores only roots
//! needed to rediscover those bundles after desktop/service restart.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::error::{Error, Result};
use crate::paths::MorainePaths;

thread_local! {
    static REGISTRY_PATH_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRegistry {
    pub schema_version: u32,
    pub projects: Vec<RegisteredProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredProject {
    pub root: String,
    pub registered_at: String,
}

impl Default for ProjectRegistry {
    fn default() -> Self {
        Self {
            schema_version: 1,
            projects: Vec::new(),
        }
    }
}

pub fn default_project_registry_path() -> Result<PathBuf> {
    if let Some(path) = REGISTRY_PATH_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Ok(path);
    }
    Ok(MorainePaths::default_ensure()?
        .data_dir
        .join("projects.json"))
}

/// Runs a synchronous operation with an isolated project-registry path.
///
/// This is primarily useful for hermetic embedding and integration tests that
/// cannot safely mutate process-wide user-data environment variables.
#[doc(hidden)]
pub fn with_project_registry_path_override<T>(path: PathBuf, operation: impl FnOnce() -> T) -> T {
    struct Reset(Option<PathBuf>);

    impl Drop for Reset {
        fn drop(&mut self) {
            REGISTRY_PATH_OVERRIDE.with(|slot| {
                *slot.borrow_mut() = self.0.take();
            });
        }
    }

    let previous = REGISTRY_PATH_OVERRIDE.with(|slot| slot.replace(Some(path)));
    let _reset = Reset(previous);
    operation()
}

pub fn register_project_root(root: &Path) -> Result<PathBuf> {
    let registry = default_project_registry_path()?;
    register_project_root_at(&registry, root)?;
    Ok(registry)
}

pub fn register_project_root_at(registry_path: &Path, root: &Path) -> Result<()> {
    let canonical = std::fs::canonicalize(root).map_err(Error::Io)?;
    let canonical_text = canonical.display().to_string();
    let mut registry = read_project_registry_at(registry_path)?;
    if !registry
        .projects
        .iter()
        .any(|entry| entry.root == canonical_text)
    {
        registry.projects.push(RegisteredProject {
            root: canonical_text,
            registered_at: Utc::now().to_rfc3339(),
        });
        registry.projects.sort_by(|a, b| a.root.cmp(&b.root));
        let bytes = serde_json::to_vec_pretty(&registry)?;
        write_atomic(registry_path, &bytes)?;
    }
    Ok(())
}

pub fn read_project_registry() -> Result<ProjectRegistry> {
    read_project_registry_at(&default_project_registry_path()?)
}

pub fn read_project_registry_at(path: &Path) -> Result<ProjectRegistry> {
    if !path.exists() {
        return Ok(ProjectRegistry::default());
    }
    let bytes = std::fs::read(path)?;
    let registry: ProjectRegistry = serde_json::from_slice(&bytes)?;
    if registry.schema_version != 1 {
        return Err(Error::other(format!(
            "unsupported project registry schema {}",
            registry.schema_version
        )));
    }
    Ok(registry)
}

pub fn registered_project_roots() -> Result<Vec<PathBuf>> {
    Ok(read_project_registry()?
        .projects
        .into_iter()
        .map(|entry| PathBuf::from(entry.root))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_survives_reload_and_deduplicates_canonical_roots() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let registry = dir.path().join("data/projects.json");

        register_project_root_at(&registry, &project).unwrap();
        register_project_root_at(&registry, &project.join(".")).unwrap();

        let reloaded = read_project_registry_at(&registry).unwrap();
        assert_eq!(reloaded.projects.len(), 1);
        assert_eq!(
            PathBuf::from(&reloaded.projects[0].root),
            std::fs::canonicalize(project).unwrap()
        );
    }

    #[test]
    fn missing_registered_project_remains_diagnosable() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let registry = dir.path().join("projects.json");
        register_project_root_at(&registry, &project).unwrap();
        std::fs::remove_dir(&project).unwrap();

        let reloaded = read_project_registry_at(&registry).unwrap();
        assert_eq!(reloaded.projects.len(), 1);
        assert!(!Path::new(&reloaded.projects[0].root).exists());
    }
}
