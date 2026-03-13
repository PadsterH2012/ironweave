use std::fs;
use std::path::Path;

/// Maximum bytes to read from CLAUDE.md before truncating.
pub const CLAUDE_MD_MAX_BYTES: usize = 8192;

/// Maximum lines in the generated file tree before truncating.
pub const FILE_TREE_MAX_LINES: usize = 200;

/// Directory names to exclude from the file tree walk.
pub const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    ".next",
    "__pycache__",
    ".venv",
    "venv",
    ".svelte-kit",
];

/// File names to exclude from the file tree (secrets, OS junk).
const EXCLUDED_FILES: &[&str] = &[".env", ".DS_Store"];

/// Reads `CLAUDE.md` from the given directory, truncated to [`CLAUDE_MD_MAX_BYTES`].
///
/// Returns `None` if the file does not exist or cannot be read.
/// If the content exceeds the byte limit it is truncated and a marker is appended.
pub fn read_claude_md(dir: &Path) -> Option<String> {
    let path = dir.join("CLAUDE.md");
    let file = fs::File::open(&path).ok()?;
    let mut buf = String::new();
    use std::io::Read;
    file.take((CLAUDE_MD_MAX_BYTES + 1) as u64)
        .read_to_string(&mut buf)
        .ok()?;

    if buf.len() <= CLAUDE_MD_MAX_BYTES {
        Some(buf)
    } else {
        let truncated = safe_truncate(&buf, CLAUDE_MD_MAX_BYTES);
        Some(format!("{truncated}\n\n[...truncated at 8KB]"))
    }
}

/// Walks `dir` recursively and produces an indented file-tree listing.
///
/// Entries are sorted alphabetically at each level.  Directories end with `/`.
/// The tree is capped at [`FILE_TREE_MAX_LINES`] lines; if exceeded a truncation
/// marker is appended.
pub fn generate_file_tree(dir: &Path) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut truncated = false;
    walk_dir(dir, 0, &mut lines, &mut truncated);

    let mut output = lines.join("\n");
    if truncated {
        output.push('\n');
        output.push_str("[...truncated at 200 lines]");
    }
    output
}

const MAX_DEPTH: usize = 20;

/// Recursively collect tree lines for `dir` at the given `depth`.
fn walk_dir(dir: &Path, depth: usize, lines: &mut Vec<String>, truncated: &mut bool) {
    if depth >= MAX_DEPTH {
        return;
    }
    if *truncated {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    // Collect and sort entries alphabetically by file name.
    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let indent = "  ".repeat(depth);

    for entry in sorted {
        if lines.len() >= FILE_TREE_MAX_LINES {
            *truncated = true;
            return;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        if is_dir {
            if EXCLUDED_DIRS.contains(&name.as_ref()) {
                continue;
            }
            lines.push(format!("{indent}{name}/"));
            walk_dir(&entry.path(), depth + 1, lines, truncated);
        } else {
            if EXCLUDED_FILES.contains(&name.as_ref()) {
                continue;
            }
            lines.push(format!("{indent}{name}"));
        }
    }
}

/// Truncate a string to at most `max_bytes` bytes on a valid UTF-8 char boundary.
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_claude_md_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_claude_md(dir.path()).is_none());
    }

    #[test]
    fn read_claude_md_returns_content_when_small() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "hello").unwrap();
        let result = read_claude_md(dir.path()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn read_claude_md_truncates_large_content() {
        let dir = tempfile::tempdir().unwrap();
        let big = "x".repeat(CLAUDE_MD_MAX_BYTES + 500);
        fs::write(dir.path().join("CLAUDE.md"), &big).unwrap();
        let result = read_claude_md(dir.path()).unwrap();
        assert!(result.ends_with("[...truncated at 8KB]"));
        // The content before the marker should be exactly CLAUDE_MD_MAX_BYTES bytes.
        let prefix = result.strip_suffix("\n\n[...truncated at 8KB]").unwrap();
        assert_eq!(prefix.len(), CLAUDE_MD_MAX_BYTES);
    }

    #[test]
    fn generate_file_tree_excludes_dirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "").unwrap();
        fs::write(dir.path().join("README.md"), "").unwrap();

        let tree = generate_file_tree(dir.path());
        assert!(!tree.contains(".git"));
        assert!(!tree.contains("node_modules"));
        assert!(tree.contains("src/"));
        assert!(tree.contains("  main.rs"));
        assert!(tree.contains("README.md"));
    }

    #[test]
    fn generate_file_tree_truncates_at_limit() {
        let dir = tempfile::tempdir().unwrap();
        // Create more files than FILE_TREE_MAX_LINES
        for i in 0..250 {
            fs::write(dir.path().join(format!("file_{i:03}.txt")), "").unwrap();
        }
        let tree = generate_file_tree(dir.path());
        assert!(tree.ends_with("[...truncated at 200 lines]"));
        // Count actual content lines (excluding the truncation marker)
        let content = tree.strip_suffix("\n[...truncated at 200 lines]").unwrap();
        assert_eq!(content.lines().count(), FILE_TREE_MAX_LINES);
    }
}
