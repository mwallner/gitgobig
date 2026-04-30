use iced::widget::{
    button, center, column, container, mouse_area, opaque, row, rule, scrollable, stack, text,
    text_input, toggler, Column,
};
use iced::{color, Element, Fill, Font, Length};

use crate::app::{App, DashboardState, ErrorInfo, PendingConfirmation, RunningOperation, Screen, SetupScreen};
use crate::message::Message;

impl App {
    pub(crate) fn view(&self) -> Element<'_, Message> {
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

    fn view_setup(&self, s: &SetupScreen) -> Element<'_, Message> {
        let repo_filter = s.tracked_repo_filter.to_lowercase();

        let repo_list: Element<'_, Message> = if self.state.repositories.is_empty() {
            text("No repositories tracked yet.").into()
        } else {
            let items: Vec<Element<'_, Message>> = self
                .state
                .repositories
                .iter()
                .enumerate()
                .filter(|(_, repo)| {
                    if repo_filter.is_empty() {
                        return true;
                    }
                    repo.name.to_lowercase().contains(&repo_filter)
                })
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
            if items.is_empty() {
                text("No tracked repositories match the filter.").into()
            } else {
                Column::with_children(items).spacing(4).into()
            }
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
            text_input("Filter tracked repositories…", &s.tracked_repo_filter)
                .on_input(Message::TrackedRepoFilterChanged)
                .padding(8),
            repo_list,
            rule::horizontal(1),
            clone_section,
            rule::horizontal(1),
            settings_section,
        ]
        .spacing(12)
        .into()
    }

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
