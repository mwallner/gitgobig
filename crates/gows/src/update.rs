use iced::{clipboard, window, Event, Task};

use crate::app::App;
use crate::message::{
    load_commits, load_commits_for_branches, load_detail, Message,
};
use crate::style::{ContextMenu, DragState, InspectState, MIN_COL_WIDTH, PAGE_SIZE, ResizeHandle};

impl App {
    pub(crate) fn refilter(&mut self) {
        if !self.has_active_search() {
            self.filtered_indices.clear();
            return;
        }

        let use_regex = self.search_regex;
        let hash_re = if use_regex && !self.search_hash.is_empty() {
            regex::RegexBuilder::new(&self.search_hash)
                .case_insensitive(true)
                .build()
                .ok()
        } else {
            None
        };
        let msg_re = if use_regex && !self.search_message.is_empty() {
            regex::RegexBuilder::new(&self.search_message)
                .case_insensitive(true)
                .build()
                .ok()
        } else {
            None
        };
        let date_re = if use_regex && !self.search_date.is_empty() {
            regex::RegexBuilder::new(&self.search_date)
                .case_insensitive(true)
                .build()
                .ok()
        } else {
            None
        };
        let author_re = if use_regex && !self.search_author.is_empty() {
            regex::RegexBuilder::new(&self.search_author)
                .case_insensitive(true)
                .build()
                .ok()
        } else {
            None
        };

        let h = self.search_hash.to_lowercase();
        let m = self.search_message.to_lowercase();
        let d = self.search_date.to_lowercase();
        let a = self.search_author.to_lowercase();

        self.filtered_indices = self
            .commits
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let match_hash = if self.search_hash.is_empty() {
                    true
                } else if let Some(ref re) = hash_re {
                    re.is_match(&c.hash) || re.is_match(&c.short_hash)
                } else {
                    c.hash.to_lowercase().contains(&h) || c.short_hash.to_lowercase().contains(&h)
                };
                let match_msg = if self.search_message.is_empty() {
                    true
                } else if let Some(ref re) = msg_re {
                    re.is_match(&c.subject)
                } else {
                    c.subject.to_lowercase().contains(&m)
                };
                let match_date = if self.search_date.is_empty() {
                    true
                } else if let Some(ref re) = date_re {
                    re.is_match(&c.date)
                } else {
                    c.date.to_lowercase().contains(&d)
                };
                let match_author = if self.search_author.is_empty() {
                    true
                } else if let Some(ref re) = author_re {
                    re.is_match(&c.author)
                } else {
                    c.author.to_lowercase().contains(&a)
                };
                match_hash && match_msg && match_date && match_author
            })
            .map(|(i, _)| i)
            .collect();
    }

    pub(crate) fn maybe_deep_load(&mut self) -> Task<Message> {
        let depth: usize = match self.search_depth.parse() {
            Ok(n) if n > 0 => n,
            _ => return Task::none(),
        };
        if depth <= self.commits.len() || self.all_loaded || self.search_loading {
            return Task::none();
        }
        self.search_loading = true;
        let rp = self.repo_path.clone();
        let skip = self.commits.len();
        let need = depth - skip;
        Task::perform(load_commits(rp, skip, need), Message::SearchCommitsLoaded)
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CommitsLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(new_commits) => {
                        if new_commits.len() < PAGE_SIZE {
                            self.all_loaded = true;
                        }
                        self.commits.extend(new_commits);
                        self.rebuild_graph();
                        self.refilter();
                    }
                    Err(e) => {
                        if self.commits.is_empty() {
                            self.error = Some(e);
                        }
                    }
                }
                Task::none()
            }

            Message::SearchCommitsLoaded(result) => {
                self.search_loading = false;
                if let Ok(new_commits) = result {
                    if new_commits.len() < PAGE_SIZE {
                        self.all_loaded = true;
                    }
                    self.commits.extend(new_commits);
                    self.rebuild_graph();
                    self.refilter();
                }
                Task::none()
            }

            Message::LoadMore => {
                if self.loading || self.all_loaded {
                    return Task::none();
                }
                self.loading = true;
                let rp = self.repo_path.clone();
                let skip = self.commits.len();

                let all_selected = self.selected_branches.is_empty()
                    || (self.selected_branches.len() == self.all_branches.len()
                        && self
                            .all_branches
                            .iter()
                            .all(|b| self.selected_branches.contains(b)));

                if all_selected {
                    Task::perform(load_commits(rp, skip, PAGE_SIZE), Message::CommitsLoaded)
                } else {
                    let branches: Vec<String> = self.selected_branches.iter().cloned().collect();
                    Task::perform(
                        load_commits_for_branches(rp, skip, PAGE_SIZE, branches),
                        Message::CommitsLoaded,
                    )
                }
            }

            Message::SelectCommit(idx) => {
                self.selected_index = if self.selected_index == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                Task::none()
            }

            Message::ShowContextMenu(idx) => {
                self.selected_index = Some(idx);
                self.context_menu = Some(ContextMenu { commit_index: idx });
                Task::none()
            }

            Message::DismissContextMenu => {
                self.context_menu = None;
                Task::none()
            }

            Message::CopyHash(hash) => {
                self.context_menu = None;
                clipboard::write(hash)
            }

            Message::InspectCommit(hash) => {
                self.context_menu = None;
                let rp = self.repo_path.clone();
                Task::perform(load_detail(rp, hash), Message::InspectLoaded)
            }

            Message::InspectLoaded(result) => {
                match result {
                    Ok(detail) => self.inspect = Some(InspectState { detail }),
                    Err(e) => {
                        self.inspect = Some(InspectState {
                            detail: format!("Error: {e}"),
                        })
                    }
                }
                Task::none()
            }

            Message::DismissInspect => {
                self.inspect = None;
                Task::none()
            }

            Message::DragStart(handle) => {
                let start_width = match handle {
                    ResizeHandle::Graph => self.graph_col_width,
                    ResizeHandle::Hash => self.hash_width,
                    ResizeHandle::Date => self.date_width,
                    ResizeHandle::Author => self.author_width,
                };
                self.dragging = Some(DragState {
                    handle,
                    start_x: None,
                    start_width,
                });
                Task::none()
            }

            Message::GlobalEvent(ev) => {
                match ev {
                    Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                        if let Some(ref mut drag) = self.dragging {
                            let start_x = *drag.start_x.get_or_insert(position.x);
                            let delta = position.x - start_x;
                            let max_graph = self.window_width / 2.0;
                            let new_w = match drag.handle {
                                ResizeHandle::Graph => {
                                    (drag.start_width + delta)
                                        .max(MIN_COL_WIDTH)
                                        .min(max_graph)
                                }
                                ResizeHandle::Hash => {
                                    (drag.start_width + delta).max(MIN_COL_WIDTH)
                                }
                                ResizeHandle::Date => {
                                    (drag.start_width - delta).max(MIN_COL_WIDTH)
                                }
                                ResizeHandle::Author => {
                                    (drag.start_width - delta).max(MIN_COL_WIDTH)
                                }
                            };
                            match drag.handle {
                                ResizeHandle::Graph => {
                                    self.graph_col_width = new_w;
                                    self.graph_manually_sized = true;
                                }
                                ResizeHandle::Hash => self.hash_width = new_w,
                                ResizeHandle::Date => self.date_width = new_w,
                                ResizeHandle::Author => self.author_width = new_w,
                            };
                        }
                    }
                    Event::Mouse(iced::mouse::Event::ButtonReleased(
                        iced::mouse::Button::Left,
                    )) => {
                        self.dragging = None;
                    }
                    Event::Window(window::Event::Resized(size)) => {
                        self.window_width = size.width;
                        if self.graph_col_width > self.window_width / 2.0 {
                            self.graph_col_width = self.window_width / 2.0;
                        }
                    }
                    _ => {}
                }
                Task::none()
            }

            Message::SearchDepth(v) => {
                if v.is_empty() || v.chars().all(|c| c.is_ascii_digit()) {
                    self.search_depth = v;
                    self.refilter();
                    return self.maybe_deep_load();
                }
                Task::none()
            }

            Message::SearchHash(v) => {
                self.search_hash = v;
                self.refilter();
                self.maybe_deep_load()
            }

            Message::SearchMessage(v) => {
                self.search_message = v;
                self.refilter();
                self.maybe_deep_load()
            }

            Message::SearchDate(v) => {
                self.search_date = v;
                self.refilter();
                self.maybe_deep_load()
            }

            Message::SearchAuthor(v) => {
                self.search_author = v;
                self.refilter();
                self.maybe_deep_load()
            }

            Message::ToggleRegex(v) => {
                self.search_regex = v;
                self.refilter();
                Task::none()
            }

            Message::ClearSearch => {
                self.search_hash.clear();
                self.search_message.clear();
                self.search_date.clear();
                self.search_author.clear();
                self.search_depth.clear();
                self.filtered_indices.clear();
                Task::none()
            }

            Message::BranchesLoaded(result) => {
                self.branches_loading = false;
                if let Ok(branches) = result {
                    self.selected_branches = branches.iter().cloned().collect();
                    self.all_branches = branches;
                }
                Task::none()
            }

            Message::ToggleBranchDropdown => {
                self.branch_dropdown_open = !self.branch_dropdown_open;
                Task::none()
            }

            Message::DismissBranchDropdown => {
                self.branch_dropdown_open = false;
                Task::none()
            }

            Message::BranchFilterText(v) => {
                self.branch_filter_text = v;
                Task::none()
            }

            Message::ToggleBranch(branch) => {
                if self.selected_branches.contains(&branch) {
                    self.selected_branches.remove(&branch);
                } else {
                    self.selected_branches.insert(branch);
                }
                self.update(Message::BranchSelectionChanged)
            }

            Message::SelectAllBranches => {
                self.selected_branches = self.all_branches.iter().cloned().collect();
                self.update(Message::BranchSelectionChanged)
            }

            Message::DeselectAllBranches => {
                self.selected_branches.clear();
                self.update(Message::BranchSelectionChanged)
            }

            Message::BranchSelectionChanged => {
                self.commits.clear();
                self.graph_rows.clear();
                self.max_graph_cols = 0;
                self.selected_index = None;
                self.all_loaded = false;
                self.filtered_indices.clear();
                self.loading = true;

                if self.selected_branches.is_empty() {
                    self.loading = false;
                    self.all_loaded = true;
                    return Task::none();
                }

                let all_selected = self.selected_branches.len() == self.all_branches.len()
                    && self
                        .all_branches
                        .iter()
                        .all(|b| self.selected_branches.contains(b));

                let rp = self.repo_path.clone();
                if all_selected {
                    Task::perform(
                        load_commits(rp, 0, PAGE_SIZE),
                        Message::BranchCommitsLoaded,
                    )
                } else {
                    let branches: Vec<String> = self.selected_branches.iter().cloned().collect();
                    Task::perform(
                        load_commits_for_branches(rp, 0, PAGE_SIZE, branches),
                        Message::BranchCommitsLoaded,
                    )
                }
            }

            Message::BranchCommitsLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(new_commits) => {
                        if new_commits.len() < PAGE_SIZE {
                            self.all_loaded = true;
                        }
                        self.commits = new_commits;
                        self.rebuild_graph();
                        self.refilter();
                    }
                    Err(e) => {
                        if self.commits.is_empty() {
                            self.error = Some(e);
                        }
                    }
                }
                Task::none()
            }
        }
    }
}
