use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tokio::process::Command;

use gitgobig_core::Worktree;

/// Run a git command asynchronously via `tokio::process::Command`.
async fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd.output().await.context("failed to execute git")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn clone_bare(url: String, dest: PathBuf) -> Result<()> {
    if dest.exists() {
        bail!("destination already exists: {}", dest.display());
    }
    let dest_str = dest
        .to_str()
        .context("destination path is not valid UTF-8")?
        .to_string();
    run_git(
        &["clone", "--bare", "--filter=blob:none", &url, &dest_str],
        None,
    )
    .await?;
    Ok(())
}

pub async fn sync(repo_path: PathBuf) -> Result<String> {
    run_git(&["fetch", "--all", "--prune"], Some(&repo_path)).await
}

pub async fn worktree_add(
    repo_path: PathBuf,
    worktree_path: PathBuf,
    branch: String,
    new_branch: Option<String>,
) -> Result<()> {
    if worktree_path.exists() {
        bail!(
            "worktree target directory already exists: {}",
            worktree_path.display()
        );
    }
    let wt_str = worktree_path
        .to_str()
        .context("worktree path is not valid UTF-8")?
        .to_string();

    match new_branch {
        Some(ref name) => {
            run_git(
                &["worktree", "add", "-b", name, &wt_str, &branch],
                Some(&repo_path),
            )
            .await?;
        }
        None => {
            run_git(
                &["worktree", "add", &wt_str, &branch],
                Some(&repo_path),
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn worktree_list(repo_path: PathBuf) -> Result<Vec<Worktree>> {
    let output = run_git(&["worktree", "list", "--porcelain"], Some(&repo_path)).await?;
    Ok(parse_worktree_porcelain(&output))
}

pub async fn worktree_remove(repo_path: PathBuf, worktree_path: PathBuf) -> Result<()> {
    let wt_str = worktree_path
        .to_str()
        .context("worktree path is not valid UTF-8")?
        .to_string();
    run_git(&["worktree", "remove", &wt_str], Some(&repo_path)).await?;
    run_git(&["worktree", "prune"], Some(&repo_path)).await?;
    Ok(())
}

pub async fn branch_list(repo_path: PathBuf) -> Result<Vec<String>> {
    let output = run_git(&["branch", "-a"], Some(&repo_path)).await?;
    Ok(parse_branch_list(&output))
}

fn parse_worktree_porcelain(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut commit: Option<String> = None;
    let mut branch: Option<String> = None;

    for line in output.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(prev_path) = path.take() {
                worktrees.push(Worktree {
                    path: prev_path,
                    branch: branch.take(),
                    commit: commit.take(),
                });
            }
            path = Some(PathBuf::from(p));
            commit = None;
            branch = None;
        } else if let Some(h) = line.strip_prefix("HEAD ") {
            commit = Some(h.to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
        }
    }

    if let Some(p) = path {
        worktrees.push(Worktree {
            path: p,
            branch: branch.take(),
            commit: commit.take(),
        });
    }

    worktrees
}

fn parse_branch_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_start_matches("* ");
            if trimmed.is_empty() || trimmed.contains(" -> ") {
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_worktree_porcelain_parses_entries() {
        let input = "\
worktree /repos/project.git
HEAD abc123
branch refs/heads/main
bare

worktree /work/feature
HEAD def456
branch refs/heads/feature
";
        let wts = parse_worktree_porcelain(input);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].branch.as_deref(), Some("main"));
        assert_eq!(wts[1].branch.as_deref(), Some("feature"));
    }

    #[test]
    fn parse_branch_list_filters_symbolic_refs() {
        let input = "\
* main
  remotes/origin/HEAD -> origin/main
  remotes/origin/main
";
        let branches = parse_branch_list(input);
        assert_eq!(branches, vec!["main", "remotes/origin/main"]);
    }
}
