use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub issue_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    #[serde(skip_serializing)]
    pub stored_path: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAttachment {
    pub issue_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub stored_path: String,
}

impl Attachment {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            issue_id: row.get("issue_id")?,
            filename: row.get("filename")?,
            mime_type: row.get("mime_type")?,
            size_bytes: row.get("size_bytes")?,
            stored_path: row.get("stored_path")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateAttachment) -> crate::error::Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO attachments (id, issue_id, filename, mime_type, size_bytes, stored_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, input.issue_id, input.filename, input.mime_type, input.size_bytes, input.stored_path],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> crate::error::Result<Self> {
        conn.query_row(
            "SELECT * FROM attachments WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|_| crate::error::IronweaveError::NotFound(format!("attachment: {}", id)))
    }

    pub fn list_by_issue(conn: &Connection, issue_id: &str) -> crate::error::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM attachments WHERE issue_id = ?1 ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map(params![issue_id], Self::from_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn delete(conn: &Connection, id: &str) -> crate::error::Result<()> {
        let changes = conn.execute("DELETE FROM attachments WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(crate::error::IronweaveError::NotFound(format!("attachment: {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO issues (id, project_id, type, title, description, status, priority, depends_on, needs_intake, scope_mode)
             VALUES ('i1', 'p1', 'task', 'test issue', 'desc', 'open', 3, '[]', 1, 'auto')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let input = CreateAttachment {
            issue_id: "i1".to_string(),
            filename: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 1024,
            stored_path: "/data/attachments/i1/abc_test.txt".to_string(),
        };
        let att = Attachment::create(&conn, &input).unwrap();
        assert_eq!(att.filename, "test.txt");
        assert_eq!(att.size_bytes, 1024);
        let fetched = Attachment::get_by_id(&conn, &att.id).unwrap();
        assert_eq!(fetched.id, att.id);
    }

    #[test]
    fn test_list_by_issue() {
        let conn = setup_db();
        for i in 0..3 {
            Attachment::create(&conn, &CreateAttachment {
                issue_id: "i1".to_string(),
                filename: format!("file{}.txt", i),
                mime_type: "text/plain".to_string(),
                size_bytes: 100,
                stored_path: format!("/data/attachments/i1/{}_file{}.txt", i, i),
            }).unwrap();
        }
        let list = Attachment::list_by_issue(&conn, "i1").unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let att = Attachment::create(&conn, &CreateAttachment {
            issue_id: "i1".to_string(),
            filename: "delete_me.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 50,
            stored_path: "/data/attachments/i1/del.txt".to_string(),
        }).unwrap();
        Attachment::delete(&conn, &att.id).unwrap();
        assert!(Attachment::get_by_id(&conn, &att.id).is_err());
    }
}
