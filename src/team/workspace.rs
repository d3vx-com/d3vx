//! File-based persistence for team state.
//!
//! Each team gets a directory under `.d3vx/teams/{name}/` containing
//! a `manifest.json` that tracks membership and metadata.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tracing::debug;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Persistent team manifest serialized as JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamManifest {
    pub name: String,
    pub description: String,
    /// ISO 3339 timestamp of creation.
    pub created_at: String,
    pub lead_call_sign: Option<String>,
    pub members: Vec<MemberEntry>,
    pub workspace_root: String,
}

/// A member entry in the manifest (lighter than an in-memory descriptor).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemberEntry {
    pub agent_id: String,
    pub call_sign: String,
    /// Serde string representation of `AgentType`.
    pub agent_type_key: String,
    /// Serde string representation of `AgentRole`.
    pub role_key: String,
    /// One of "idle", "working", "done", "failed".
    pub status: String,
}

// ---------------------------------------------------------------------------
// TeamWorkspace
// ---------------------------------------------------------------------------

/// Manages file-based team state under `.d3vx/teams/{name}/`.
pub struct TeamWorkspace {
    root: PathBuf,
}

impl TeamWorkspace {
    /// Compute the workspace path as `{working_dir}/.d3vx/teams/{team_name}/`.
    pub fn new(working_dir: &str, team_name: &str) -> Self {
        let mut root = PathBuf::from(working_dir);
        root.push(".d3vx");
        root.push("teams");
        root.push(team_name);
        Self { root }
    }

    /// Absolute path to `manifest.json`.
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    /// Whether the team directory and manifest already exist.
    pub fn exists(&self) -> bool {
        self.manifest_path().is_file()
    }

    /// Create the team directory and write the initial manifest.
    pub fn create(&self, manifest: &TeamManifest) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create team dir: {}", self.root.display()))?;

        self.write_manifest(manifest)?;
        debug!(team = %manifest.name, "created team workspace");
        Ok(())
    }

    /// Read and deserialize the manifest from disk.
    pub fn load(&self) -> Result<TeamManifest> {
        let raw = fs::read_to_string(self.manifest_path()).with_context(|| {
            format!(
                "failed to read manifest: {}",
                self.manifest_path().display()
            )
        })?;
        let manifest: TeamManifest =
            serde_json::from_str(&raw).with_context(|| "failed to deserialize manifest")?;
        debug!(team = %manifest.name, "loaded team manifest");
        Ok(manifest)
    }

    /// Overwrite the manifest with an updated version.
    pub fn update(&self, manifest: &TeamManifest) -> Result<()> {
        if !self.exists() {
            anyhow::bail!("cannot update non-existent team: {}", manifest.name);
        }
        self.write_manifest(manifest)?;
        debug!(team = %manifest.name, "updated team manifest");
        Ok(())
    }

    /// Remove the entire team directory.
    pub fn delete(&self) -> Result<()> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)
                .with_context(|| format!("failed to delete team dir: {}", self.root.display()))?;
            debug!(path = %self.root.display(), "deleted team workspace");
        }
        Ok(())
    }

    // -- helpers -----------------------------------------------------------

    fn write_manifest(&self, manifest: &TeamManifest) -> Result<()> {
        let json = serde_json::to_string_pretty(manifest)
            .with_context(|| "failed to serialize manifest")?;
        fs::write(self.manifest_path(), json).with_context(|| {
            format!(
                "failed to write manifest: {}",
                self.manifest_path().display()
            )
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(team_name: &str, root: &str) -> TeamManifest {
        TeamManifest {
            name: team_name.to_string(),
            description: "test team".to_string(),
            created_at: "2026-03-30T00:00:00Z".to_string(),
            lead_call_sign: Some("alpha".to_string()),
            members: vec![MemberEntry {
                agent_id: "agent-1".to_string(),
                call_sign: "alpha".to_string(),
                agent_type_key: "Coder".to_string(),
                role_key: "Lead".to_string(),
                status: "idle".to_string(),
            }],
            workspace_root: root.to_string(),
        }
    }

    #[test]
    fn create_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = TeamWorkspace::new(dir.path().to_str().unwrap(), "swarm-1");
        let original = sample_manifest("swarm-1", dir.path().to_str().unwrap());

        ws.create(&original).expect("create");
        let loaded = ws.load().expect("load");

        assert_eq!(loaded.name, original.name);
        assert_eq!(loaded.description, original.description);
        assert_eq!(loaded.members.len(), 1);
        assert_eq!(loaded.members[0].call_sign, "alpha");
    }

    #[test]
    fn update_persists_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = TeamWorkspace::new(dir.path().to_str().unwrap(), "swarm-2");
        let mut manifest = sample_manifest("swarm-2", dir.path().to_str().unwrap());
        ws.create(&manifest).expect("create");

        manifest.description = "updated description".to_string();
        manifest.members[0].status = "working".to_string();
        ws.update(&manifest).expect("update");

        let loaded = ws.load().expect("load");
        assert_eq!(loaded.description, "updated description");
        assert_eq!(loaded.members[0].status, "working");
    }

    #[test]
    fn delete_removes_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = TeamWorkspace::new(dir.path().to_str().unwrap(), "swarm-3");
        let manifest = sample_manifest("swarm-3", dir.path().to_str().unwrap());
        ws.create(&manifest).expect("create");

        assert!(ws.exists());
        ws.delete().expect("delete");
        assert!(!ws.exists());
        assert!(!ws.root.exists());
    }

    #[test]
    fn exists_false_for_missing_team() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = TeamWorkspace::new(dir.path().to_str().unwrap(), "ghost");
        assert!(!ws.exists());
    }
}
