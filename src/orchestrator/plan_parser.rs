use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

/// A task extracted from a plan markdown file.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedTask {
    pub task_number: usize,
    pub title: String,
    pub description: String,
    pub role: Option<String>,
    pub depends_on_task_numbers: Vec<usize>,
}

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^### Task (\d+):\s*(.+)$").unwrap());
static ROLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\*\*Role:\*\*\s*(.+)$").unwrap());
static DEPENDS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\*\*Depends on:\*\*\s*(.+)$").unwrap());

/// Parse a plan markdown file into a list of tasks.
///
/// Splits on `### Task N: Title` headings. Each task body becomes the description
/// (with metadata lines stripped). Supports optional `**Role:**` and `**Depends on:**`
/// lines within task body. If no explicit depends_on, each task depends on the previous one.
pub fn parse_plan(content: &str) -> Vec<ParsedTask> {
    let headings: Vec<_> = HEADING_RE
        .captures_iter(content)
        .map(|cap| {
            let full_match = cap.get(0).unwrap();
            let task_number: usize = cap[1].parse().unwrap_or(0);
            let title = cap[2].trim().to_string();
            (task_number, title, full_match.start(), full_match.end())
        })
        .collect();

    let mut tasks = Vec::new();

    for (i, (task_number, title, _start, heading_end)) in headings.iter().enumerate() {
        let body_start = *heading_end;
        let body_end = if i + 1 < headings.len() {
            headings[i + 1].2
        } else {
            content.len()
        };

        let body = content[body_start..body_end].trim();

        let role = ROLE_RE
            .captures(body)
            .map(|cap| cap[1].trim().to_string());

        let depends_on_task_numbers = if let Some(cap) = DEPENDS_RE.captures(body) {
            let deps_str = cap[1].trim();
            deps_str
                .split(',')
                .filter_map(|s| {
                    let s = s.trim().trim_start_matches("Task ").trim();
                    s.parse::<usize>().ok()
                })
                .collect()
        } else if *task_number > 1 {
            vec![task_number - 1]
        } else {
            vec![]
        };

        // Strip metadata lines from description
        let description: String = body
            .lines()
            .filter(|line| {
                !ROLE_RE.is_match(line) && !DEPENDS_RE.is_match(line)
            })
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        tasks.push(ParsedTask {
            task_number: *task_number,
            title: title.clone(),
            description,
            role,
            depends_on_task_numbers,
        });
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_plan() {
        let plan = "\
### Task 1: Set up project structure

Create the initial project layout with src/ and tests/.

### Task 2: Implement core logic

Build the main processing pipeline.

### Task 3: Add logging

**Role:** DevOps

Integrate structured logging throughout the app.
";

        let tasks = parse_plan(plan);
        assert_eq!(tasks.len(), 3);

        assert_eq!(tasks[0].task_number, 1);
        assert_eq!(tasks[0].title, "Set up project structure");
        assert!(tasks[0].depends_on_task_numbers.is_empty());
        assert!(tasks[0].role.is_none());

        assert_eq!(tasks[1].task_number, 2);
        assert_eq!(tasks[1].title, "Implement core logic");
        assert_eq!(tasks[1].depends_on_task_numbers, vec![1]);
        assert!(tasks[1].role.is_none());

        assert_eq!(tasks[2].task_number, 3);
        assert_eq!(tasks[2].title, "Add logging");
        assert_eq!(tasks[2].depends_on_task_numbers, vec![2]);
        assert_eq!(tasks[2].role.as_deref(), Some("DevOps"));
        // Description should not contain the **Role:** line
        assert!(!tasks[2].description.contains("**Role:**"));
        assert!(tasks[2].description.contains("Integrate structured logging"));
    }

    #[test]
    fn test_parse_empty_plan() {
        let plan = "# My Plan\n\nSome intro text without any task headings.\n";
        let tasks = parse_plan(plan);
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_multi_depends() {
        let plan = "\
### Task 1: Foundation

Set up the base.

### Task 2: Module A

Build module A.

### Task 3: Integration

**Depends on:** 1, 2

Integrate modules together.
";

        let tasks = parse_plan(plan);
        assert_eq!(tasks.len(), 3);

        assert_eq!(tasks[2].task_number, 3);
        assert_eq!(tasks[2].title, "Integration");
        assert_eq!(tasks[2].depends_on_task_numbers, vec![1, 2]);
        // Description should not contain the **Depends on:** line
        assert!(!tasks[2].description.contains("**Depends on:**"));
        assert!(tasks[2].description.contains("Integrate modules"));
    }
}
