use std::path::PathBuf;

use gitgobig_core::git;
use gitgobig_core::CommitEntry;
use iced::Event;

use crate::style::ResizeHandle;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    CommitsLoaded(Result<Vec<CommitEntry>, String>),
    LoadMore,
    SelectCommit(usize),
    ShowContextMenu(usize),
    DismissContextMenu,
    CopyHash(String),
    InspectCommit(String),
    InspectLoaded(Result<String, String>),
    DismissInspect,
    DragStart(ResizeHandle),
    GlobalEvent(Event),
    SearchDepth(String),
    SearchHash(String),
    SearchMessage(String),
    SearchDate(String),
    SearchAuthor(String),
    ToggleRegex(bool),
    ClearSearch,
    SearchCommitsLoaded(Result<Vec<CommitEntry>, String>),
    BranchesLoaded(Result<Vec<String>, String>),
    ToggleBranchDropdown,
    DismissBranchDropdown,
    BranchFilterText(String),
    ToggleBranch(String),
    SelectAllBranches,
    DeselectAllBranches,
    BranchSelectionChanged,
    BranchCommitsLoaded(Result<Vec<CommitEntry>, String>),
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

pub(crate) async fn load_commits(
    repo_path: PathBuf,
    skip: usize,
    count: usize,
) -> Result<Vec<CommitEntry>, String> {
    tokio::task::spawn_blocking(move || git::log_graph(&repo_path, count, skip))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

pub(crate) async fn load_detail(
    repo_path: PathBuf,
    hash: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || git::log_detail(&repo_path, &hash))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

pub(crate) async fn load_branches(repo_path: PathBuf) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || git::branch_list(&repo_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

pub(crate) async fn load_commits_for_branches(
    repo_path: PathBuf,
    skip: usize,
    count: usize,
    branches: Vec<String>,
) -> Result<Vec<CommitEntry>, String> {
    tokio::task::spawn_blocking(move || {
        git::log_graph_branches(&repo_path, count, skip, &branches)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
