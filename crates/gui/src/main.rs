mod async_git;

use std::path::PathBuf;

use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_input,
    toggler, Column,
};
use iced::{Element, Fill, Task, Theme};

use gitgobig_config::{load_state, save_state};
use gitgobig_core::{AppState, Repository, Worktree};

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
    cloning: bool,
}

struct DashboardState {
    repo_index: usize,
    syncing: bool,
    // Worktree creation form
    wt_path_input: String,
    wt_branch: Option<String>,
    wt_new_branch_toggle: bool,
    wt_new_branch_name: String,
    // Cached data
    branches: Vec<String>,
    worktrees: Vec<Worktree>,
    loading_worktrees: bool,
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
    CloneFinished(Result<(), String>),

    // -- Dashboard --
    Sync,
    SyncFinished(Result<String, String>),
    OpenRepoFolder,

    // -- Worktree management --
    RefreshWorktrees,
    WorktreesLoaded(Result<Vec<Worktree>, String>),
    BranchesLoaded(Result<Vec<String>, String>),
    WtPathChanged(String),
    PickWtDir,
    WtDirPicked(Option<PathBuf>),
    WtBranchSelected(String),
    WtNewBranchToggled(bool),
    WtNewBranchNameChanged(String),
    AddWorktree,
    WorktreeAdded(Result<(), String>),
    RemoveWorktree(PathBuf),
    WorktreeRemoved(Result<(), String>),
    OpenWorktreeFolder(PathBuf),

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
                cloning: false,
            }),
            state,
            error: git_error,
        };
        (app, Task::none())
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
                    cloning: false,
                });
                Task::none()
            }
            Message::GoToDashboard(idx) => {
                if idx >= self.state.repositories.len() {
                    return Task::none();
                }
                self.screen = Screen::Dashboard(DashboardState {
                    repo_index: idx,
                    syncing: false,
                    wt_path_input: String::new(),
                    wt_branch: None,
                    wt_new_branch_toggle: false,
                    wt_new_branch_name: String::new(),
                    branches: Vec::new(),
                    worktrees: Vec::new(),
                    loading_worktrees: true,
                });
                let repo_path = self.state.repositories[idx].path.clone();
                let rp2 = repo_path.clone();
                Task::batch([
                    Task::perform(
                        async_git::worktree_list(repo_path),
                        |r| Message::WorktreesLoaded(r.map_err(|e| e.to_string())),
                    ),
                    Task::perform(
                        async_git::branch_list(rp2),
                        |r| Message::BranchesLoaded(r.map_err(|e| e.to_string())),
                    ),
                ])
            }

            // -- Setup screen -----------------------------------------------
            Message::UrlChanged(url) => {
                if let Screen::Setup(s) = &mut self.screen {
                    s.url_input = url;
                }
                Task::none()
            }
            Message::DestChanged(dest) => {
                if let Screen::Setup(s) = &mut self.screen {
                    s.dest_input = dest;
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
            Message::Clone => {
                if let Screen::Setup(s) = &mut self.screen {
                    if s.url_input.trim().is_empty() || s.dest_input.trim().is_empty() {
                        self.set_error("URL and destination must not be empty", None);
                        return Task::none();
                    }
                    s.cloning = true;
                    let url = s.url_input.trim().to_string();
                    let dest = PathBuf::from(s.dest_input.trim());
                    Task::perform(async_git::clone_bare(url, dest), |r| {
                        Message::CloneFinished(r.map_err(|e| e.to_string()))
                    })
                } else {
                    Task::none()
                }
            }
            Message::CloneFinished(result) => {
                if let Screen::Setup(s) = &mut self.screen {
                    s.cloning = false;
                    match result {
                        Ok(()) => {
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
                        Err(e) => self.set_error("Clone failed", Some(e)),
                    }
                }
                Task::none()
            }

            // -- Dashboard --------------------------------------------------
            Message::Sync => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.syncing = true;
                    let repo_path = self.state.repositories[d.repo_index].path.clone();
                    Task::perform(async_git::sync(repo_path), |r| {
                        Message::SyncFinished(r.map_err(|e| e.to_string()))
                    })
                } else {
                    Task::none()
                }
            }
            Message::SyncFinished(result) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    d.syncing = false;
                    if let Err(e) = result {
                        self.set_error("Sync failed", Some(e));
                    }
                }
                Task::none()
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
            Message::WtBranchSelected(b) => {
                if let Screen::Dashboard(d) = &mut self.screen {
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
                    let new_branch = if d.wt_new_branch_toggle && !d.wt_new_branch_name.trim().is_empty() {
                        Some(d.wt_new_branch_name.trim().to_string())
                    } else {
                        None
                    };
                    Task::perform(
                        async_git::worktree_add(repo_path, wt_path, branch, new_branch),
                        |r| Message::WorktreeAdded(r.map_err(|e| e.to_string())),
                    )
                } else {
                    Task::none()
                }
            }
            Message::WorktreeAdded(result) => {
                if let Screen::Dashboard(d) = &mut self.screen {
                    match result {
                        Ok(()) => {
                            // Persist and refresh
                            let _ = save_state(&self.state);
                            d.wt_path_input.clear();
                            d.wt_new_branch_name.clear();
                            d.wt_new_branch_toggle = false;
                            return self.update(Message::RefreshWorktrees);
                        }
                        Err(e) => self.set_error("Failed to add worktree", Some(e)),
                    }
                }
                Task::none()
            }
            Message::RemoveWorktree(wt_path) => {
                if let Screen::Dashboard(d) = &self.screen {
                    let repo_path = self.state.repositories[d.repo_index].path.clone();
                    Task::perform(
                        async_git::worktree_remove(repo_path, wt_path),
                        |r| Message::WorktreeRemoved(r.map_err(|e| e.to_string())),
                    )
                } else {
                    Task::none()
                }
            }
            Message::WorktreeRemoved(result) => {
                match result {
                    Ok(()) => return self.update(Message::RefreshWorktrees),
                    Err(e) => self.set_error("Failed to remove worktree", Some(e)),
                }
                Task::none()
            }
            Message::OpenWorktreeFolder(path) => {
                let _ = opener::open(&path);
                Task::none()
            }

            // -- Error ------------------------------------------------------
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

        container(scrollable(page)).into()
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
                    button(text(&repo.name).size(14))
                        .on_press(Message::GoToDashboard(i))
                        .into()
                })
                .collect();
            Column::with_children(items).spacing(4).into()
        };

        let clone_section = column![
            text("Clone a new repository").size(20),
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
            if s.cloning {
                Element::from(text("Cloning…"))
            } else {
                button("Clone").on_press(Message::Clone).into()
            },
        ]
        .spacing(8);

        column![
            text("gitgobig").size(28),
            rule::horizontal(1),
            text("Tracked Repositories").size(18),
            repo_list,
            rule::horizontal(1),
            clone_section,
        ]
        .spacing(12)
        .into()
    }

    // -- Dashboard view -----------------------------------------------------

    fn view_dashboard(&self, d: &DashboardState) -> Element<'_, Message> {
        let repo = &self.state.repositories[d.repo_index];

        let header = column![
            row![
                button("← Back").on_press(Message::GoToSetup),
                text(&repo.name).size(24),
            ]
            .spacing(12),
            text(format!("Path: {}", repo.path.display())).size(13),
            text(format!("URL: {}", repo.url)).size(13),
            row![
                if d.syncing {
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

    fn view_worktree_list(&self, d: &DashboardState) -> Element<'_, Message> {
        if d.loading_worktrees {
            return text("Loading…").into();
        }
        if d.worktrees.is_empty() {
            return text("No worktrees.").into();
        }

        let rows: Vec<Element<'_, Message>> = d
            .worktrees
            .iter()
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

        Column::with_children(rows).spacing(4).into()
    }

    fn view_worktree_create(&self, d: &DashboardState) -> Element<'_, Message> {
        let branch_picker: Element<'_, Message> = pick_list(
            d.branches.clone(),
            d.wt_branch.clone(),
            Message::WtBranchSelected,
        )
        .placeholder("Select branch")
        .into();

        let mut form = column![
            row![
                text_input("Worktree path", &d.wt_path_input)
                    .on_input(Message::WtPathChanged)
                    .padding(8)
                    .width(Fill),
                button("Browse…").on_press(Message::PickWtDir),
            ]
            .spacing(8),
            branch_picker,
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
