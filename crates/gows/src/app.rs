use std::collections::HashSet;
use std::path::PathBuf;

use iced::{event, Subscription, Task, Theme};

use gitgobig_core::CommitEntry;

use crate::graph::layout::{compute_graph, GraphRow};
use crate::message::{load_branches, load_commits, Message};
use crate::style::{
    ContextMenu, DragState, InspectState, DEFAULT_AUTHOR_WIDTH, DEFAULT_DATE_WIDTH,
    DEFAULT_HASH_WIDTH, LANE_WIDTH, PAGE_SIZE,
};
use crate::REPO_PATH;

pub(crate) struct App {
    pub(crate) repo_path: PathBuf,
    pub(crate) commits: Vec<CommitEntry>,
    pub(crate) graph_rows: Vec<GraphRow>,
    pub(crate) max_graph_cols: usize,
    pub(crate) selected_index: Option<usize>,
    pub(crate) loading: bool,
    pub(crate) all_loaded: bool,
    pub(crate) error: Option<String>,
    pub(crate) context_menu: Option<ContextMenu>,
    pub(crate) inspect: Option<InspectState>,
    pub(crate) graph_col_width: f32,
    pub(crate) graph_manually_sized: bool,
    pub(crate) window_width: f32,
    pub(crate) hash_width: f32,
    pub(crate) date_width: f32,
    pub(crate) author_width: f32,
    pub(crate) dragging: Option<DragState>,
    // Search state
    pub(crate) search_depth: String,
    pub(crate) search_hash: String,
    pub(crate) search_message: String,
    pub(crate) search_date: String,
    pub(crate) search_author: String,
    pub(crate) search_regex: bool,
    pub(crate) filtered_indices: Vec<usize>,
    pub(crate) search_loading: bool,
    // Branch selector state
    pub(crate) all_branches: Vec<String>,
    pub(crate) selected_branches: HashSet<String>,
    pub(crate) branch_filter_text: String,
    pub(crate) branch_dropdown_open: bool,
    pub(crate) branches_loading: bool,
}

impl App {
    pub(crate) fn new() -> (Self, Task<Message>) {
        let repo_path = REPO_PATH.get().expect("REPO_PATH not set").clone();
        let rp = repo_path.clone();
        let app = Self {
            repo_path,
            commits: Vec::new(),
            graph_rows: Vec::new(),
            max_graph_cols: 0,
            selected_index: None,
            loading: true,
            all_loaded: false,
            error: None,
            context_menu: None,
            inspect: None,
            graph_col_width: 120.0,
            graph_manually_sized: false,
            window_width: 1280.0,
            hash_width: DEFAULT_HASH_WIDTH,
            date_width: DEFAULT_DATE_WIDTH,
            author_width: DEFAULT_AUTHOR_WIDTH,
            dragging: None,
            search_depth: String::new(),
            search_hash: String::new(),
            search_message: String::new(),
            search_date: String::new(),
            search_author: String::new(),
            search_regex: false,
            filtered_indices: Vec::new(),
            search_loading: false,
            all_branches: Vec::new(),
            selected_branches: HashSet::new(),
            branch_filter_text: String::new(),
            branch_dropdown_open: false,
            branches_loading: true,
        };
        let rp2 = rp.clone();
        (
            app,
            Task::batch([
                Task::perform(load_commits(rp, 0, PAGE_SIZE), Message::CommitsLoaded),
                Task::perform(load_branches(rp2), Message::BranchesLoaded),
            ]),
        )
    }

    pub(crate) fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub(crate) fn subscription(&self) -> Subscription<Message> {
        event::listen().map(Message::GlobalEvent)
    }

    pub(crate) fn rebuild_graph(&mut self) {
        self.graph_rows = compute_graph(&self.commits);
        self.max_graph_cols = self
            .graph_rows
            .iter()
            .map(|r| r.num_cols)
            .max()
            .unwrap_or(0);
        if !self.graph_manually_sized {
            let natural =
                ((self.max_graph_cols.max(1)) as f32 * LANE_WIDTH + LANE_WIDTH).max(40.0);
            self.graph_col_width = natural.min(self.window_width / 2.0);
        }
    }

    pub(crate) fn has_active_search(&self) -> bool {
        !self.search_hash.is_empty()
            || !self.search_message.is_empty()
            || !self.search_date.is_empty()
            || !self.search_author.is_empty()
    }
}
