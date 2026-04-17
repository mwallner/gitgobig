use std::fs;

use anyhow::{Context, Result};
use gitgobig_core::AppState;

use crate::paths::config_dir;

const STATE_FILE: &str = "state.json";

fn state_path() -> Result<std::path::PathBuf> {
    Ok(config_dir()?.join(STATE_FILE))
}

/// Load the persisted application state from disk.
/// Returns `AppState::default()` if the file does not yet exist.
pub fn load_state() -> Result<AppState> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(AppState::default());
    }
    let data = fs::read_to_string(&path)
        .with_context(|| format!("failed to read state file: {}", path.display()))?;
    let state: AppState = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse state file: {}", path.display()))?;
    Ok(state)
}

/// Save the application state to disk as pretty-printed JSON.
/// Creates the config directory if it does not exist.
pub fn save_state(state: &AppState) -> Result<()> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(state).context("failed to serialize state")?;
    fs::write(&path, json.as_bytes())
        .with_context(|| format!("failed to write state file: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gitgobig_core::{AppState, Repository};

    /// Round-trip test using a temp directory.
    #[test]
    fn roundtrip_state() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state.json");

        let state = AppState {
            repositories: vec![Repository {
                name: "test-repo".into(),
                path: PathBuf::from("/tmp/test.git"),
                url: "https://example.com/repo.git".into(),
                worktrees: vec![],
            }],
            default_repo_dir: None,
        };

        // Write
        let json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&file, json.as_bytes()).unwrap();

        // Read back
        let data = std::fs::read_to_string(&file).unwrap();
        let loaded: AppState = serde_json::from_str(&data).unwrap();

        assert_eq!(state, loaded);
    }
}
