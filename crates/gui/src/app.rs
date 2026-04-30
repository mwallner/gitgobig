use std::path::PathBuf;

use gitgobig_config::load_state;
use gitgobig_core::{AppState, Worktree};
use iced::{clipboard, Task, Theme};

use crate::async_git::GitResult;
use crate::message::Message;

pub(crate) struct App {
    pub(crate) screen: Screen,
    pub(crate) state: AppState,
    pub(crate) error: Option<ErrorInfo>,
    pub(crate) pending_confirmation: Option<PendingConfirmation>,
    pub(crate) running_operation: Option<RunningOperation>,
}

/// Tracks a git operation currently in progress (or just finished).
pub(crate) struct RunningOperation {
    /// Human-readable label, e.g. "Cloning…" or "Syncing…"
    pub(crate) label: String,
    /// Accumulated stdout/stderr output lines from the git process.
    pub(crate) output_lines: Vec<String>,
    /// Set when the operation finishes; None while still running.
    pub(crate) finished: Option<FinishedOp>,
}

/// Outcome stored when a streaming git operation completes.
pub(crate) enum FinishedOp {
    Success(GitResult),
    Failed,
}

#[derive(Debug, Clone)]
pub(crate) enum PendingConfirmation {
    RemoveWorktree {
        description: String,
        repo_path: PathBuf,
        wt_path: PathBuf,
    },
    RemoveRepository {
        description: String,
        index: usize,
    },
}

impl PendingConfirmation {
    pub(crate) fn description(&self) -> &str {
        match self {
            PendingConfirmation::RemoveWorktree { description, .. } => description,
            PendingConfirmation::RemoveRepository { description, .. } => description,
        }
    }
}

pub(crate) struct ErrorInfo {
    pub(crate) message: String,
    pub(crate) detail: Option<String>,
    pub(crate) show_detail: bool,
}

pub(crate) enum Screen {
    Setup(SetupScreen),
    Dashboard(DashboardState),
}

pub(crate) struct SetupScreen {
    pub(crate) url_input: String,
    pub(crate) dest_input: String,
    pub(crate) dest_auto_filled: bool,
    pub(crate) tracked_repo_filter: String,
}

pub(crate) struct DashboardState {
    pub(crate) repo_index: usize,
    // Worktree creation form
    pub(crate) wt_path_input: String,
    pub(crate) wt_branch: Option<String>,
    pub(crate) wt_branch_filter: String,
    pub(crate) wt_new_branch_toggle: bool,
    pub(crate) wt_new_branch_name: String,
    // Cached data
    pub(crate) branches: Vec<String>,
    pub(crate) worktrees: Vec<Worktree>,
    pub(crate) loading_worktrees: bool,
    // Worktree list filter
    pub(crate) wt_list_filter: String,
}

impl App {
    pub(crate) fn new() -> (Self, Task<Message>) {
        let state = load_state().unwrap_or_default();
        let git_error = match gitgobig_core::git::check_git_installed() {
            Ok(_) => None,
            Err(e) => Some(ErrorInfo {
                message: "git is not installed or not found on PATH".to_string(),
                detail: Some(e.to_string()),
                show_detail: false,
            }),
        };
        let app = Self {
            screen: Screen::Setup(SetupScreen {
                url_input: String::new(),
                dest_input: String::new(),
                dest_auto_filled: false,
                tracked_repo_filter: String::new(),
            }),
            state,
            error: git_error,
            pending_confirmation: None,
            running_operation: None,
        };
        (app, clipboard::read().map(Message::ClipboardRead))
    }

    pub(crate) fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub(crate) fn set_error(&mut self, message: &str, detail: Option<String>) {
        self.error = Some(ErrorInfo {
            message: message.to_string(),
            detail,
            show_detail: false,
        });
    }
}
