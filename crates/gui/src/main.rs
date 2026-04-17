mod async_git;

use std::path::PathBuf;

use iced::widget::{
    Column, button, center, column, container, mouse_area, opaque, row, rule, scrollable, stack,
    text, text_input, toggler,
};
use iced::{Element, Fill, Font, Length, Task, Theme, clipboard, color};

use gitgobig_config::{load_state, save_state};
use gitgobig_core::{AppState, Repository, Worktree};

use async_git::{GitEvent, GitResult};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(app_title)
        .theme(App::theme)
        .run()
}

fn app_title(_app: &App) -> String {
    String::from("gitgobig")
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    screen: Screen,
    state: AppState,
    error: Option<ErrorInfo>,
    pending_confirmation: Option<PendingConfirmation>,
    running_operation: Option<RunningOperation>,
}

/// Tracks a git operation currently in progress (or just finished).
struct RunningOperation {
    /// Human-readable label, e.g. "Cloning…" or "Syncing…"
    label: String,
    /// Accumulated stdout/stderr output lines from the git process.
    output_lines: Vec<String>,
    /// Set when the operation finishes; `None` while still running.
    finished: Option<FinishedOp>,
}

/// Outcome stored when a streaming git operation completes.
enum FinishedOp {
    Success(GitResult),
    Failed,
}

#[derive(Debug, Clone)]
enum PendingConfirmation {
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
    fn description(&self) -> &str {
        match self {
            PendingConfirmation::RemoveWorktree { description, .. } => description,
            PendingConfirmation::RemoveRepository { description, .. } => description,
        }
    }
}

struct ErrorInfo {
    message: String,
    detail: Option<String>,
    show_detail: bool,
}

enum Screen {
    Setup(SetupScreen),
    Dashboard(DashboardState),
}

struct SetupScreen {
    url_input: String,
    dest_input: String,
    dest_auto_filled: bool,
}

struct DashboardState {
    repo_index: usize,
    // Worktree creation form
    wt_path_input: String,
    wt_branch: Option<String>,
    wt_branch_filter: String,
    wt_new_branch_toggle: bool,
    wt_new_branch_name: String,
    // Cached data
    branches: Vec<String>,
    worktrees: Vec<Worktree>,
    loading_worktrees: bool,
    // Worktree list filter
    wt_list_filter: String,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Message {
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
    RemoveRepository(usize),

    // -- Dashboard --
    Sync,
    OpenRepoFolder,

    // -- Git operation streaming --
    GitOp(GitEvent),

    // -- Worktree management --
    RefreshWorktrees,
    WorktreesLoaded(Result<Vec<Worktree>, String>),
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

// ---------------------------------------------------------------------------
// Application impl
// ---------------------------------------------------------------------------

impl App {
    fn new() -> (Self, Task<Message>) {
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
            }),
            state,
            error: git_error,
            pending_confirmation: None,
            running_operation: None,
        };
        (app, clipboard::read().map(Message::ClipboardRead))
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // -- Navigation -------------------------------------------------
            Message::GoToSetup => {
                self.screen = Screen::Setup(SetupScreen {
                    url_input: String::new(),
                    dest_input: String::new(),
                    dest_auto_filled: false,
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
                        Some(format!(
                            "Could not start {}: {e}",
                            binary.display()
                        )),
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

    fn view(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match &self.screen {
            Screen::Setup(s) => self.view_setup(s),
            Screen::Dashboard(d) => self.view_dashboard(d),
        };

        let mut page = Column::new().spacing(10).padding(20).push(content);

        if let Some(err) = &self.error {
            page = page.push(rule::horizontal(1));
            page = page.push(view_error(err));
        }

        let base: Element<'_, Message> = container(scrollable(page)).into();

        // Determine which overlay to show (if any)
        let overlay: Option<Element<'_, Message>> = if let Some(op) = &self.running_operation {
            Some(self.view_progress_overlay(op))
        } else if let Some(confirmation) = &self.pending_confirmation {
            Some(view_confirmation_overlay(confirmation))
        } else {
            None
        };

        match overlay {
            Some(overlay) => stack![base, overlay].into(),
            None => base,
        }
    }

    fn view_progress_overlay<'a>(&self, op: &'a RunningOperation) -> Element<'a, Message> {
        let is_finished = op.finished.is_some();
        let status_icon = if is_finished { "✔" } else { "⏳" };

        let terminal_content: Element<'_, Message> = if op.output_lines.is_empty() {
            text("Waiting for output…")
                .size(12)
                .font(Font::MONOSPACE)
                .into()
        } else {
            let lines: Vec<Element<'_, Message>> = op
                .output_lines
                .iter()
                .map(|l| text(l.as_str()).size(12).font(Font::MONOSPACE).into())
                .collect();
            Column::with_children(lines).spacing(2).into()
        };

        let terminal_box = container(scrollable(terminal_content).height(250))
            .padding(12)
            .width(Fill)
            .style(|_theme| container::Style {
                background: Some(color!(0x11111b).into()),
                border: iced::Border {
                    color: color!(0x45475a),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });

        let mut content = column![
            row![text(status_icon).size(20), text(&op.label).size(16),].spacing(8),
            terminal_box,
        ]
        .spacing(16)
        .padding(24)
        .width(Fill);

        if is_finished {
            let log = op.output_lines.join("\n");
            content = content.push(
                row![
                    button("Proceed")
                        .on_press(Message::DismissProgress)
                        .padding([6, 20]),
                    container("").width(Fill),
                    button("📋")
                        .on_press(Message::CopyToClipboard(log))
                        .padding([4, 6]),
                ]
                .spacing(8),
            );
        }

        let modal_dialog = container(content)
            .style(|_theme| container::Style {
                background: Some(color!(0x1e1e2e).into()),
                border: iced::Border {
                    color: color!(0x585878),
                    width: 2.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: color!(0x000000),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                ..Default::default()
            })
            .max_width(600);

        opaque(
            center(opaque(modal_dialog))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.6,
                            ..color!(0x000000)
                        }
                        .into(),
                    ),
                    ..Default::default()
                }),
        )
        .into()
    }

    // -- Setup screen view --------------------------------------------------

    fn view_setup(&self, s: &SetupScreen) -> Element<'_, Message> {
        let repo_list: Element<'_, Message> = if self.state.repositories.is_empty() {
            text("No repositories tracked yet.").into()
        } else {
            let items: Vec<Element<'_, Message>> = self
                .state
                .repositories
                .iter()
                .enumerate()
                .map(|(i, repo)| {
                    row![
                        button(text(&repo.name).size(14)).on_press(Message::GoToDashboard(i)),
                        text(repo.path.display().to_string()).size(12),
                        button(text("📋").size(12))
                            .on_press(Message::CopyToClipboard(repo.path.display().to_string(),))
                            .padding(2),
                        button(text("✕").size(12))
                            .on_press(Message::RemoveRepository(i))
                            .padding(2),
                    ]
                    .spacing(8)
                    .into()
                })
                .collect();
            Column::with_children(items).spacing(4).into()
        };

        let clone_section = column![
            text("Add a repository").size(20),
            button("Add existing local repository…").on_press(Message::PickLocalRepo),
            text("— or clone a new one —").size(14),
            text_input("Repository URL", &s.url_input)
                .on_input(Message::UrlChanged)
                .padding(8),
            row![
                text_input("Destination directory", &s.dest_input)
                    .on_input(Message::DestChanged)
                    .padding(8)
                    .width(Fill),
                button("Browse…").on_press(Message::PickDestDir),
            ]
            .spacing(8),
            if self.running_operation.is_some() {
                Element::from(text("Cloning…"))
            } else {
                button("Clone").on_press(Message::Clone).into()
            },
        ]
        .spacing(8);

        let default_dir_display = self
            .state
            .default_repo_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let settings_section = column![
            text("Settings").size(20),
            text("Default repository directory").size(14),
            row![
                text_input("Not configured", &default_dir_display)
                    .on_input(Message::DefaultRepoDirChanged)
                    .padding(8)
                    .width(Fill),
                button("Browse…").on_press(Message::PickDefaultRepoDir),
                button("Clear").on_press(Message::ClearDefaultRepoDir),
            ]
            .spacing(8),
        ]
        .spacing(8);

        column![
            text("gitgobig").size(28),
            rule::horizontal(1),
            text("Tracked Repositories").size(18),
            repo_list,
            rule::horizontal(1),
            clone_section,
            rule::horizontal(1),
            settings_section,
        ]
        .spacing(12)
        .into()
    }

    // -- Dashboard view -----------------------------------------------------

    fn view_dashboard<'a>(&'a self, d: &'a DashboardState) -> Element<'a, Message> {
        let repo = &self.state.repositories[d.repo_index];

        let header = column![
            row![
                button("← Back").on_press(Message::GoToSetup),
                text(&repo.name).size(24),
            ]
            .spacing(12),
            row![
                text(format!("Path: {}", repo.path.display())).size(13),
                button(text("📋").size(11))
                    .on_press(Message::CopyToClipboard(repo.path.display().to_string(),))
                    .padding(2),
            ]
            .spacing(4),
            row![
                text(format!("URL: {}", repo.url)).size(13),
                button(text("📋").size(11))
                    .on_press(Message::CopyToClipboard(repo.url.clone()))
                    .padding(2),
            ]
            .spacing(4),
            row![
                if self.running_operation.is_some() {
                    Element::from(text("Syncing…"))
                } else {
                    button("Sync").on_press(Message::Sync).into()
                },
                button("Open folder").on_press(Message::OpenRepoFolder),
            ]
            .spacing(8),
        ]
        .spacing(6);

        let worktree_list_section = self.view_worktree_list(d);
        let worktree_create_section = self.view_worktree_create(d);

        column![
            header,
            rule::horizontal(1),
            text("Worktrees").size(20),
            worktree_list_section,
            rule::horizontal(1),
            text("Create Worktree").size(18),
            worktree_create_section,
        ]
        .spacing(12)
        .into()
    }

    fn view_worktree_list<'a>(&'a self, d: &'a DashboardState) -> Element<'a, Message> {
        if d.loading_worktrees {
            return text("Loading…").into();
        }
        if d.worktrees.is_empty() {
            return text("No worktrees.").into();
        }

        let filter_lower = d.wt_list_filter.to_lowercase();

        let rows: Vec<Element<'_, Message>> = d
            .worktrees
            .iter()
            .filter(|wt| {
                if filter_lower.is_empty() {
                    return true;
                }
                let path_str = wt.path.display().to_string().to_lowercase();
                let branch_str = wt.branch.as_deref().unwrap_or("").to_lowercase();
                path_str.contains(&filter_lower) || branch_str.contains(&filter_lower)
            })
            .map(|wt| {
                let branch_label = wt.branch.as_deref().unwrap_or("(detached)");
                let commit_label = wt
                    .commit
                    .as_deref()
                    .map(|c| &c[..c.len().min(12)])
                    .unwrap_or("?");

                row![
                    text(format!(
                        "{} — {} [{}]",
                        wt.path.display(),
                        branch_label,
                        commit_label,
                    ))
                    .size(13)
                    .width(Fill),
                    button(text("📋").size(11))
                        .on_press(Message::CopyToClipboard(wt.path.display().to_string(),))
                        .padding(2),
                    button("Explore")
                        .on_press(Message::ExploreWorktree(wt.path.clone()))
                        .padding(4),
                    button("Open")
                        .on_press(Message::OpenWorktreeFolder(wt.path.clone()))
                        .padding(4),
                    button("Remove")
                        .on_press(Message::RemoveWorktree(wt.path.clone()))
                        .style(button::danger)
                        .padding(4),
                ]
                .spacing(6)
                .into()
            })
            .collect();

        column![
            text_input("Filter worktrees…", &d.wt_list_filter)
                .on_input(Message::WtListFilterChanged)
                .padding(8),
            Column::with_children(rows).spacing(4),
        ]
        .spacing(8)
        .into()
    }

    fn view_worktree_create<'a>(&'a self, d: &'a DashboardState) -> Element<'a, Message> {
        let filter_lower = d.wt_branch_filter.to_lowercase();
        let filtered_branches: Vec<&String> = if filter_lower.is_empty() {
            d.branches.iter().collect()
        } else {
            d.branches
                .iter()
                .filter(|b| b.to_lowercase().contains(&filter_lower))
                .collect()
        };

        let branch_items: Vec<Element<'_, Message>> = filtered_branches
            .into_iter()
            .map(|b| {
                let is_selected = d.wt_branch.as_ref() == Some(b);
                let label = if is_selected {
                    text(format!("✓ {b}")).size(13)
                } else {
                    text(b.as_str()).size(13)
                };
                button(label)
                    .on_press(Message::WtBranchSelected(b.clone()))
                    .width(Fill)
                    .style(if is_selected {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .padding(4)
                    .into()
            })
            .collect();

        let branch_list =
            container(scrollable(Column::with_children(branch_items).spacing(2))).height(150);

        let selected_label: Element<'_, Message> = match &d.wt_branch {
            Some(b) => text(format!("Selected: {b}")).size(13).into(),
            None => text("No branch selected").size(13).into(),
        };

        let mut form = column![
            row![
                text_input("Worktree path", &d.wt_path_input)
                    .on_input(Message::WtPathChanged)
                    .padding(8)
                    .width(Fill),
                button("Browse…").on_press(Message::PickWtDir),
            ]
            .spacing(8),
            text("Branch").size(14),
            text_input("Filter branches…", &d.wt_branch_filter)
                .on_input(Message::WtBranchFilterChanged)
                .padding(8),
            branch_list,
            selected_label,
            toggler(d.wt_new_branch_toggle)
                .label("Create new branch")
                .on_toggle(Message::WtNewBranchToggled),
        ]
        .spacing(8);

        if d.wt_new_branch_toggle {
            form = form.push(
                text_input("New branch name", &d.wt_new_branch_name)
                    .on_input(Message::WtNewBranchNameChanged)
                    .padding(8),
            );
        }

        form = form.push(button("Create Worktree").on_press(Message::AddWorktree));

        form.into()
    }

    fn set_error(&mut self, message: &str, detail: Option<String>) {
        self.error = Some(ErrorInfo {
            message: message.to_string(),
            detail,
            show_detail: false,
        });
    }
}

fn view_error(err: &ErrorInfo) -> Element<'_, Message> {
    let mut col = column![
        row![
            text(&err.message).style(text::danger).size(14),
            button("✕").on_press(Message::DismissError).padding(2),
        ]
        .spacing(8),
    ]
    .spacing(4);

    if err.detail.is_some() {
        col = col.push(
            button(if err.show_detail {
                "Hide details"
            } else {
                "Show details"
            })
            .on_press(Message::ToggleErrorDetail)
            .padding(4),
        );
        if err.show_detail {
            if let Some(ref detail) = err.detail {
                col = col.push(text(detail).size(12));
            }
        }
    }

    col.into()
}

fn view_confirmation_overlay(confirmation: &PendingConfirmation) -> Element<'_, Message> {
    let modal_dialog = container(
        column![
            text(confirmation.description()).size(16),
            row![
                button("Cancel").on_press(Message::CancelAction),
                button("Confirm")
                    .on_press(Message::ConfirmAction)
                    .style(button::danger),
            ]
            .spacing(12),
        ]
        .spacing(16)
        .padding(24),
    )
    .style(|_theme| container::Style {
        background: Some(color!(0x1e1e2e).into()),
        border: iced::Border {
            color: color!(0x585878),
            width: 2.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: color!(0x000000),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 20.0,
        },
        ..Default::default()
    })
    .max_width(400);

    opaque(
        mouse_area(
            center(opaque(modal_dialog))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.6,
                            ..color!(0x000000)
                        }
                        .into(),
                    ),
                    ..Default::default()
                }),
        )
        .on_press(Message::CancelAction),
    )
    .into()
}
