use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePattern {
    pub id: String,
    pub project_id: String,
    pub pattern_type: String,
    pub role: Option<String>,
    pub task_type: Option<String>,
    pub keywords: String,
    pub title: String,
    pub content: String,
    pub confidence: f64,
    pub observations: i64,
    pub source_type: String,
    pub source_id: Option<String>,
    pub files_involved: Option<String>,
    pub is_shared: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateKnowledgePattern {
    pub project_id: String,
    pub pattern_type: String,
    pub role: Option<String>,
    pub task_type: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub files_involved: Option<Vec<String>>,
    pub is_shared: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateKnowledgePattern {
    pub title: Option<String>,
    pub content: Option<String>,
    pub role: Option<String>,
    pub task_type: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub confidence: Option<f64>,
    pub is_shared: Option<bool>,
    pub files_involved: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct KnowledgeSearchQuery {
    pub query: String,
    pub role: Option<String>,
    pub task_type: Option<String>,
    pub pattern_type: Option<String>,
    pub files: Option<Vec<String>>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeSearchResult {
    #[serde(flatten)]
    pub pattern: KnowledgePattern,
    pub score: f64,
    pub source_project: String,
}

/// Extract keywords from text: split on whitespace/punctuation, lowercase, remove stopwords, deduplicate
pub fn extract_keywords(text: &str) -> Vec<String> {
    let stopwords = ["the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
        "may", "might", "shall", "can", "need", "must", "to", "of", "in", "for", "on",
        "with", "at", "by", "from", "as", "into", "through", "during", "before", "after",
        "above", "below", "between", "under", "again", "further", "then", "once",
        "here", "there", "when", "where", "why", "how", "all", "each", "every",
        "both", "few", "more", "most", "other", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very", "just", "because",
        "but", "and", "or", "if", "while", "this", "that", "these", "those", "it"];

    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !stopwords.contains(w))
        .map(|w| w.to_string())
        .collect();

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    words.into_iter().filter(|w| seen.insert(w.clone())).collect()
}

/// Compute keyword overlap score: matched / total query keywords
pub fn keyword_overlap_score(query_keywords: &[String], pattern_keywords: &[String]) -> f64 {
    if query_keywords.is_empty() { return 0.0; }
    let pattern_set: std::collections::HashSet<&str> = pattern_keywords.iter().map(|s| s.as_str()).collect();
    let matched = query_keywords.iter().filter(|k| pattern_set.contains(k.as_str())).count();
    matched as f64 / query_keywords.len() as f64
}

impl KnowledgePattern {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            pattern_type: row.get("pattern_type")?,
            role: row.get("role")?,
            task_type: row.get("task_type")?,
            keywords: row.get("keywords")?,
            title: row.get("title")?,
            content: row.get("content")?,
            confidence: row.get("confidence")?,
            observations: row.get("observations")?,
            source_type: row.get("source_type")?,
            source_id: row.get("source_id")?,
            files_involved: row.get("files_involved")?,
            is_shared: row.get::<_, i64>("is_shared").unwrap_or(0) != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateKnowledgePattern) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let keywords_json = serde_json::to_string(
            &input.keywords.as_deref().unwrap_or(&[])
        ).unwrap_or_else(|_| "[]".to_string());
        let files_json = input.files_involved.as_ref().map(|f| {
            serde_json::to_string(f).unwrap_or_else(|_| "[]".to_string())
        });
        let is_shared: i64 = if input.is_shared.unwrap_or(false) { 1 } else { 0 };

        conn.execute(
            "INSERT INTO knowledge_patterns (id, project_id, pattern_type, role, task_type, keywords, title, content, source_type, source_id, files_involved, is_shared)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                id,
                input.project_id,
                input.pattern_type,
                input.role,
                input.task_type,
                keywords_json,
                input.title,
                input.content,
                input.source_type,
                input.source_id,
                files_json,
                is_shared,
            ],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM knowledge_patterns WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("knowledge_pattern {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(
        conn: &Connection,
        project_id: &str,
        pattern_type: Option<&str>,
        role: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Self>> {
        let mut sql = "SELECT * FROM knowledge_patterns WHERE project_id = ?1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id.to_string())];
        let mut idx = 2;

        if let Some(pt) = pattern_type {
            sql.push_str(&format!(" AND pattern_type = ?{}", idx));
            param_values.push(Box::new(pt.to_string()));
            idx += 1;
        }
        if let Some(r) = role {
            sql.push_str(&format!(" AND role = ?{}", idx));
            param_values.push(Box::new(r.to_string()));
            idx += 1;
        }
        sql.push_str(&format!(" ORDER BY confidence DESC LIMIT ?{}", idx));
        param_values.push(Box::new(limit));

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateKnowledgePattern) -> Result<Self> {
        // Build dynamic UPDATE
        let mut sets = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref title) = input.title {
            sets.push(format!("title = ?{}", idx));
            param_values.push(Box::new(title.clone()));
            idx += 1;
        }
        if let Some(ref content) = input.content {
            sets.push(format!("content = ?{}", idx));
            param_values.push(Box::new(content.clone()));
            idx += 1;
        }
        if let Some(ref role) = input.role {
            sets.push(format!("role = ?{}", idx));
            param_values.push(Box::new(role.clone()));
            idx += 1;
        }
        if let Some(ref task_type) = input.task_type {
            sets.push(format!("task_type = ?{}", idx));
            param_values.push(Box::new(task_type.clone()));
            idx += 1;
        }
        if let Some(ref keywords) = input.keywords {
            let kw_json = serde_json::to_string(keywords).unwrap_or_else(|_| "[]".to_string());
            sets.push(format!("keywords = ?{}", idx));
            param_values.push(Box::new(kw_json));
            idx += 1;
        }
        if let Some(confidence) = input.confidence {
            sets.push(format!("confidence = ?{}", idx));
            param_values.push(Box::new(confidence));
            idx += 1;
        }
        if let Some(is_shared) = input.is_shared {
            let val: i64 = if is_shared { 1 } else { 0 };
            sets.push(format!("is_shared = ?{}", idx));
            param_values.push(Box::new(val));
            idx += 1;
        }
        if let Some(ref files) = input.files_involved {
            let files_json = serde_json::to_string(files).unwrap_or_else(|_| "[]".to_string());
            sets.push(format!("files_involved = ?{}", idx));
            param_values.push(Box::new(files_json));
            idx += 1;
        }

        if sets.is_empty() {
            return Self::get_by_id(conn, id);
        }

        sets.push(format!("updated_at = datetime('now')"));

        let sql = format!(
            "UPDATE knowledge_patterns SET {} WHERE id = ?{}",
            sets.join(", "),
            idx
        );
        param_values.push(Box::new(id.to_string()));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let changes = conn.execute(&sql, params_refs.as_slice())?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("knowledge_pattern {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM knowledge_patterns WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("knowledge_pattern {}", id)));
        }
        Ok(())
    }

    pub fn search(conn: &Connection, project_id: &str, query: &KnowledgeSearchQuery) -> Result<Vec<KnowledgeSearchResult>> {
        let mut sql = "SELECT * FROM knowledge_patterns WHERE project_id = ?1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id.to_string())];
        let mut idx = 2;

        if let Some(ref role) = query.role {
            sql.push_str(&format!(" AND role = ?{}", idx));
            param_values.push(Box::new(role.clone()));
            idx += 1;
        }
        if let Some(ref task_type) = query.task_type {
            sql.push_str(&format!(" AND task_type = ?{}", idx));
            param_values.push(Box::new(task_type.clone()));
            idx += 1;
        }
        if let Some(ref pattern_type) = query.pattern_type {
            sql.push_str(&format!(" AND pattern_type = ?{}", idx));
            param_values.push(Box::new(pattern_type.clone()));
            let _ = idx; // suppress unused warning
        }

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;

        let query_keywords = extract_keywords(&query.query);
        let query_files: Vec<String> = query.files.clone().unwrap_or_default();

        let mut results: Vec<KnowledgeSearchResult> = Vec::new();
        for row in rows {
            let pattern = row?;
            let pattern_keywords: Vec<String> = serde_json::from_str(&pattern.keywords).unwrap_or_default();
            let mut score = keyword_overlap_score(&query_keywords, &pattern_keywords);

            // File boost
            if !query_files.is_empty() {
                if let Some(ref fi) = pattern.files_involved {
                    let pattern_files: Vec<String> = serde_json::from_str(fi).unwrap_or_default();
                    let has_overlap = query_files.iter().any(|qf| pattern_files.contains(qf));
                    if has_overlap {
                        score += 0.2;
                    }
                }
            }

            // Weight by confidence
            let final_score = score * pattern.confidence;

            let source_project = pattern.project_id.clone();
            results.push(KnowledgeSearchResult {
                pattern,
                score: final_score,
                source_project,
            });
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        let limit = query.limit.unwrap_or(10) as usize;
        results.truncate(limit);
        Ok(results)
    }

    pub fn search_cross_project(conn: &Connection, query: &KnowledgeSearchQuery) -> Result<Vec<KnowledgeSearchResult>> {
        let mut sql = "SELECT * FROM knowledge_patterns WHERE is_shared = 1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref role) = query.role {
            sql.push_str(&format!(" AND role = ?{}", idx));
            param_values.push(Box::new(role.clone()));
            idx += 1;
        }
        if let Some(ref task_type) = query.task_type {
            sql.push_str(&format!(" AND task_type = ?{}", idx));
            param_values.push(Box::new(task_type.clone()));
            idx += 1;
        }
        if let Some(ref pattern_type) = query.pattern_type {
            sql.push_str(&format!(" AND pattern_type = ?{}", idx));
            param_values.push(Box::new(pattern_type.clone()));
            let _ = idx;
        }

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), Self::from_row)?;

        let query_keywords = extract_keywords(&query.query);
        let query_files: Vec<String> = query.files.clone().unwrap_or_default();

        let mut results: Vec<KnowledgeSearchResult> = Vec::new();
        for row in rows {
            let pattern = row?;
            let pattern_keywords: Vec<String> = serde_json::from_str(&pattern.keywords).unwrap_or_default();
            let mut score = keyword_overlap_score(&query_keywords, &pattern_keywords);

            if !query_files.is_empty() {
                if let Some(ref fi) = pattern.files_involved {
                    let pattern_files: Vec<String> = serde_json::from_str(fi).unwrap_or_default();
                    let has_overlap = query_files.iter().any(|qf| pattern_files.contains(qf));
                    if has_overlap {
                        score += 0.2;
                    }
                }
            }

            let final_score = score * pattern.confidence;
            let source_project = pattern.project_id.clone();
            results.push(KnowledgeSearchResult {
                pattern,
                score: final_score,
                source_project,
            });
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        let limit = query.limit.unwrap_or(10) as usize;
        results.truncate(limit);
        Ok(results)
    }

    pub fn merge_or_increment(
        conn: &Connection,
        project_id: &str,
        pattern_type: &str,
        role: Option<&str>,
        task_type: Option<&str>,
        title: &str,
        content: &str,
        source_type: &str,
        source_id: Option<&str>,
        files: Option<&[String]>,
        is_shared: bool,
    ) -> Result<Self> {
        // Check for existing pattern with same key fields
        let mut sql = "SELECT * FROM knowledge_patterns WHERE project_id = ?1 AND pattern_type = ?2 AND title = ?3".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(project_id.to_string()),
            Box::new(pattern_type.to_string()),
            Box::new(title.to_string()),
        ];
        let mut idx = 4;

        match role {
            Some(r) => {
                sql.push_str(&format!(" AND role = ?{}", idx));
                param_values.push(Box::new(r.to_string()));
                idx += 1;
            }
            None => {
                sql.push_str(" AND role IS NULL");
            }
        }
        match task_type {
            Some(tt) => {
                sql.push_str(&format!(" AND task_type = ?{}", idx));
                param_values.push(Box::new(tt.to_string()));
            }
            None => {
                sql.push_str(" AND task_type IS NULL");
            }
        }
        sql.push_str(" LIMIT 1");

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let existing = stmt.query_map(params_refs.as_slice(), Self::from_row)?
            .filter_map(|r| r.ok())
            .next();

        match existing {
            Some(pattern) => {
                let new_obs = pattern.observations + 1;
                // Recalculate confidence: approaches 1.0 asymptotically
                let new_confidence = 1.0 - (1.0 / (new_obs as f64 + 1.0));
                conn.execute(
                    "UPDATE knowledge_patterns SET observations = ?1, confidence = ?2, updated_at = datetime('now') WHERE id = ?3",
                    params![new_obs, new_confidence, pattern.id],
                )?;
                Self::get_by_id(conn, &pattern.id)
            }
            None => {
                let keywords = extract_keywords(&format!("{} {}", title, content));
                Self::create(conn, &CreateKnowledgePattern {
                    project_id: project_id.to_string(),
                    pattern_type: pattern_type.to_string(),
                    role: role.map(|s| s.to_string()),
                    task_type: task_type.map(|s| s.to_string()),
                    keywords: Some(keywords),
                    title: title.to_string(),
                    content: content.to_string(),
                    source_type: source_type.to_string(),
                    source_id: source_id.map(|s| s.to_string()),
                    files_involved: files.map(|f| f.to_vec()),
                    is_shared: Some(is_shared),
                })
            }
        }
    }

    pub fn decay_confidence(conn: &Connection, id: &str) -> Result<Self> {
        conn.execute(
            "UPDATE knowledge_patterns SET confidence = confidence * 0.8, updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Self::get_by_id(conn, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'TestProject', '/tmp', 'work')", []).unwrap();
        conn.execute("INSERT INTO projects (id, name, directory, context) VALUES ('p2', 'OtherProject', '/tmp2', 'work')", []).unwrap();
        conn
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup();
        let input = CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: Some("Senior Coder".to_string()),
            task_type: Some("feature".to_string()),
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "Auth middleware pattern".to_string(),
            content: "Use bearer token validation middleware".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: Some(vec!["src/auth/mod.rs".to_string()]),
            is_shared: Some(true),
        };
        let pattern = KnowledgePattern::create(&conn, &input).unwrap();
        assert_eq!(pattern.project_id, "p1");
        assert_eq!(pattern.pattern_type, "solution");
        assert_eq!(pattern.role, Some("Senior Coder".to_string()));
        assert_eq!(pattern.task_type, Some("feature".to_string()));
        assert_eq!(pattern.title, "Auth middleware pattern");
        assert_eq!(pattern.content, "Use bearer token validation middleware");
        assert_eq!(pattern.source_type, "manual");
        assert!(pattern.is_shared);
        assert_eq!(pattern.observations, 1);
        assert_eq!(pattern.confidence, 0.5);

        let keywords: Vec<String> = serde_json::from_str(&pattern.keywords).unwrap();
        assert_eq!(keywords, vec!["auth", "middleware"]);

        let files: Vec<String> = serde_json::from_str(pattern.files_involved.as_ref().unwrap()).unwrap();
        assert_eq!(files, vec!["src/auth/mod.rs"]);

        let fetched = KnowledgePattern::get_by_id(&conn, &pattern.id).unwrap();
        assert_eq!(fetched.id, pattern.id);
        assert_eq!(fetched.title, pattern.title);
    }

    #[test]
    fn test_list_by_project() {
        let conn = setup();
        // Create 3 patterns with different types/roles
        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: Some("Senior Coder".to_string()),
            task_type: None,
            keywords: Some(vec!["auth".to_string()]),
            title: "Pattern 1".to_string(),
            content: "Content 1".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();

        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "gotcha".to_string(),
            role: Some("Tester".to_string()),
            task_type: None,
            keywords: Some(vec!["db".to_string()]),
            title: "Pattern 2".to_string(),
            content: "Content 2".to_string(),
            source_type: "trace".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();

        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: Some("Tester".to_string()),
            task_type: None,
            keywords: Some(vec!["api".to_string()]),
            title: "Pattern 3".to_string(),
            content: "Content 3".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();

        // All patterns
        let all = KnowledgePattern::list_by_project(&conn, "p1", None, None, 10).unwrap();
        assert_eq!(all.len(), 3);

        // Filter by type
        let solutions = KnowledgePattern::list_by_project(&conn, "p1", Some("solution"), None, 10).unwrap();
        assert_eq!(solutions.len(), 2);

        // Filter by role
        let tester = KnowledgePattern::list_by_project(&conn, "p1", None, Some("Tester"), 10).unwrap();
        assert_eq!(tester.len(), 2);

        // Filter by both
        let solution_tester = KnowledgePattern::list_by_project(&conn, "p1", Some("solution"), Some("Tester"), 10).unwrap();
        assert_eq!(solution_tester.len(), 1);
        assert_eq!(solution_tester[0].title, "Pattern 3");
    }

    #[test]
    fn test_search_keyword_scoring() {
        let conn = setup();
        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string(), "token".to_string()]),
            title: "Auth pattern".to_string(),
            content: "Auth middleware with tokens".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();

        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["database".to_string(), "migration".to_string()]),
            title: "DB pattern".to_string(),
            content: "Database migration tips".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();

        let results = KnowledgePattern::search(&conn, "p1", &KnowledgeSearchQuery {
            query: "auth middleware".to_string(),
            role: None,
            task_type: None,
            pattern_type: None,
            files: None,
            limit: None,
        }).unwrap();

        assert!(results.len() >= 1);
        assert_eq!(results[0].pattern.title, "Auth pattern");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_file_boost() {
        let conn = setup();
        // Pattern with matching files
        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "Auth with files".to_string(),
            content: "Auth pattern".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: Some(vec!["src/auth/mod.rs".to_string()]),
            is_shared: None,
        }).unwrap();

        // Pattern without matching files
        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "Auth without files".to_string(),
            content: "Auth pattern no files".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();

        let results = KnowledgePattern::search(&conn, "p1", &KnowledgeSearchQuery {
            query: "auth middleware".to_string(),
            role: None,
            task_type: None,
            pattern_type: None,
            files: Some(vec!["src/auth/mod.rs".to_string()]),
            limit: None,
        }).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].pattern.title, "Auth with files");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_search_confidence_weight() {
        let conn = setup();
        // High confidence pattern
        let p1 = KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "High confidence".to_string(),
            content: "High conf pattern".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();
        conn.execute(
            "UPDATE knowledge_patterns SET confidence = 0.9 WHERE id = ?1",
            params![p1.id],
        ).unwrap();

        // Low confidence pattern
        let p2 = KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "Low confidence".to_string(),
            content: "Low conf pattern".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();
        conn.execute(
            "UPDATE knowledge_patterns SET confidence = 0.2 WHERE id = ?1",
            params![p2.id],
        ).unwrap();

        let results = KnowledgePattern::search(&conn, "p1", &KnowledgeSearchQuery {
            query: "auth middleware".to_string(),
            role: None,
            task_type: None,
            pattern_type: None,
            files: None,
            limit: None,
        }).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].pattern.title, "High confidence");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_merge_or_increment() {
        let conn = setup();
        let p1 = KnowledgePattern::merge_or_increment(
            &conn, "p1", "solution", Some("Coder"), Some("feature"),
            "Auth pattern", "Use bearer tokens", "manual", None, None, false,
        ).unwrap();
        assert_eq!(p1.observations, 1);
        let orig_confidence = p1.confidence;

        // Call again with same key fields
        let p2 = KnowledgePattern::merge_or_increment(
            &conn, "p1", "solution", Some("Coder"), Some("feature"),
            "Auth pattern", "Use bearer tokens", "manual", None, None, false,
        ).unwrap();
        assert_eq!(p2.id, p1.id);
        assert_eq!(p2.observations, 2);
        assert!(p2.confidence > orig_confidence);
    }

    #[test]
    fn test_decay_confidence() {
        let conn = setup();
        let p = KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["test".to_string()]),
            title: "Decay test".to_string(),
            content: "Content".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: None,
        }).unwrap();
        // Set confidence to 0.8
        conn.execute(
            "UPDATE knowledge_patterns SET confidence = 0.8 WHERE id = ?1",
            params![p.id],
        ).unwrap();

        let decayed = KnowledgePattern::decay_confidence(&conn, &p.id).unwrap();
        assert!((decayed.confidence - 0.64).abs() < 0.001);
    }

    #[test]
    fn test_cross_project_search() {
        let conn = setup();
        // Shared pattern in p1
        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p1".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "Shared auth pattern".to_string(),
            content: "Shared auth content".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: Some(true),
        }).unwrap();

        // Non-shared pattern in p2
        KnowledgePattern::create(&conn, &CreateKnowledgePattern {
            project_id: "p2".to_string(),
            pattern_type: "solution".to_string(),
            role: None,
            task_type: None,
            keywords: Some(vec!["auth".to_string(), "middleware".to_string()]),
            title: "Private auth pattern".to_string(),
            content: "Private auth content".to_string(),
            source_type: "manual".to_string(),
            source_id: None,
            files_involved: None,
            is_shared: Some(false),
        }).unwrap();

        let results = KnowledgePattern::search_cross_project(&conn, &KnowledgeSearchQuery {
            query: "auth middleware".to_string(),
            role: None,
            task_type: None,
            pattern_type: None,
            files: None,
            limit: None,
        }).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].pattern.title, "Shared auth pattern");
        assert_eq!(results[0].source_project, "p1");
    }

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_keywords("The quick brown fox jumps over the lazy dog");
        assert_eq!(keywords, vec!["quick", "brown", "fox", "jumps", "over", "lazy", "dog"]);
    }

    #[test]
    fn test_keyword_overlap_score() {
        let query = vec!["auth".to_string(), "middleware".to_string(), "token".to_string()];
        let pattern = vec!["auth".to_string(), "middleware".to_string(), "database".to_string()];
        let score = keyword_overlap_score(&query, &pattern);
        // 2 out of 3 matched
        assert!((score - 2.0 / 3.0).abs() < 0.001);
    }
}
