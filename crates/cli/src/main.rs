use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use gitgobig_config::{load_state, save_state};
use gitgobig_core::git;
use gitgobig_core::Repository;

#[derive(Parser)]
#[command(name = "gitgobig", about = "Manage large Git repos via bare clones and worktrees")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone a repository as a bare repo with partial clone
    Clone {
        /// Remote URL to clone
        url: String,
        /// Local directory for the bare clone
        dest: PathBuf,
    },
    /// Fetch all remotes and prune stale branches in a tracked repo
    Sync {
        /// Name of a tracked repository (as shown in `list`)
        name: String,
    },
    /// Worktree operations
    #[command(subcommand)]
    Worktree(WorktreeCmd),
}

#[derive(Subcommand)]
enum WorktreeCmd {
    /// Create a new worktree from a tracked bare repo
    Add {
        /// Name of a tracked repository
        repo: String,
        /// Local path for the new worktree
        path: PathBuf,
        /// Branch or commit to check out
        branch: String,
        /// Optionally create a new branch with this name
        #[arg(short = 'b', long)]
        new_branch: Option<String>,
    },
    /// List worktrees for a tracked bare repo
    List {
        /// Name of a tracked repository
        repo: String,
    },
    /// Remove a worktree from a tracked bare repo
    Remove {
        /// Name of a tracked repository
        repo: String,
        /// Path of the worktree to remove
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    git::check_git_installed().context("gitgobig requires git to be installed and on your PATH")?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Clone { url, dest } => cmd_clone(&url, &dest),
        Commands::Sync { name } => cmd_sync(&name),
        Commands::Worktree(wt) => match wt {
            WorktreeCmd::Add {
                repo,
                path,
                branch,
                new_branch,
            } => cmd_worktree_add(&repo, &path, &branch, new_branch.as_deref()),
            WorktreeCmd::List { repo } => cmd_worktree_list(&repo),
            WorktreeCmd::Remove { repo, path } => cmd_worktree_remove(&repo, &path),
        },
    }
}

fn cmd_clone(url: &str, dest: &PathBuf) -> Result<()> {
    println!("Cloning {} into {} …", url, dest.display());
    git::clone_bare(url, dest)?;

    // Derive a display name from the URL.
    let name = url
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .to_string();

    // Persist the new repo in app state.
    let mut state = load_state()?;
    if state.repositories.iter().any(|r| r.path == *dest) {
        bail!("a repository at {} is already tracked", dest.display());
    }
    state.repositories.push(Repository {
        name: name.clone(),
        path: dest.clone(),
        url: url.to_string(),
        worktrees: vec![],
    });
    save_state(&state)?;

    println!("Cloned and tracked as \"{}\"", name);
    Ok(())
}

fn cmd_sync(name: &str) -> Result<()> {
    let state = load_state()?;
    let repo = find_repo(&state.repositories, name)?;
    println!("Syncing {} …", repo.path.display());
    let output = git::sync(&repo.path)?;
    if !output.trim().is_empty() {
        println!("{}", output.trim());
    }
    println!("Done.");
    Ok(())
}

fn cmd_worktree_add(
    repo_name: &str,
    path: &PathBuf,
    branch: &str,
    new_branch: Option<&str>,
) -> Result<()> {
    let mut state = load_state()?;
    let repo = find_repo_mut(&mut state.repositories, repo_name)?;
    git::worktree_add(&repo.path, path, branch, new_branch)?;

    // Record the worktree in persisted state.
    repo.worktrees.push(gitgobig_core::Worktree {
        path: path.clone(),
        branch: new_branch.map(String::from).or_else(|| Some(branch.to_string())),
        commit: None,
    });
    save_state(&state)?;

    println!(
        "Worktree created at {} ({})",
        path.display(),
        new_branch.unwrap_or(branch)
    );
    Ok(())
}

fn cmd_worktree_list(repo_name: &str) -> Result<()> {
    let state = load_state()?;
    let repo = find_repo(&state.repositories, repo_name)?;
    let worktrees = git::worktree_list(&repo.path)?;

    if worktrees.is_empty() {
        println!("No worktrees.");
        return Ok(());
    }

    for wt in &worktrees {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let commit = wt.commit.as_deref().map(|c| &c[..c.len().min(12)]).unwrap_or("?");
        println!("{}\t{}\t{}", wt.path.display(), branch, commit);
    }
    Ok(())
}

fn cmd_worktree_remove(repo_name: &str, path: &PathBuf) -> Result<()> {
    let mut state = load_state()?;
    let repo = find_repo_mut(&mut state.repositories, repo_name)?;
    git::worktree_remove(&repo.path, path)?;

    // Remove from persisted state.
    repo.worktrees.retain(|w| w.path != *path);
    save_state(&state)?;

    println!("Worktree removed: {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_repo<'a>(repos: &'a [Repository], name: &str) -> Result<&'a Repository> {
    repos
        .iter()
        .find(|r| r.name == name)
        .ok_or_else(|| anyhow::anyhow!("no tracked repository named \"{}\"", name))
}

fn find_repo_mut<'a>(repos: &'a mut [Repository], name: &str) -> Result<&'a mut Repository> {
    repos
        .iter_mut()
        .find(|r| r.name == name)
        .ok_or_else(|| anyhow::anyhow!("no tracked repository named \"{}\"", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_repo_returns_matching() {
        let repos = vec![Repository {
            name: "foo".into(),
            path: PathBuf::from("/tmp/foo.git"),
            url: "https://example.com/foo.git".into(),
            worktrees: vec![],
        }];
        assert!(find_repo(&repos, "foo").is_ok());
    }

    #[test]
    fn find_repo_errors_on_missing() {
        let repos: Vec<Repository> = vec![];
        assert!(find_repo(&repos, "nope").is_err());
    }

    #[test]
    fn cli_parses_clone() {
        let cli = Cli::try_parse_from(["gitgobig", "clone", "https://x.com/r.git", "/tmp/r"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn cli_parses_sync() {
        let cli = Cli::try_parse_from(["gitgobig", "sync", "myrepo"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn cli_parses_worktree_add() {
        let cli = Cli::try_parse_from([
            "gitgobig", "worktree", "add", "myrepo", "/tmp/wt", "main",
        ]);
        assert!(cli.is_ok());
    }

    #[test]
    fn cli_parses_worktree_add_with_new_branch() {
        let cli = Cli::try_parse_from([
            "gitgobig", "worktree", "add", "myrepo", "/tmp/wt", "main", "-b", "feat",
        ]);
        assert!(cli.is_ok());
    }

    #[test]
    fn cli_parses_worktree_list() {
        let cli = Cli::try_parse_from(["gitgobig", "worktree", "list", "myrepo"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn cli_parses_worktree_remove() {
        let cli = Cli::try_parse_from(["gitgobig", "worktree", "remove", "myrepo", "/tmp/wt"]);
        assert!(cli.is_ok());
    }
}
