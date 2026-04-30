use std::path::PathBuf;

use gitgobig_config::save_state;
use gitgobig_core::Repository;
use iced::{clipboard, Task};

use crate::app::{
    App, DashboardState, FinishedOp, PendingConfirmation, RunningOperation, Screen, SetupScreen,
};
use crate::async_git::{self, GitEvent, GitResult};
use crate::message::Message;

impl App {
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // -- Navigation -------------------------------------------------
            Message::GoToSetup => {
                self.screen = Screen::Setup(SetupScreen {
                    url_input: String::new(),
                    dest_input: String::new(),
                    dest_auto_filled: false,
                    tracked_repo_filter: String::new(),
                });
                clipboard::read().map(Message::ClipboardRead)
            }
            Message::GoToDashboard(idx) => {
                if idx >= self.state.repositories.len() {
                    return Task::none();
                }
                self.screen = Screen::Dashboard(DashboardState {
                    repo_index: idx,
                    wt_path_input: String::new(),
                    wt_branch: None,
                    wt_branch_filter: String::new(),
                    wt_new_branch_toggle: false,
                    wt_new_branch_name: String::new(),
                    branches: Vec::new(),
                    worktrees: Vec::new(),
                    loading_worktrees: true,
                    wt_list_filter: String::new(),
                });
                let repo_path = self.state.repositories[idx].path.clone();
                let rp2 = repo_path.clone();
                Task::batch([
                    Task::perform(async_git::worktree_list(repo_path), |r| {
                        Message::WorktreesLoaded(r.map_err(|e| e.to_string()))
                    }),
                    Task::perform(async_git::branch_list(rp2), |r| {
                        Message::BranchesLoaded(r.map_err(|e| e.to_string()))
                    }),
                ])
            }

            // -- Setup screen -----------------------------------------------
            Message::UrlChanged(url) => {
                if let Screen::Setup(s) = &mut self.screen {
                    s.url_input = url.clone();
                    // Auto-fill destination when default repo dir is configured
                    if let Some(ref base) = self.state.default_repo_dir {
                        if s.dest_input.is_empty() || s.dest_auto_filled {
                            let name = gitgobig_core::git::repo_name_from_url(&url);
                            if !name.is_empty() {
                                let suggested = base.join(&name).join(format!("{name}_bare"));
                                s.dest_input = suggested.display().to_string();
                                s.dest_auto_filled = true;
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::DestChanged(dest) => {
                if let Screen::Setup(s) = &mut self.screen {
                    s.dest_input = dest;
                    s.dest_auto_filled = false;
                }
                Task::none()
            }
            Message::PickDestDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose clone destination")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::DestDirPicked,
            ),
            Message::DestDirPicked(path) => {
                if let (Screen::Setup(s), Some(p)) = (&mut self.screen, path) {
                    s.dest_input = p.display().to_string();
                }
                Task::none()
            }
            Message::PickLocalRepo => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose existing bare repository")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::LocalRepoPicked,
            ),
            Message::LocalRepoPicked(path) => {
                if let Some(p) = path {
                    if !gitgobig_core::git::is_git_repo(&p) {
                        self.set_error(
                            "Not a git repository",
                            Some(format!("{} is not a git repository", p.display())),
                        );
                        return Task::none();
                    }
                    if self.state.repositories.iter().any(|r| r.path == p) {
                        self.set_error("This repository is already tracked", None);
                        return Task::none();
                    }
                    let is_bare = gitgobig_core::git::is_bare_repo(&p).unwrap_or(false);
                    let url = gitgobig_core::git::get_remote_url(&p)
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    let name = if !url.is_empty() {
                        gitgobig_core::git::repo_name_from_url(&url)
                    } else {
                        p.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unnamed".into())
                    };
                    self.state.repositories.push(Repository {
                        name,
                        path: p,
                        url,
                        worktrees: vec![],
                    });
                    let _ = save_state(&self.state);
                    if !is_bare {
                        self.set_error(
                            "Note: this is not a bare repository",
                            Some("Worktree management features may not work as expected with non-bare repositories.".into()),
                        );
                    }
                    let idx = self.state.repositories.len() - 1;
                    return self.update(Message::GoToDashboard(idx));
                }
                Task::none()
            }
            Message::TrackedRepoFilterChanged(filter) => {
                if let Screen::Setup(s) = &mut self.screen {
                    s.tracked_repo_filter = filter;
                }
                Task::none()
            }
            Message::RemoveRepository(idx) => {
                if idx < self.state.repositories.len() {
                    let name = self.state.repositories[idx].name.clone();
                    self.pending_confirmation = Some(PendingConfirmation::RemoveRepository {
                        description: format!(
                            "Remove tracked repository \"{}\"? (No files will be deleted on disk.)",
                            name
                        ),
                        index: idx,
                    });
                }
                Task::none()
            }
            Message::Clone => {
                if let Screen::Setup(s) = &mut self.screen {
                    if s.url_input.trim().is_empty() || s.dest_input.trim().is_empty() {
                        self.set_error("URL and destination must not be empty", None);
                        return Task::none();
                    }
                    let url = s.url_input.trim().to_string();
                    let dest = PathBuf::from(s.dest_input.trim());
                    self.running_operation = Some(RunningOperation {
                        label: "Cloning…".into(),
                        output_lines: Vec::new(),
                        finished: None,
                    });
                    Task::run(async_git::clone_bare_stream(url, dest), Message::GitOp)
                } else {
                    Task::none()
                }
            }

            // -- Dashboard --------------------------------------------------
            Message::Sync => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    let repo_path = self.state.repositories[d.repo_index].path.clone();
                    self.running_operation = Some(RunningOperation {
                        label: "Syncing…".into(),
                        output_lines: Vec::new(),
                        finished: None,
                    });
                    Task::run(async_git::sync_stream(repo_path), Message::GitOp)
                } else {
                    Task::none()
                }
            }
            Message::OpenRepoFolder => {
                if let Screen::Dashboard(d) = &self.screen {
                    let path = &self.state.repositories[d.repo_index].path;
                    let _ = opener::open(path);
                }
                Task::none()
            }

            // -- Worktree management ----------------------------------------
            Message::RefreshWorktrees => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.loading_worktrees = true;
                    let repo_path = self.state.repositories[d.repo_index].path.clone();
                    Task::perform(async_git::worktree_list(repo_path), |r| {
                        Message::WorktreesLoaded(r.map_err(|e| e.to_string()))
                    })
                } else {
                    Task::none()
                }
            }
            Message::WorktreesLoaded(result) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.loading_worktrees = false;
                    match result {
                        Ok(wts) => d.worktrees = wts,
                        Err(e) => self.set_error("Failed to list worktrees", Some(e)),
                    }
                }
                Task::none()
            }
            Message::BranchesLoaded(result) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    match result {
                        Ok(branches) => {
                            d.branches = branches;
                            if d.wt_branch.is_none() {
                                d.wt_branch = d.branches.first().cloned();
                            }
                        }
                        Err(e) => self.set_error("Failed to list branches", Some(e)),
                    }
                }
                Task::none()
            }
            Message::WtPathChanged(p) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.wt_path_input = p;
                }
                Task::none()
            }
            Message::PickWtDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose worktree directory")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::WtDirPicked,
            ),
            Message::WtDirPicked(path) => {
                if let (Screen::Dashboard(d), Some(p)) = (&mut self.screen, path) {
                    d.wt_path_input = p.display().to_string();
                }
                Task::none()
            }
            Message::WtBranchFilterChanged(f) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.wt_branch_filter = f;
                }
                Task::none()
            }
            Message::WtBranchSelected(b) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.wt_branch_filter = b.clone();
                    d.wt_branch = Some(b);
                }
                Task::none()
            }
            Message::WtNewBranchToggled(on) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.wt_new_branch_toggle = on;
                }
                Task::none()
            }
            Message::WtNewBranchNameChanged(name) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.wt_new_branch_name = name;
                }
                Task::none()
            }
            Message::AddWorktree => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    let repo_path = self.state.repositories[d.repo_index].path.clone();
                    let wt_path = PathBuf::from(d.wt_path_input.trim());
                    let branch = match &d.wt_branch {
                        Some(b) => b.clone(),
                        None => {
                            self.set_error("Select a branch first", None);
                            return Task::none();
                        }
                    };
                    let new_branch =
                        if d.wt_new_branch_toggle && !d.wt_new_branch_name.trim().is_empty() {
                            Some(d.wt_new_branch_name.trim().to_string())
                        } else {
                            None
                        };
                    self.running_operation = Some(RunningOperation {
                        label: "Creating worktree…".into(),
                        output_lines: Vec::new(),
                        finished: None,
                    });
                    Task::run(
                        async_git::worktree_add_stream(repo_path, wt_path, branch, new_branch),
                        Message::GitOp,
                    )
                } else {
                    Task::none()
                }
            }
            Message::RemoveWorktree(wt_path) => {
                if let Screen::Dashboard(d) = &self.screen {
                    let repo_path = self.state.repositories[d.repo_index].path.clone();
                    self.pending_confirmation = Some(PendingConfirmation::RemoveWorktree {
                        description: format!("Remove worktree at {}?", wt_path.display()),
                        repo_path,
                        wt_path,
                    });
                }
                Task::none()
            }
            Message::OpenWorktreeFolder(path) => {
                let _ = opener::open(&path);
                Task::none()
            }
            Message::ExploreWorktree(path) => {
                // Look for gitgows next to the current binary, then fall back to PATH.
                let binary = std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(|d| d.join("gitgows")))
                    .filter(|p| p.exists())
                    .unwrap_or_else(|| PathBuf::from("gitgows"));
                if let Err(e) = std::process::Command::new(&binary).arg(&path).spawn() {
                    self.set_error(
                        "Failed to launch gitgows",
                        Some(format!("Could not start {}: {e}", binary.display())),
                    );
                }
                Task::none()
            }

            // -- Git operation streaming --------------------------------
            Message::GitOp(event) => match event {
                GitEvent::Output(line) => {
                    if let Some(op) = &mut self.running_operation {
                        op.output_lines.push(line);
                    }
                    Task::none()
                }
                GitEvent::Done(result) => {
                    if let Some(op) = &mut self.running_operation {
                        op.label = match &result {
                            GitResult::CloneDone => "Clone finished ✓".into(),
                            GitResult::SyncDone(s) => format!("{s}\nSync finished ✓"),
                            GitResult::WorktreeAddDone => "Worktree added ✓".into(),
                            GitResult::WorktreeRemoveDone => "Worktree removed ✓".into(),
                        };
                        op.finished = Some(FinishedOp::Success(result));
                    }
                    Task::none()
                }
                GitEvent::Failed(e) => {
                    if let Some(op) = &mut self.running_operation {
                        op.label = "Operation failed ✗".into();
                        op.output_lines.push(format!("ERROR: {e}"));
                        op.finished = Some(FinishedOp::Failed);
                    }
                    Task::none()
                }
            },

            // -- Dismiss progress overlay after completion --------------
            Message::DismissProgress => {
                if let Some(op) = self.running_operation.take() {
                    match op.finished {
                        Some(FinishedOp::Success(result)) => match result {
                            GitResult::CloneDone => {
                                if let Screen::Setup(s) = &mut self.screen {
                                    let url = s.url_input.trim().to_string();
                                    let dest = PathBuf::from(s.dest_input.trim());
                                    let name = url
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or(&url)
                                        .trim_end_matches(".git")
                                        .to_string();
                                    self.state.repositories.push(Repository {
                                        name,
                                        path: dest,
                                        url,
                                        worktrees: vec![],
                                    });
                                    let _ = save_state(&self.state);
                                    let idx = self.state.repositories.len() - 1;
                                    return self.update(Message::GoToDashboard(idx));
                                }
                            }
                            GitResult::SyncDone(_) => {}
                            GitResult::WorktreeAddDone => {
                                if let Screen::Dashboard(d) = &mut self.screen {
                                    let _ = save_state(&self.state);
                                    d.wt_path_input.clear();
                                    d.wt_new_branch_name.clear();
                                    d.wt_new_branch_toggle = false;
                                    d.wt_branch_filter.clear();
                                    return self.update(Message::RefreshWorktrees);
                                }
                            }
                            GitResult::WorktreeRemoveDone => {
                                return self.update(Message::RefreshWorktrees);
                            }
                        },
                        Some(FinishedOp::Failed) | None => {}
                    }
                }
                Task::none()
            }

            // -- Worktree list filter -----------------------------------
            Message::WtListFilterChanged(f) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.wt_list_filter = f;
                }
                Task::none()
            }

            // -- Confirmation dialog ------------------------------------
            Message::ConfirmAction => {
                if let Some(action) = self.pending_confirmation.take() {
                    match action {
                        PendingConfirmation::RemoveWorktree {
                            repo_path, wt_path, ..
                        } => {
                            self.running_operation = Some(RunningOperation {
                                label: "Removing worktree…".into(),
                                output_lines: Vec::new(),
                                finished: None,
                            });
                            return Task::run(
                                async_git::worktree_remove_stream(repo_path, wt_path),
                                Message::GitOp,
                            );
                        }
                        PendingConfirmation::RemoveRepository { index, .. } => {
                            if index < self.state.repositories.len() {
                                self.state.repositories.remove(index);
                                let _ = save_state(&self.state);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::CancelAction => {
                self.pending_confirmation = None;
                Task::none()
            }

            // -- Clipboard ----------------------------------------------
            Message::CopyToClipboard(content) => clipboard::write(content),
            Message::ClipboardRead(content) => {
                if let Screen::Setup(s) = &mut self.screen {
                    if s.url_input.is_empty() {
                        if let Some(text) = content {
                            let trimmed = text.trim();
                            if trimmed.ends_with(".git")
                                && (trimmed.starts_with("https://")
                                    || trimmed.starts_with("http://")
                                    || trimmed.starts_with("git@")
                                    || trimmed.starts_with("ssh://"))
                            {
                                s.url_input = trimmed.to_string();
                                // Also auto-fill destination if configured
                                if let Some(ref base) = self.state.default_repo_dir {
                                    let name = gitgobig_core::git::repo_name_from_url(trimmed);
                                    if !name.is_empty() {
                                        let suggested =
                                            base.join(&name).join(format!("{name}_bare"));
                                        s.dest_input = suggested.display().to_string();
                                        s.dest_auto_filled = true;
                                    }
                                }
                            }
                        }
                    }
                }
                Task::none()
            }

            // -- Settings -----------------------------------------------
            Message::DefaultRepoDirChanged(dir) => {
                self.state.default_repo_dir = if dir.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(dir))
                };
                let _ = save_state(&self.state);
                Task::none()
            }
            Message::PickDefaultRepoDir => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Choose default repository directory")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::DefaultRepoDirPicked,
            ),
            Message::DefaultRepoDirPicked(path) => {
                if let Some(p) = path {
                    self.state.default_repo_dir = Some(p);
                    let _ = save_state(&self.state);
                }
                Task::none()
            }
            Message::ClearDefaultRepoDir => {
                self.state.default_repo_dir = None;
                let _ = save_state(&self.state);
                Task::none()
            }

            // -- Error --------------------------------------------------
            Message::ToggleErrorDetail => {
                if let Some(err) = &mut self.error {
                    err.show_detail = !err.show_detail;
                }
                Task::none()
            }
            Message::DismissError => {
                self.error = None;
                Task::none()
            }
        }
    }
}
