use std::path::PathBuf;

use crate::async_git::GitEvent;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    // -- Navigation --
    GoToSetup,
    GoToDashboard(usize),

    // -- Setup screen --
    UrlChanged(String),
    DestChanged(String),
    PickDestDir,
    DestDirPicked(Option<PathBuf>),
    Clone,

    // -- Add local repo --
    PickLocalRepo,
    LocalRepoPicked(Option<PathBuf>),
    TrackedRepoFilterChanged(String),
    RemoveRepository(usize),

    // -- Dashboard --
    Sync,
    OpenRepoFolder,

    // -- Git operation streaming --
    GitOp(GitEvent),

    // -- Worktree management --
    RefreshWorktrees,
    WorktreesLoaded(Result<Vec<gitgobig_core::Worktree>, String>),
    BranchesLoaded(Result<Vec<String>, String>),
    WtPathChanged(String),
    PickWtDir,
    WtDirPicked(Option<PathBuf>),
    WtBranchFilterChanged(String),
    WtBranchSelected(String),
    WtNewBranchToggled(bool),
    WtNewBranchNameChanged(String),
    AddWorktree,
    RemoveWorktree(PathBuf),
    OpenWorktreeFolder(PathBuf),
    ExploreWorktree(PathBuf),

    // -- Worktree list filter --
    WtListFilterChanged(String),

    // -- Confirmation dialog --
    ConfirmAction,
    CancelAction,

    // -- Clipboard --
    CopyToClipboard(String),
    ClipboardRead(Option<String>),

    // -- Settings --
    DefaultRepoDirChanged(String),
    PickDefaultRepoDir,
    DefaultRepoDirPicked(Option<PathBuf>),
    ClearDefaultRepoDir,

    // -- Progress overlay --
    DismissProgress,

    // -- Error --
    ToggleErrorDetail,
    DismissError,
}
