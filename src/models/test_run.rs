use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRun {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub test_type: String,
    pub target_url: Option<String>,
    pub total_tests: i64,
    pub passed: i64,
    pub failed: i64,
    pub skipped: i64,
    pub duration_seconds: Option<f64>,
    pub output: Option<String>,
    pub failed_tests: String,
    pub triggered_by: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTestRun {
    pub project_id: String,
    pub test_type: String,
    pub target_url: Option<String>,
    pub triggered_by: String,
}

impl TestRun {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            status: row.get("status")?,
            test_type: row.get("test_type")?,
            target_url: row.get("target_url")?,
            total_tests: row.get("total_tests")?,
            passed: row.get("passed")?,
            failed: row.get("failed")?,
            skipped: row.get("skipped")?,
            duration_seconds: row.get("duration_seconds")?,
            output: row.get("output")?,
            failed_tests: row.get("failed_tests")?,
            triggered_by: row.get("triggered_by")?,
            created_at: row.get("created_at")?,
            completed_at: row.get("completed_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateTestRun) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO test_runs (id, project_id, test_type, target_url, triggered_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, input.project_id, input.test_type, input.target_url, input.triggered_by],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM test_runs WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("test_run {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str, limit: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM test_runs WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![project_id, limit], Self::from_row)?;
        let mut runs = Vec::new();
        for row in rows { runs.push(row?); }
        Ok(runs)
    }

    pub fn latest_by_project(conn: &Connection, project_id: &str) -> Result<Option<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM test_runs WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 1"
        )?;
        let result = stmt.query_row(params![project_id], Self::from_row).ok();
        Ok(result)
    }

    pub fn update_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
        let changes = conn.execute(
            "UPDATE test_runs SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("test_run {}", id)));
        }
        Ok(())
    }

    pub fn complete(
        conn: &Connection,
        id: &str,
        status: &str,
        total: i64,
        passed: i64,
        failed: i64,
        skipped: i64,
        duration: f64,
        output: Option<&str>,
        failed_tests: Option<&str>,
    ) -> Result<Self> {
        let completed_at = chrono::Utc::now().to_rfc3339();
        let ft = failed_tests.unwrap_or("[]");
        conn.execute(
            "UPDATE test_runs SET status = ?1, total_tests = ?2, passed = ?3, failed = ?4,
             skipped = ?5, duration_seconds = ?6, output = ?7, failed_tests = ?8,
             completed_at = ?9 WHERE id = ?10",
            params![status, total, passed, failed, skipped, duration, output, ft, completed_at, id],
        )?;
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM test_runs WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("test_run {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, String) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();

        let project = crate::models::project::Project::create(&conn, &crate::models::project::CreateProject {
            name: "TestProject".to_string(),
            directory: "/tmp/test-project".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap();

        (conn, project.id)
    }

    #[test]
    fn test_create_and_get() {
        let (conn, project_id) = setup();
        let input = CreateTestRun {
            project_id: project_id.clone(),
            test_type: "e2e".to_string(),
            target_url: Some("http://localhost:3000".to_string()),
            triggered_by: "manual".to_string(),
        };
        let run = TestRun::create(&conn, &input).unwrap();
        assert_eq!(run.project_id, project_id);
        assert_eq!(run.status, "pending");
        assert_eq!(run.test_type, "e2e");
        assert_eq!(run.target_url, Some("http://localhost:3000".to_string()));
        assert_eq!(run.triggered_by, "manual");
        assert_eq!(run.total_tests, 0);
        assert_eq!(run.failed_tests, "[]");

        let fetched = TestRun::get_by_id(&conn, &run.id).unwrap();
        assert_eq!(fetched.id, run.id);
    }

    #[test]
    fn test_list_and_latest() {
        let (conn, project_id) = setup();
        let input1 = CreateTestRun {
            project_id: project_id.clone(),
            test_type: "unit".to_string(),
            target_url: None,
            triggered_by: "manual".to_string(),
        };
        let run1 = TestRun::create(&conn, &input1).unwrap();

        // Ensure second run has a later created_at
        conn.execute(
            "UPDATE test_runs SET created_at = datetime('now', '+1 second') WHERE id = ?1",
            params![run1.id],
        ).ok();

        let input2 = CreateTestRun {
            project_id: project_id.clone(),
            test_type: "e2e".to_string(),
            target_url: Some("http://localhost:5000".to_string()),
            triggered_by: "orchestrator".to_string(),
        };
        let run2 = TestRun::create(&conn, &input2).unwrap();

        // Bump run2's created_at so it's definitively later
        conn.execute(
            "UPDATE test_runs SET created_at = datetime('now', '+1 second') WHERE id = ?1",
            params![run2.id],
        ).unwrap();

        let list = TestRun::list_by_project(&conn, &project_id, 10).unwrap();
        assert_eq!(list.len(), 2);
        // Ordered DESC, so run2 first
        assert_eq!(list[0].id, run2.id);
        assert_eq!(list[1].id, run1.id);

        let latest = TestRun::latest_by_project(&conn, &project_id).unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().id, run2.id);
    }

    #[test]
    fn test_complete() {
        let (conn, project_id) = setup();
        let input = CreateTestRun {
            project_id,
            test_type: "full".to_string(),
            target_url: None,
            triggered_by: "merge-queue".to_string(),
        };
        let run = TestRun::create(&conn, &input).unwrap();
        assert!(run.completed_at.is_none());

        let failed_json = r#"["test_login", "test_signup"]"#;
        let completed = TestRun::complete(
            &conn, &run.id, "failed", 10, 7, 2, 1, 42.5,
            Some("Test output here"), Some(failed_json),
        ).unwrap();

        assert_eq!(completed.status, "failed");
        assert_eq!(completed.total_tests, 10);
        assert_eq!(completed.passed, 7);
        assert_eq!(completed.failed, 2);
        assert_eq!(completed.skipped, 1);
        assert_eq!(completed.duration_seconds, Some(42.5));
        assert_eq!(completed.output, Some("Test output here".to_string()));
        assert_eq!(completed.failed_tests, failed_json);
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_delete() {
        let (conn, project_id) = setup();
        let input = CreateTestRun {
            project_id,
            test_type: "e2e".to_string(),
            target_url: None,
            triggered_by: "manual".to_string(),
        };
        let run = TestRun::create(&conn, &input).unwrap();
        TestRun::delete(&conn, &run.id).unwrap();
        assert!(TestRun::get_by_id(&conn, &run.id).is_err());
    }
}
