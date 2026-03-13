use git2::Repository;
use std::path::{Path, PathBuf};

pub struct WorktreeManager {
    base_dir: PathBuf,
}

impl WorktreeManager {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }

    /// Detect the default branch of a repo (main, master, or whatever HEAD points to)
    pub fn detect_default_branch(repo_path: &Path) -> Option<String> {
        let repo = Repository::open(repo_path).ok()?;
        // Try HEAD first
        if let Ok(head) = repo.head() {
            if let Some(name) = head.shorthand() {
                return Some(name.to_string());
            }
        }
        // Fallback: check for common branch names
        for name in &["main", "master"] {
            if repo.find_branch(name, git2::BranchType::Local).is_ok() {
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
        let repo = Repository::open(repo_path)?;
        let branch_name = format!("ironweave/{}/{}", agent_id, task_hash);
        let worktree_path = self.base_dir.join(&branch_name.replace('/', "-"));

        // Create branch from base (or reuse if it already exists from a previous attempt)
        let base = repo.find_branch(base_branch, git2::BranchType::Local)?;
        let commit = base.get().peel_to_commit()?;
        match repo.branch(&branch_name, &commit, false) {
            Ok(_) => {},
            Err(e) if e.code() == git2::ErrorCode::Exists => {
                // Branch already exists from a previous failed attempt — clean up old worktree first
                let old_wt_name = branch_name.replace('/', "-");
                if let Ok(wt) = repo.find_worktree(&old_wt_name) {
                    let mut prune_opts = git2::WorktreePruneOptions::new();
                    prune_opts.valid(true).working_tree(true);
                    let _ = wt.prune(Some(&mut prune_opts));
                }
                if worktree_path.exists() {
                    let _ = std::fs::remove_dir_all(&worktree_path);
                }
                // Delete and recreate the branch to get a clean state
                let mut existing = repo.find_branch(&branch_name, git2::BranchType::Local)?;
                existing.delete()?;
                repo.branch(&branch_name, &commit, false)?;
            },
            Err(e) => return Err(e.into()),
        }

        // Look up the newly created branch reference
        let ref_name = format!("refs/heads/{}", branch_name);
        let reference = repo.find_reference(&ref_name)?;

        // Create worktree
        let mut opts = git2::WorktreeAddOptions::new();
        opts.reference(Some(&reference));
        repo.worktree(
            &branch_name.replace('/', "-"),
            &worktree_path,
            Some(&mut opts),
        )?;

        Ok((worktree_path, branch_name))
    }

    /// Remove a worktree after merge or abandonment
    pub fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_name: &str,
    ) -> crate::error::Result<()> {
        let repo = Repository::open(repo_path)?;
        let wt = repo.find_worktree(worktree_name)?;
        let mut prune_opts = git2::WorktreePruneOptions::new();
        prune_opts.valid(true).working_tree(true);
        wt.prune(Some(&mut prune_opts))?;
        let worktree_path = self.base_dir.join(worktree_name);
        if worktree_path.exists() {
            std::fs::remove_dir_all(&worktree_path)?;
        }
        Ok(())
    }

    /// List all active worktrees
    pub fn list_worktrees(&self, repo_path: &Path) -> crate::error::Result<Vec<String>> {
        let repo = Repository::open(repo_path)?;
        Ok(repo
            .worktrees()?
            .iter()
            .filter_map(|s| s.map(String::from))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Helper: create a git repo with an initial commit on "main"
    fn init_repo_with_commit(path: &Path) -> Repository {
        let repo = Repository::init(path).expect("failed to init repo");

        // Configure a committer identity for the test repo
        let mut config = repo.config().expect("failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("failed to set user.email");

        // Create an initial commit so "main" branch exists
        {
            let sig = repo.signature().expect("failed to create signature");
            let tree_id = {
                let mut index = repo.index().expect("failed to get index");
                index.write_tree().expect("failed to write tree")
            };
            let tree = repo.find_tree(tree_id).expect("failed to find tree");
            repo.commit(Some("refs/heads/main"), &sig, &sig, "initial commit", &tree, &[])
                .expect("failed to create initial commit");
        }

        repo
    }

    #[test]
    fn test_create_worktree() {
        let repo_dir = tempdir().expect("failed to create temp dir for repo");
        let wt_dir = tempdir().expect("failed to create temp dir for worktrees");

        let _repo = init_repo_with_commit(repo_dir.path());
        let mgr = WorktreeManager::new(wt_dir.path().to_path_buf());

        let (wt_path, branch) = mgr
            .create_worktree(repo_dir.path(), "agent1", "abc123", "main")
            .expect("create_worktree failed");

        assert_eq!(branch, "ironweave/agent1/abc123");
        assert!(wt_path.exists(), "worktree directory should exist");

        // The worktree should be a valid git checkout
        let wt_repo = Repository::open(&wt_path).expect("should open worktree as repo");
        assert!(wt_repo.head().is_ok());
    }

    #[test]
    fn test_list_worktrees() {
        let repo_dir = tempdir().expect("failed to create temp dir for repo");
        let wt_dir = tempdir().expect("failed to create temp dir for worktrees");

        let _repo = init_repo_with_commit(repo_dir.path());
        let mgr = WorktreeManager::new(wt_dir.path().to_path_buf());

        // Initially no worktrees (beyond the main one, which git2 may or may not list)
        let before = mgr
            .list_worktrees(repo_dir.path())
            .expect("list_worktrees failed");

        // Create two worktrees
        mgr.create_worktree(repo_dir.path(), "agent1", "task1", "main")
            .expect("create first worktree");
        mgr.create_worktree(repo_dir.path(), "agent2", "task2", "main")
            .expect("create second worktree");

        let after = mgr
            .list_worktrees(repo_dir.path())
            .expect("list_worktrees failed");

        assert_eq!(
            after.len(),
            before.len() + 2,
            "should have two more worktrees"
        );
        assert!(after.contains(&"ironweave-agent1-task1".to_string()));
        assert!(after.contains(&"ironweave-agent2-task2".to_string()));
    }

    #[test]
    fn test_remove_worktree() {
        let repo_dir = tempdir().expect("failed to create temp dir for repo");
        let wt_dir = tempdir().expect("failed to create temp dir for worktrees");

        let _repo = init_repo_with_commit(repo_dir.path());
        let mgr = WorktreeManager::new(wt_dir.path().to_path_buf());

        let (wt_path, _branch) = mgr
            .create_worktree(repo_dir.path(), "agent1", "rm_test", "main")
            .expect("create_worktree failed");

        assert!(wt_path.exists());

        mgr.remove_worktree(repo_dir.path(), "ironweave-agent1-rm_test")
            .expect("remove_worktree failed");

        assert!(!wt_path.exists(), "worktree directory should be removed");
    }
}
