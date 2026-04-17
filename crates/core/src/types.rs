use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A bare-cloned git repository managed by gitgobig.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Repository {
    /// Display name (typically derived from the URL).
    pub name: String,
    /// Local path to the bare repository.
    pub path: PathBuf,
    /// Remote URL the repo was cloned from.
    pub url: String,
    /// Worktrees created from this bare repo.
    pub worktrees: Vec<Worktree>,
}

/// A git worktree checked out from a bare repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Worktree {
    /// Local filesystem path of the worktree.
    pub path: PathBuf,
    /// Branch name, if the worktree is on a branch.
    pub branch: Option<String>,
    /// HEAD commit hash.
    pub commit: Option<String>,
}

/// A single commit from `git log` output.
#[derive(Debug, Clone)]
pub struct CommitEntry {
    /// Full SHA hash.
    pub hash: String,
    /// Abbreviated SHA hash.
    pub short_hash: String,
    /// First line of commit message.
    pub subject: String,
    /// ISO 8601 author date.
    pub date: String,
    /// Author name.
    pub author: String,
    /// Decorated ref names (branches, tags).
    pub refs: String,
    /// Parent commit hashes (full SHA).
    pub parents: Vec<String>,
}

/// Top-level application state persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppState {
    pub repositories: Vec<Repository>,
    /// Default base directory for cloned repositories.
    #[serde(default)]
    pub default_repo_dir: Option<PathBuf>,
}
