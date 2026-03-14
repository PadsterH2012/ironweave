use std::path::{Path, PathBuf};
use std::process::Command;

pub struct WorktreeManager {
    base_dir: PathBuf,
}

impl WorktreeManager {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }

    /// Run a git command in the given repo directory, returning stdout on success
    fn git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("failed to run git: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    /// Detect the default branch of a repo (main, master, or whatever HEAD points to)
    pub fn detect_default_branch(repo_path: &Path) -> Option<String> {
        // Try symbolic-ref HEAD first
        if let Ok(refname) = Self::git(repo_path, &["symbolic-ref", "--short", "HEAD"]) {
            if !refname.is_empty() {
                return Some(refname);
            }
        }
        // Fallback: check for common branch names
        for name in &["main", "master"] {
            if Self::git(repo_path, &["rev-parse", "--verify", &format!("refs/heads/{}", name)]).is_ok() {
                return Some(name.to_string());
            }
        }
        None
    }

    /// Create a worktree for an agent's task
    pub fn create_worktree(
        &self,
        repo_path: &Path,
        agent_id: &str,
        task_hash: &str,
        base_branch: &str,
    ) -> crate::error::Result<(PathBuf, String)> {
        let branch_name = format!("ironweave/{}/{}", agent_id, task_hash);
        let worktree_name = branch_name.replace('/', "-");
        let worktree_path = self.base_dir.join(&worktree_name);

        // If worktree path already exists from a failed attempt, clean up
        if worktree_path.exists() {
            let _ = std::fs::remove_dir_all(&worktree_path);
        }
        // Prune any stale worktree references
        let _ = Self::git(repo_path, &["worktree", "prune"]);

        // Delete existing branch if it exists (from a prior failed attempt)
        let _ = Self::git(repo_path, &["branch", "-D", &branch_name]);

        // Create worktree with new branch from base
        let wt_path_str = worktree_path.to_string_lossy().to_string();
        Self::git(repo_path, &["worktree", "add", "-b", &branch_name, &wt_path_str, base_branch])
            .map_err(|e| crate::error::IronweaveError::Internal(
                format!("git worktree add failed: {}", e)
            ))?;

        Ok((worktree_path, branch_name))
    }

    /// Remove a worktree after merge or abandonment
    pub fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_name: &str,
    ) -> crate::error::Result<()> {
        let worktree_path = self.base_dir.join(worktree_name);
        // Remove the worktree directory first
        if worktree_path.exists() {
            let _ = std::fs::remove_dir_all(&worktree_path);
        }
        // Prune stale worktree references
        let _ = Self::git(repo_path, &["worktree", "prune"]);
        // Delete the branch
        let branch_name = worktree_name.replace('-', "/").replacen("ironweave/", "ironweave/", 1);
        let _ = Self::git(repo_path, &["branch", "-D", &branch_name]);
        Ok(())
    }

    /// List all active worktrees
    pub fn list_worktrees(&self, repo_path: &Path) -> crate::error::Result<Vec<String>> {
        let output = Self::git(repo_path, &["worktree", "list", "--porcelain"])
            .map_err(|e| crate::error::IronweaveError::Internal(
                format!("git worktree list failed: {}", e)
            ))?;
        let worktrees: Vec<String> = output
            .lines()
            .filter_map(|line| {
                if let Some(path) = line.strip_prefix("worktree ") {
                    let name = Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string());
                    name
                } else {
                    None
                }
            })
            .collect();
        Ok(worktrees)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Helper: create a git repo with an initial commit on "main"
    fn init_repo_with_commit(path: &Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .output()
            .expect("git init failed");
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .expect("git config failed");
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()
            .expect("git config failed");
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "initial commit"])
            .current_dir(path)
            .output()
            .expect("git commit failed");
    }

    #[test]
    fn test_create_worktree() {
        let repo_dir = tempdir().expect("failed to create temp dir for repo");
        let wt_dir = tempdir().expect("failed to create temp dir for worktrees");

        init_repo_with_commit(repo_dir.path());
        let mgr = WorktreeManager::new(wt_dir.path().to_path_buf());

        let (wt_path, branch) = mgr
            .create_worktree(repo_dir.path(), "agent1", "abc123", "main")
            .expect("create_worktree failed");

        assert_eq!(branch, "ironweave/agent1/abc123");
        assert!(wt_path.exists(), "worktree directory should exist");

        // The worktree should have a .git file (not directory)
        assert!(wt_path.join(".git").exists());
    }

    #[test]
    fn test_remove_worktree() {
        let repo_dir = tempdir().expect("failed to create temp dir for repo");
        let wt_dir = tempdir().expect("failed to create temp dir for worktrees");

        init_repo_with_commit(repo_dir.path());
        let mgr = WorktreeManager::new(wt_dir.path().to_path_buf());

        let (wt_path, _branch) = mgr
            .create_worktree(repo_dir.path(), "agent1", "rm_test", "main")
            .expect("create_worktree failed");

        assert!(wt_path.exists());

        mgr.remove_worktree(repo_dir.path(), "ironweave-agent1-rm_test")
            .expect("remove_worktree failed");

        assert!(!wt_path.exists(), "worktree directory should be removed");
    }

    #[test]
    fn test_detect_default_branch() {
        let repo_dir = tempdir().expect("failed to create temp dir for repo");
        init_repo_with_commit(repo_dir.path());

        let branch = WorktreeManager::detect_default_branch(repo_dir.path());
        assert_eq!(branch, Some("main".to_string()));
    }
}
