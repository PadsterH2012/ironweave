use std::path::Path;
use std::process::Command;

pub enum MergeResult {
    Success,
    Conflict { files: Vec<String> },
    Error(String),
}

pub struct MergeQueueProcessor;

impl MergeQueueProcessor {
    /// Run a git command in the given repo directory
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

    /// Attempt to merge a branch into the target branch using git CLI.
    pub fn try_merge(
        repo_path: &Path,
        source_branch: &str,
        target_branch: &str,
    ) -> crate::error::Result<MergeResult> {
        // Verify both branches exist
        if let Err(e) = Self::git(repo_path, &["rev-parse", "--verify", &format!("refs/heads/{}", source_branch)]) {
            return Ok(MergeResult::Error(format!("source branch '{}' not found: {}", source_branch, e)));
        }
        if let Err(e) = Self::git(repo_path, &["rev-parse", "--verify", &format!("refs/heads/{}", target_branch)]) {
            return Ok(MergeResult::Error(format!("target branch '{}' not found: {}", target_branch, e)));
        }

        // Check if already up to date (source is ancestor of target)
        if Self::git(repo_path, &["merge-base", "--is-ancestor", source_branch, target_branch]).is_ok() {
            return Ok(MergeResult::Success);
        }

        // Checkout target branch
        if let Err(e) = Self::git(repo_path, &["checkout", target_branch]) {
            return Ok(MergeResult::Error(format!("failed to checkout {}: {}", target_branch, e)));
        }

        // Attempt merge with no-edit (non-interactive)
        let merge_result = Self::git(repo_path, &[
            "merge",
            "--no-edit",
            "-m", &format!("ironweave: merge {} into {}", source_branch, target_branch),
            source_branch,
        ]);

        match merge_result {
            Ok(_) => Ok(MergeResult::Success),
            Err(stderr) => {
                // Check if it's a conflict
                if stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed") {
                    // Get list of conflicted files
                    let conflicts = Self::git(repo_path, &["diff", "--name-only", "--diff-filter=U"])
                        .unwrap_or_default();
                    let files: Vec<String> = conflicts
                        .lines()
                        .filter(|l| !l.is_empty())
                        .map(|l| l.to_string())
                        .collect();

                    // Abort the merge to leave repo clean
                    let _ = Self::git(repo_path, &["merge", "--abort"]);

                    Ok(MergeResult::Conflict { files })
                } else {
                    // Abort any partial merge
                    let _ = Self::git(repo_path, &["merge", "--abort"]);
                    Ok(MergeResult::Error(format!("merge failed: {}", stderr)))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn init_repo(path: &Path) {
        Command::new("git").args(["init", "-b", "main"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.name", "Test"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
        fs::write(path.join("README.md"), "# Init\n").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "initial"]).current_dir(path).output().unwrap();
    }

    fn commit_file(path: &Path, branch: &str, file: &str, content: &str) {
        Command::new("git").args(["checkout", branch]).current_dir(path).output().unwrap();
        fs::write(path.join(file), content).unwrap();
        Command::new("git").args(["add", file]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", &format!("add {}", file)]).current_dir(path).output().unwrap();
    }

    #[test]
    fn test_fast_forward_merge() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        Command::new("git").args(["checkout", "-b", "feature"]).current_dir(dir.path()).output().unwrap();
        commit_file(dir.path(), "feature", "new.txt", "hello\n");
        Command::new("git").args(["checkout", "main"]).current_dir(dir.path()).output().unwrap();

        let result = MergeQueueProcessor::try_merge(dir.path(), "feature", "main").unwrap();
        assert!(matches!(result, MergeResult::Success));

        // Verify new.txt exists on main
        let output = Command::new("git").args(["show", "main:new.txt"]).current_dir(dir.path()).output().unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn test_normal_merge_no_conflicts() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        Command::new("git").args(["checkout", "-b", "feature"]).current_dir(dir.path()).output().unwrap();
        commit_file(dir.path(), "feature", "feature.txt", "from feature\n");
        commit_file(dir.path(), "main", "main.txt", "from main\n");

        let result = MergeQueueProcessor::try_merge(dir.path(), "feature", "main").unwrap();
        assert!(matches!(result, MergeResult::Success));
    }

    #[test]
    fn test_conflict_detection() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        Command::new("git").args(["checkout", "-b", "feature"]).current_dir(dir.path()).output().unwrap();
        commit_file(dir.path(), "feature", "shared.txt", "feature version\n");
        commit_file(dir.path(), "main", "shared.txt", "main version\n");

        let result = MergeQueueProcessor::try_merge(dir.path(), "feature", "main").unwrap();
        match result {
            MergeResult::Conflict { files } => {
                assert!(files.contains(&"shared.txt".to_string()));
            }
            _ => panic!("Expected conflict"),
        }
    }
}
