use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::Worktree;

// ---------------------------------------------------------------------------
// Preflight
// ---------------------------------------------------------------------------

/// Check that `git` is installed and reachable on `$PATH`.
/// Returns the version string on success.
pub fn check_git_installed() -> Result<String> {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .context("git is not installed or not found on PATH")?;
    if !output.status.success() {
        bail!("git --version returned a non-zero exit code");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a git command with the given args, returning stdout on success.
/// On non-zero exit, returns an error containing stderr.
fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd.output().context("failed to execute git")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.first().unwrap_or(&""), stderr.trim());
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(stdout)
}

/// Basic URL validation – must be non-empty and look like an http(s)/ssh/git URL
/// or a local path. This is intentionally permissive; git itself will reject
/// truly invalid URLs.
fn validate_url(url: &str) -> Result<()> {
    let url = url.trim();
    if url.is_empty() {
        bail!("repository URL must not be empty");
    }
    // Reject strings that contain shell meta-characters that could be dangerous
    // even though we use Command::arg (defense in depth).
    if url.contains(';') || url.contains('|') || url.contains('$') || url.contains('`') {
        bail!("repository URL contains disallowed characters");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Clone
// ---------------------------------------------------------------------------

/// Clone a remote repository as a bare repo with partial clone (blob filter).
///
/// Runs: `git clone --bare --filter=blob:none <url> <dest>`
pub fn clone_bare(url: &str, dest: &Path) -> Result<()> {
    validate_url(url)?;
    if dest.exists() {
        bail!("destination already exists: {}", dest.display());
    }
    let dest_str = dest
        .to_str()
        .context("destination path is not valid UTF-8")?;
    run_git(
        &["clone", "--bare", "--filter=blob:none", url, dest_str],
        None,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

/// Fetch all remotes and prune stale tracking branches in a bare repo.
///
/// Runs: `git fetch --all --prune` inside `repo_path`.
pub fn sync(repo_path: &Path) -> Result<String> {
    run_git(&["fetch", "--all", "--prune"], Some(repo_path))
}

// ---------------------------------------------------------------------------
// Worktree Add
// ---------------------------------------------------------------------------

/// Add a new worktree from the bare repo.
///
/// If `new_branch` is `Some(name)`, creates a new branch at `start_point`.
/// Otherwise checks out the existing `branch_or_commit`.
///
/// Validates that `worktree_path` does not already exist.
pub fn worktree_add(
    repo_path: &Path,
    worktree_path: &Path,
    branch_or_commit: &str,
    new_branch: Option<&str>,
) -> Result<()> {
    if worktree_path.exists() {
        bail!(
            "worktree target directory already exists: {}",
            worktree_path.display()
        );
    }
    let wt_str = worktree_path
        .to_str()
        .context("worktree path is not valid UTF-8")?;

    match new_branch {
        Some(name) => {
            run_git(
                &["worktree", "add", "-b", name, wt_str, branch_or_commit],
                Some(repo_path),
            )?;
        }
        None => {
            run_git(
                &["worktree", "add", wt_str, branch_or_commit],
                Some(repo_path),
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Worktree List
// ---------------------------------------------------------------------------

/// List worktrees from a bare repo by parsing `git worktree list --porcelain`.
pub fn worktree_list(repo_path: &Path) -> Result<Vec<Worktree>> {
    let output = run_git(&["worktree", "list", "--porcelain"], Some(repo_path))?;
    Ok(parse_worktree_porcelain(&output))
}

/// Parse the porcelain output of `git worktree list --porcelain` into a vec
/// of `Worktree` structs.
///
/// Example porcelain block:
/// ```text
/// worktree /path/to/wt
/// HEAD abc123
/// branch refs/heads/main
/// ```
fn parse_worktree_porcelain(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut commit: Option<String> = None;
    let mut branch: Option<String> = None;

    for line in output.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            // If we already accumulated a worktree, push it before starting the next.
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
            // Strip refs/heads/ prefix for readability.
            branch = Some(
                b.strip_prefix("refs/heads/")
                    .unwrap_or(b)
                    .to_string(),
            );
        }
        // Ignore other lines (bare, detached, prunable, etc.)
    }

    // Push the last accumulated worktree if any.
    if let Some(p) = path {
        worktrees.push(Worktree {
            path: p,
            branch: branch.take(),
            commit: commit.take(),
        });
    }

    worktrees
}

// ---------------------------------------------------------------------------
// Worktree Remove
// ---------------------------------------------------------------------------

/// Remove a worktree and prune stale worktree metadata.
///
/// Runs: `git worktree remove <path>` then `git worktree prune`.
pub fn worktree_remove(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let wt_str = worktree_path
        .to_str()
        .context("worktree path is not valid UTF-8")?;
    run_git(&["worktree", "remove", wt_str], Some(repo_path))?;
    run_git(&["worktree", "prune"], Some(repo_path))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Branch List
// ---------------------------------------------------------------------------

/// List all branches (local + remote) from a bare repo.
///
/// Runs: `git branch -a` and parses the output into a sorted list of names.
pub fn branch_list(repo_path: &Path) -> Result<Vec<String>> {
    let output = run_git(&["branch", "-a"], Some(repo_path))?;
    Ok(parse_branch_list(&output))
}

/// Parse `git branch -a` output into a vec of branch name strings.
fn parse_branch_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_start_matches("* ");
            if trimmed.is_empty() || trimmed.contains(" -> ") {
                // Skip empty lines and symbolic refs like `remotes/origin/HEAD -> origin/main`
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // -- check_git_installed ------------------------------------------------

    #[test]
    fn check_git_installed_returns_version() {
        let version = check_git_installed().unwrap();
        assert!(version.starts_with("git version"));
    }

    // -- validate_url -------------------------------------------------------

    #[test]
    fn validate_url_rejects_empty() {
        assert!(validate_url("").is_err());
        assert!(validate_url("   ").is_err());
    }

    #[test]
    fn validate_url_rejects_shell_metacharacters() {
        assert!(validate_url("https://x.com/repo;rm -rf /").is_err());
        assert!(validate_url("https://x.com/repo|cat /etc/passwd").is_err());
        assert!(validate_url("$(malicious)").is_err());
        assert!(validate_url("`id`").is_err());
    }

    #[test]
    fn validate_url_accepts_valid_urls() {
        assert!(validate_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_url("git@github.com:user/repo.git").is_ok());
        assert!(validate_url("/local/path/to/repo").is_ok());
    }

    // -- parse_worktree_porcelain -------------------------------------------

    #[test]
    fn parse_worktree_porcelain_parses_multiple_entries() {
        let input = "\
worktree /repos/project.git
HEAD abc123def456
branch refs/heads/main
bare

worktree /work/feature-a
HEAD 789def012345
branch refs/heads/feature-a

worktree /work/detached
HEAD deadbeef1234
detached
";
        let wts = parse_worktree_porcelain(input);

        assert_eq!(wts.len(), 3);

        assert_eq!(wts[0].path, PathBuf::from("/repos/project.git"));
        assert_eq!(wts[0].commit.as_deref(), Some("abc123def456"));
        assert_eq!(wts[0].branch.as_deref(), Some("main"));

        assert_eq!(wts[1].path, PathBuf::from("/work/feature-a"));
        assert_eq!(wts[1].branch.as_deref(), Some("feature-a"));

        assert_eq!(wts[2].path, PathBuf::from("/work/detached"));
        assert_eq!(wts[2].commit.as_deref(), Some("deadbeef1234"));
        assert_eq!(wts[2].branch, None);
    }

    #[test]
    fn parse_worktree_porcelain_handles_empty() {
        assert!(parse_worktree_porcelain("").is_empty());
    }

    // -- parse_branch_list --------------------------------------------------

    #[test]
    fn parse_branch_list_strips_markers_and_symbolic_refs() {
        let input = "\
* main
  develop
  remotes/origin/HEAD -> origin/main
  remotes/origin/main
  remotes/origin/develop
";
        let branches = parse_branch_list(input);

        assert_eq!(
            branches,
            vec!["main", "develop", "remotes/origin/main", "remotes/origin/develop"]
        );
    }

    #[test]
    fn parse_branch_list_handles_empty() {
        assert!(parse_branch_list("").is_empty());
    }

    // -- Integration-style tests using real git repos -----------------------
    // These tests create temporary repos and verify real git operations.

    /// Helper: init a bare repo and seed it with one commit so branches exist.
    fn setup_bare_repo() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();

        // Create a regular repo with one commit.
        let src = dir.path().join("src_repo");
        Command::new("git").args(["init", src.to_str().unwrap()]).output().unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&src)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&src)
            .output()
            .unwrap();
        std::fs::write(src.join("file.txt"), "hello").unwrap();
        Command::new("git").args(["add", "."]).current_dir(&src).output().unwrap();
        Command::new("git")
            .args(["-c", "commit.gpgsign=false", "commit", "-m", "init"])
            .current_dir(&src)
            .output()
            .unwrap();

        // Clone as bare.
        let bare = dir.path().join("bare.git");
        Command::new("git")
            .args(["clone", "--bare", src.to_str().unwrap(), bare.to_str().unwrap()])
            .output()
            .unwrap();

        (dir, bare, src)
    }

    #[test]
    fn clone_bare_creates_bare_repo() {
        let dir = tempfile::tempdir().unwrap();

        // Create a local source repo with one commit.
        let src = dir.path().join("source");
        Command::new("git").args(["init", src.to_str().unwrap()]).output().unwrap();
        Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(&src)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(&src)
            .output()
            .unwrap();
        std::fs::write(src.join("f.txt"), "x").unwrap();
        Command::new("git").args(["add", "."]).current_dir(&src).output().unwrap();
        Command::new("git")
            .args(["-c", "commit.gpgsign=false", "commit", "-m", "init"])
            .current_dir(&src)
            .output()
            .unwrap();

        let dest = dir.path().join("cloned.git");
        clone_bare(src.to_str().unwrap(), &dest).unwrap();

        // A bare repo has a HEAD file directly in the directory.
        assert!(dest.join("HEAD").exists());
    }

    #[test]
    fn clone_bare_rejects_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("existing");
        std::fs::create_dir(&dest).unwrap();
        assert!(clone_bare("https://example.com/repo.git", &dest).is_err());
    }

    #[test]
    fn sync_fetches_in_bare_repo() {
        let (_dir, bare, _src) = setup_bare_repo();
        let result = sync(&bare);
        // Fetch should succeed (even if nothing new to fetch).
        assert!(result.is_ok());
    }

    #[test]
    fn worktree_add_list_remove_roundtrip() {
        let (_dir, bare, _src) = setup_bare_repo();

        // Determine the default branch name.
        let branches = branch_list(&bare).unwrap();
        let default_branch = branches.first().unwrap().clone();

        // Add a worktree.
        let wt_path = _dir.path().join("wt-test");
        worktree_add(&bare, &wt_path, &default_branch, None).unwrap();
        assert!(wt_path.join("file.txt").exists());

        // List should include the worktree.
        let wts = worktree_list(&bare).unwrap();
        assert!(wts.iter().any(|w| w.path == wt_path));

        // Remove the worktree.
        worktree_remove(&bare, &wt_path).unwrap();
        let wts_after = worktree_list(&bare).unwrap();
        assert!(!wts_after.iter().any(|w| w.path == wt_path));
    }

    #[test]
    fn worktree_add_with_new_branch() {
        let (_dir, bare, _src) = setup_bare_repo();

        let branches = branch_list(&bare).unwrap();
        let default_branch = branches.first().unwrap().clone();

        let wt_path = _dir.path().join("wt-new-branch");
        worktree_add(&bare, &wt_path, &default_branch, Some("my-feature")).unwrap();

        // The new worktree should be on the new branch.
        let wts = worktree_list(&bare).unwrap();
        let wt = wts.iter().find(|w| w.path == wt_path).unwrap();
        assert_eq!(wt.branch.as_deref(), Some("my-feature"));
    }

    #[test]
    fn worktree_add_rejects_existing_path() {
        let (_dir, bare, _src) = setup_bare_repo();
        let existing = _dir.path().join("existing-wt");
        std::fs::create_dir(&existing).unwrap();
        assert!(worktree_add(&bare, &existing, "main", None).is_err());
    }

    #[test]
    fn branch_list_returns_branches() {
        let (_dir, bare, _src) = setup_bare_repo();
        let branches = branch_list(&bare).unwrap();
        assert!(!branches.is_empty());
    }

    #[test]
    fn clone_bare_rejects_invalid_remote() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("will-not-exist.git");
        // Use a non-existent local path as remote — git fails immediately.
        let result = clone_bare("/nonexistent/path/to/repo.git", &dest);
        assert!(result.is_err());
    }

    #[test]
    fn worktree_add_rejects_non_empty_existing_dir() {
        let (_dir, bare, _src) = setup_bare_repo();
        let non_empty = _dir.path().join("non-empty-wt");
        std::fs::create_dir(&non_empty).unwrap();
        std::fs::write(non_empty.join("file.txt"), "content").unwrap();
        let err = worktree_add(&bare, &non_empty, "main", None);
        assert!(err.is_err());
        assert!(
            err.unwrap_err().to_string().contains("already exists"),
            "error should mention the directory already exists"
        );
    }

    #[test]
    fn worktree_remove_nonexistent_fails() {
        let (_dir, bare, _src) = setup_bare_repo();
        let missing = _dir.path().join("does-not-exist");
        let result = worktree_remove(&bare, &missing);
        assert!(result.is_err());
    }
}
