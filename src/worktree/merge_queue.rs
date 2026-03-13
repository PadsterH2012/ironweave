use git2::{MergeOptions, Repository};
use std::path::Path;

pub enum MergeResult {
    Success,
    Conflict { files: Vec<String> },
    Error(String),
}

pub struct MergeQueueProcessor;

impl MergeQueueProcessor {
    /// Attempt to merge a branch into the target branch.
    ///
    /// Performs merge analysis first: if already up-to-date returns Success,
    /// if fast-forwardable updates the target ref, otherwise attempts a
    /// normal merge and reports any conflicts.
    pub fn try_merge(
        repo_path: &Path,
        source_branch: &str,
        target_branch: &str,
    ) -> crate::error::Result<MergeResult> {
        let repo = Repository::open(repo_path)?;

        let source = repo.find_branch(source_branch, git2::BranchType::Local)?;
        let source_commit = source.get().peel_to_commit()?;

        let target = repo.find_branch(target_branch, git2::BranchType::Local)?;
        let target_commit = target.get().peel_to_commit()?;

        // Set HEAD to target branch so merge_analysis works correctly
        repo.set_head(&format!("refs/heads/{}", target_branch))?;

        let source_annotated = repo.find_annotated_commit(source_commit.id())?;

        // Check merge analysis
        let (analysis, _) = repo.merge_analysis(&[&source_annotated])?;

        if analysis.is_up_to_date() {
            return Ok(MergeResult::Success);
        }

        if analysis.is_fast_forward() {
            // Fast-forward merge
            let mut target_ref =
                repo.find_reference(&format!("refs/heads/{}", target_branch))?;
            target_ref.set_target(source_commit.id(), "ironweave: fast-forward merge")?;
            return Ok(MergeResult::Success);
        }

        // Normal merge — check for conflicts
        let mut index = repo.merge_commits(
            &target_commit,
            &source_commit,
            Some(&MergeOptions::new()),
        )?;

        if index.has_conflicts() {
            let conflicts: Vec<String> = index
                .conflicts()?
                .filter_map(|c| c.ok())
                .filter_map(|c| {
                    c.our
                        .map(|e| String::from_utf8_lossy(&e.path).to_string())
                })
                .collect();
            return Ok(MergeResult::Conflict { files: conflicts });
        }

        // No conflicts — create merge commit
        let oid = index.write_tree_to(&repo)?;
        let tree = repo.find_tree(oid)?;
        let sig = repo
            .signature()
            .unwrap_or_else(|_| git2::Signature::now("ironweave", "ironweave@local").unwrap());
        repo.commit(
            Some(&format!("refs/heads/{}", target_branch)),
            &sig,
            &sig,
            &format!(
                "ironweave: merge {} into {}",
                source_branch, target_branch
            ),
            &tree,
            &[&target_commit, &source_commit],
        )?;

        Ok(MergeResult::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Oid, Repository, Signature};
    use std::fs;
    use std::path::Path;

    /// Create an initial commit on the repo so HEAD and "main" exist.
    fn init_repo_with_commit(path: &Path) -> Repository {
        let repo = Repository::init(path).unwrap();
        let sig = Signature::now("test", "test@test.com").unwrap();

        // Create an initial file and commit
        fs::write(path.join("README.md"), "# Init\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();

            repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
                .unwrap();
        }

        // Make sure "main" branch exists (HEAD points to it)
        {
            let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
            repo.branch("main", &head_commit, true).unwrap();
        }

        repo
    }

    /// Helper: create a commit on a given branch that writes `content` to `file`.
    fn commit_file_on_branch(
        repo: &Repository,
        branch_name: &str,
        file_name: &str,
        content: &str,
        message: &str,
    ) -> Oid {
        let sig = Signature::now("test", "test@test.com").unwrap();

        let parent_commit = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();

        // Write file to workdir
        let workdir = repo.workdir().unwrap();
        fs::write(workdir.join(file_name), content).unwrap();

        // Build a new tree from the parent tree + the new blob
        let blob_oid = repo.blob(content.as_bytes()).unwrap();
        let mut builder = repo
            .treebuilder(Some(&parent_commit.tree().unwrap()))
            .unwrap();
        builder
            .insert(file_name, blob_oid, 0o100644)
            .unwrap();
        let tree_oid = builder.write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();

        repo.commit(
            Some(&format!("refs/heads/{}", branch_name)),
            &sig,
            &sig,
            message,
            &tree,
            &[&parent_commit],
        )
        .unwrap()
    }

    #[test]
    fn test_fast_forward_merge() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(dir.path());

        // Create feature branch from main
        let main_commit = repo
            .find_branch("main", git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();
        repo.branch("feature", &main_commit, false).unwrap();

        // Add a commit only on feature (main stays behind)
        let feature_oid =
            commit_file_on_branch(&repo, "feature", "new_file.txt", "hello\n", "add file");

        // Merge feature into main — should fast-forward
        let result =
            MergeQueueProcessor::try_merge(dir.path(), "feature", "main").unwrap();

        assert!(matches!(result, MergeResult::Success));

        // Verify main now points at the same commit as feature
        let main_tip = repo
            .find_branch("main", git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap()
            .id();
        assert_eq!(main_tip, feature_oid);
    }

    #[test]
    fn test_normal_merge_no_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(dir.path());

        // Create feature branch from main
        let main_commit = repo
            .find_branch("main", git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();
        repo.branch("feature", &main_commit, false).unwrap();

        // Add different files on each branch so they diverge without conflict
        commit_file_on_branch(&repo, "main", "main_file.txt", "from main\n", "main work");
        commit_file_on_branch(
            &repo,
            "feature",
            "feature_file.txt",
            "from feature\n",
            "feature work",
        );

        let result =
            MergeQueueProcessor::try_merge(dir.path(), "feature", "main").unwrap();

        assert!(matches!(result, MergeResult::Success));

        // After merge, main's tree should contain both files
        let main_commit = repo
            .find_branch("main", git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();
        let tree = main_commit.tree().unwrap();
        assert!(tree.get_name("main_file.txt").is_some());
        assert!(tree.get_name("feature_file.txt").is_some());

        // Should be a merge commit with 2 parents
        assert_eq!(main_commit.parent_count(), 2);
    }

    #[test]
    fn test_conflict_detection() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo_with_commit(dir.path());

        // Create feature branch from main
        let main_commit = repo
            .find_branch("main", git2::BranchType::Local)
            .unwrap()
            .get()
            .peel_to_commit()
            .unwrap();
        repo.branch("feature", &main_commit, false).unwrap();

        // Both branches modify the same file with different content
        commit_file_on_branch(
            &repo,
            "main",
            "shared.txt",
            "main version\n",
            "main edits shared",
        );
        commit_file_on_branch(
            &repo,
            "feature",
            "shared.txt",
            "feature version\n",
            "feature edits shared",
        );

        let result =
            MergeQueueProcessor::try_merge(dir.path(), "feature", "main").unwrap();

        match result {
            MergeResult::Conflict { files } => {
                assert!(!files.is_empty());
                assert!(files.contains(&"shared.txt".to_string()));
            }
            _ => panic!("Expected conflict, got success or error"),
        }
    }
}
