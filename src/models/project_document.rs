use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub id: String,
    pub project_id: String,
    pub doc_type: String,
    pub content: String,
    pub version: i64,
    pub previous_content: Option<String>,
    pub updated_at: String,
    pub updated_by: String,
}

#[derive(Debug, Deserialize)]
pub struct DocumentUpdate {
    pub content: String,
    pub updated_by: Option<String>,
}

pub fn detect_removals(old_content: &str, new_content: &str) -> Vec<String> {
    let old_lines: std::collections::HashSet<&str> = old_content.lines().filter(|l| !l.trim().is_empty()).collect();
    let new_lines: std::collections::HashSet<&str> = new_content.lines().filter(|l| !l.trim().is_empty()).collect();
    old_lines.difference(&new_lines).map(|s| s.to_string()).collect()
}

impl ProjectDocument {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            doc_type: row.get("doc_type")?,
            content: row.get("content")?,
            version: row.get("version")?,
            previous_content: row.get("previous_content")?,
            updated_at: row.get("updated_at")?,
            updated_by: row.get("updated_by")?,
        })
    }

    pub fn get_or_create(conn: &Connection, project_id: &str, doc_type: &str) -> Result<Self> {
        let existing = conn.query_row(
            "SELECT * FROM project_documents WHERE project_id = ?1 AND doc_type = ?2",
            params![project_id, doc_type],
            Self::from_row,
        );

        match existing {
            Ok(doc) => Ok(doc),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO project_documents (id, project_id, doc_type, content)
                     VALUES (?1, ?2, ?3, '')",
                    params![id, project_id, doc_type],
                )?;
                Self::get_by_id(conn, &id)
            }
            Err(e) => Err(IronweaveError::Database(e)),
        }
    }

    fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM project_documents WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("project_document {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn update_content(
        conn: &Connection,
        project_id: &str,
        doc_type: &str,
        content: &str,
        updated_by: &str,
    ) -> Result<Self> {
        // Ensure document exists
        let doc = Self::get_or_create(conn, project_id, doc_type)?;

        conn.execute(
            "UPDATE project_documents SET previous_content = content, content = ?1, version = version + 1, updated_at = datetime('now'), updated_by = ?2 WHERE id = ?3",
            params![content, updated_by, doc.id],
        )?;
        Self::get_by_id(conn, &doc.id)
    }

    pub fn get_history(conn: &Connection, project_id: &str, doc_type: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM project_documents WHERE project_id = ?1 AND doc_type = ?2",
            params![project_id, doc_type],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("project_document {}:{}", project_id, doc_type)),
            other => IronweaveError::Database(other),
        })
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
        conn
    }

    #[test]
    fn test_get_or_create_idempotent() {
        let conn = setup();
        let doc1 = ProjectDocument::get_or_create(&conn, "p1", "intent").unwrap();
        let doc2 = ProjectDocument::get_or_create(&conn, "p1", "intent").unwrap();
        assert_eq!(doc1.id, doc2.id);
        assert_eq!(doc1.content, "");
        assert_eq!(doc1.version, 1);
    }

    #[test]
    fn test_update_preserves_previous() {
        let conn = setup();
        let doc = ProjectDocument::get_or_create(&conn, "p1", "intent").unwrap();
        assert_eq!(doc.content, "");
        assert!(doc.previous_content.is_none());

        let updated = ProjectDocument::update_content(&conn, "p1", "intent", "New content", "user").unwrap();
        assert_eq!(updated.content, "New content");
        assert_eq!(updated.previous_content, Some("".to_string()));
        assert_eq!(updated.version, 2);
    }

    #[test]
    fn test_version_increments() {
        let conn = setup();
        ProjectDocument::get_or_create(&conn, "p1", "reality").unwrap();

        let v2 = ProjectDocument::update_content(&conn, "p1", "reality", "Version 2", "user").unwrap();
        assert_eq!(v2.version, 2);

        let v3 = ProjectDocument::update_content(&conn, "p1", "reality", "Version 3", "agent").unwrap();
        assert_eq!(v3.version, 3);
        assert_eq!(v3.content, "Version 3");
        assert_eq!(v3.previous_content, Some("Version 2".to_string()));
        assert_eq!(v3.updated_by, "agent");
    }

    #[test]
    fn test_detect_removals() {
        let old = "line one\nline two\nline three\n";
        let new = "line one\nline three\n";
        let removals = detect_removals(old, new);
        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0], "line two");
    }
}
