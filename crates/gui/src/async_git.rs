use std::path::{Path, PathBuf};

use anyhow::Result;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use gitgobig_core::{Worktree, git};

/// Events emitted by streaming git operations.
#[derive(Debug, Clone)]
pub enum GitEvent {
    /// A line of output (stdout or stderr) from the git process.
    Output(String),
    /// The operation completed successfully (with optional result payload).
    Done(GitResult),
    /// The operation failed.
    Failed(String),
}

/// Typed result payloads for different git operations.
#[derive(Debug, Clone)]
pub enum GitResult {
    CloneDone,
    SyncDone(String),
    WorktreeAddDone,
    WorktreeRemoveDone,
}

// ---------------------------------------------------------------------------
// Streaming runner (spawns git, pipes output line by line)
// ---------------------------------------------------------------------------

/// Run a git command, streaming stdout and stderr lines through the sender.
/// Returns the full stdout on success, or an error.
async fn run_git_streaming(
    args: &[&str],
    cwd: Option<&Path>,
    tx: &mut mpsc::Sender<GitEvent>,
) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // Show the command being run
    let cmd_line = format!("$ git {}", args.join(" "));
    let _ = tx.send(GitEvent::Output(cmd_line)).await;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut full_stdout = String::new();
    let mut full_stderr = String::new();

    // Read both streams concurrently until both are done.
    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if !full_stdout.is_empty() {
                            full_stdout.push('\n');
                        }
                        full_stdout.push_str(&l);
                        let _ = tx.send(GitEvent::Output(l)).await;
                    }
                    Ok(None) => {
                        // stdout closed — wait for stderr to finish, then break
                        while let Ok(Some(l)) = stderr_reader.next_line().await {
                            if !full_stderr.is_empty() {
                                full_stderr.push('\n');
                            }
                            full_stderr.push_str(&l);
                            let _ = tx.send(GitEvent::Output(l)).await;
                        }
                        break;
                    }
                    Err(e) => {
                        let _ = tx.send(GitEvent::Output(format!("[read error: {e}]"))).await;
                        break;
                    }
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if !full_stderr.is_empty() {
                            full_stderr.push('\n');
                        }
                        full_stderr.push_str(&l);
                        let _ = tx.send(GitEvent::Output(l)).await;
                    }
                    Ok(None) => {
                        // stderr closed — drain remaining stdout, then break
                        while let Ok(Some(l)) = stdout_reader.next_line().await {
                            if !full_stdout.is_empty() {
                                full_stdout.push('\n');
                            }
                            full_stdout.push_str(&l);
                            let _ = tx.send(GitEvent::Output(l)).await;
                        }
                        break;
                    }
                    Err(e) => {
                        let _ = tx.send(GitEvent::Output(format!("[read error: {e}]"))).await;
                        break;
                    }
                }
            }
        }
    }

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            full_stderr.trim()
        );
    }
    Ok(full_stdout)
}

// ---------------------------------------------------------------------------
// Streaming public API (return iced::stream channels)
// ---------------------------------------------------------------------------

pub fn clone_bare_stream(
    url: String,
    dest: PathBuf,
) -> impl iced::futures::Stream<Item = GitEvent> {
    iced::stream::channel(32, async move |mut tx| {
        // Validate before spawning
        if dest.exists() {
            let _ = tx
                .send(GitEvent::Failed(format!(
                    "destination already exists: {}",
                    dest.display()
                )))
                .await;
            return;
        }
        let dest_str = match dest.to_str() {
            Some(s) => s.to_string(),
            None => {
                let _ = tx
                    .send(GitEvent::Failed(
                        "destination path is not valid UTF-8".into(),
                    ))
                    .await;
                return;
            }
        };

        match run_git_streaming(
            &[
                "clone",
                "--bare",
                "--filter=blob:none",
                "--progress",
                &url,
                &dest_str,
            ],
            None,
            &mut tx,
        )
        .await
        {
            Ok(_) => {
                let _ = tx.send(GitEvent::Done(GitResult::CloneDone)).await;
            }
            Err(e) => {
                let _ = tx.send(GitEvent::Failed(e.to_string())).await;
            }
        }
    })
}

pub fn sync_stream(repo_path: PathBuf) -> impl iced::futures::Stream<Item = GitEvent> {
    iced::stream::channel(32, async move |mut tx| {
        match run_git_streaming(
            &["fetch", "--all", "--prune", "--progress"],
            Some(&repo_path),
            &mut tx,
        )
        .await
        {
            Ok(stdout) => {
                let _ = tx.send(GitEvent::Done(GitResult::SyncDone(stdout))).await;
            }
            Err(e) => {
                let _ = tx.send(GitEvent::Failed(e.to_string())).await;
            }
        }
    })
}

pub fn worktree_add_stream(
    repo_path: PathBuf,
    worktree_path: PathBuf,
    branch: String,
    new_branch: Option<String>,
) -> impl iced::futures::Stream<Item = GitEvent> {
    iced::stream::channel(32, async move |mut tx| {
        if worktree_path.exists() {
            let _ = tx
                .send(GitEvent::Failed(format!(
                    "worktree target directory already exists: {}",
                    worktree_path.display()
                )))
                .await;
            return;
        }
        let wt_str = match worktree_path.to_str() {
            Some(s) => s.to_string(),
            None => {
                let _ = tx
                    .send(GitEvent::Failed("worktree path is not valid UTF-8".into()))
                    .await;
                return;
            }
        };

        let args: Vec<String> = match &new_branch {
            Some(name) => vec![
                "worktree".into(),
                "add".into(),
                "-b".into(),
                name.clone(),
                wt_str,
                branch,
            ],
            None => vec!["worktree".into(), "add".into(), wt_str, branch],
        };
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        match run_git_streaming(&arg_refs, Some(&repo_path), &mut tx).await {
            Ok(_) => {
                let _ = tx.send(GitEvent::Done(GitResult::WorktreeAddDone)).await;
            }
            Err(e) => {
                let _ = tx.send(GitEvent::Failed(e.to_string())).await;
            }
        }
    })
}

pub fn worktree_remove_stream(
    repo_path: PathBuf,
    worktree_path: PathBuf,
) -> impl iced::futures::Stream<Item = GitEvent> {
    iced::stream::channel(32, async move |mut tx| {
        let wt_str = match worktree_path.to_str() {
            Some(s) => s.to_string(),
            None => {
                let _ = tx
                    .send(GitEvent::Failed("worktree path is not valid UTF-8".into()))
                    .await;
                return;
            }
        };

        match run_git_streaming(&["worktree", "remove", &wt_str], Some(&repo_path), &mut tx).await {
            Ok(_) => {
                // Also prune
                let _ = run_git_streaming(&["worktree", "prune"], Some(&repo_path), &mut tx).await;
                let _ = tx.send(GitEvent::Done(GitResult::WorktreeRemoveDone)).await;
            }
            Err(e) => {
                let _ = tx.send(GitEvent::Failed(e.to_string())).await;
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Non-streaming helpers (keep for lightweight queries)
// ---------------------------------------------------------------------------

pub async fn worktree_list(repo_path: PathBuf) -> Result<Vec<Worktree>> {
    tokio::task::spawn_blocking(move || git::worktree_list(&repo_path)).await?
}

pub async fn branch_list(repo_path: PathBuf) -> Result<Vec<String>> {
    tokio::task::spawn_blocking(move || git::branch_list(&repo_path)).await?
}
