mod app;
mod graph;
mod message;
mod style;
mod update;
mod view;
mod views;

use std::path::PathBuf;

use clap::Parser;

use gitgobig_core::git;

use crate::app::App;

/// gitgows — Git Worktree History Viewer
#[derive(Parser)]
#[command(name = "gitgows")]
struct Cli {
    /// Path to the worktree or bare repository.
    path: PathBuf,
}

fn main() -> iced::Result {
    let cli = Cli::parse();
    let path = cli.path;

    if !path.exists() {
        eprintln!("Error: path does not exist: {}", path.display());
        std::process::exit(1);
    }
    if !git::is_git_repo(&path) {
        eprintln!("Error: not a git repository: {}", path.display());
        std::process::exit(1);
    }

    REPO_PATH.set(path).expect("REPO_PATH already set");

    iced::application(App::new, App::update, App::view)
        .title(app_title)
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}

static REPO_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn app_title(app: &App) -> String {
    format!("gitgows — {}", app.repo_path.display())
}
