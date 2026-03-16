use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTask {
    pub id: String,
    pub feature_id: String,
    pub title: String,
    pub status: String,
    pub issue_id: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFeatureTask {
    pub feature_id: String,
    pub title: String,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureTask {
    pub title: Option<String>,
    pub status: Option<String>,
    pub sort_order: Option<i64>,
}

impl FeatureTask {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            feature_id: row.get("feature_id")?,
            title: row.get("title")?,
            status: row.get("status")?,
            issue_id: row.get("issue_id")?,
            sort_order: row.get("sort_order")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateFeatureTask) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let sort_order = input.sort_order.unwrap_or(0);
        conn.execute(
            "INSERT INTO feature_tasks (id, feature_id, title, sort_order)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, input.feature_id, input.title, sort_order],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM feature_tasks WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("feature_task {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_feature(conn: &Connection, feature_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM feature_tasks WHERE feature_id = ?1 ORDER BY sort_order ASC"
        )?;
        let rows = stmt.query_map(params![feature_id], Self::from_row)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateFeatureTask) -> Result<Self> {
        let mut sets = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref title) = input.title {
            sets.push(format!("title = ?{}", idx));
            param_values.push(Box::new(title.clone()));
            idx += 1;
        }
        if let Some(ref status) = input.status {
            sets.push(format!("status = ?{}", idx));
            param_values.push(Box::new(status.clone()));
            idx += 1;
        }
        if let Some(sort_order) = input.sort_order {
            sets.push(format!("sort_order = ?{}", idx));
            param_values.push(Box::new(sort_order));
            idx += 1;
        }

        if sets.is_empty() {
            return Self::get_by_id(conn, id);
        }

        let sql = format!(
            "UPDATE feature_tasks SET {} WHERE id = ?{}",
            sets.join(", "),
            idx
        );
        param_values.push(Box::new(id.to_string()));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let changes = conn.execute(&sql, params_refs.as_slice())?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature_task {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM feature_tasks WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature_task {}", id)));
        }
        Ok(())
    }

    pub fn implement(conn: &Connection, task_id: &str, issue_id: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE feature_tasks SET issue_id = ?1 WHERE id = ?2",
            params![issue_id, task_id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature_task {}", task_id)));
        }
        Self::get_by_id(conn, task_id)
    }

    pub fn all_complete(conn: &Connection, feature_id: &str) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM feature_tasks WHERE feature_id = ?1 AND status = 'todo'",
            params![feature_id],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, String) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'TestProject', '/tmp', 'work')", []).unwrap();
        conn.execute("INSERT INTO features (id, project_id, title, status) VALUES ('f1', 'p1', 'Test Feature', 'idea')", []).unwrap();
        (conn, "f1".to_string())
    }

    #[test]
    fn test_create_and_list() {
        let (conn, feature_id) = setup();
        let t1 = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id: feature_id.clone(),
            title: "Task B".to_string(),
            sort_order: Some(2),
        }).unwrap();
        let t2 = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id: feature_id.clone(),
            title: "Task A".to_string(),
            sort_order: Some(1),
        }).unwrap();

        assert_eq!(t1.status, "todo");
        assert_eq!(t1.sort_order, 2);
        assert_eq!(t2.sort_order, 1);

        let list = FeatureTask::list_by_feature(&conn, &feature_id).unwrap();
        assert_eq!(list.len(), 2);
        // Ordered by sort_order ASC
        assert_eq!(list[0].title, "Task A");
        assert_eq!(list[1].title, "Task B");
    }

    #[test]
    fn test_update_status() {
        let (conn, feature_id) = setup();
        let task = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id,
            title: "My task".to_string(),
            sort_order: None,
        }).unwrap();
        assert_eq!(task.status, "todo");

        let updated = FeatureTask::update(&conn, &task.id, &UpdateFeatureTask {
            title: None,
            status: Some("done".to_string()),
            sort_order: None,
        }).unwrap();
        assert_eq!(updated.status, "done");
    }

    #[test]
    fn test_implement_links_issue() {
        let (conn, feature_id) = setup();
        let task = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id,
            title: "Implement auth".to_string(),
            sort_order: None,
        }).unwrap();
        assert!(task.issue_id.is_none());

        let linked = FeatureTask::implement(&conn, &task.id, "issue-42").unwrap();
        assert_eq!(linked.issue_id, Some("issue-42".to_string()));
    }

    #[test]
    fn test_all_complete() {
        let (conn, feature_id) = setup();
        // Create 3 tasks
        let t1 = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id: feature_id.clone(),
            title: "Task 1".to_string(),
            sort_order: Some(1),
        }).unwrap();
        let t2 = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id: feature_id.clone(),
            title: "Task 2".to_string(),
            sort_order: Some(2),
        }).unwrap();
        let t3 = FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id: feature_id.clone(),
            title: "Task 3".to_string(),
            sort_order: Some(3),
        }).unwrap();

        // Not complete yet (all todo)
        assert!(!FeatureTask::all_complete(&conn, &feature_id).unwrap());

        // Mark 2 done, 1 skipped
        FeatureTask::update(&conn, &t1.id, &UpdateFeatureTask {
            title: None, status: Some("done".to_string()), sort_order: None,
        }).unwrap();
        FeatureTask::update(&conn, &t2.id, &UpdateFeatureTask {
            title: None, status: Some("done".to_string()), sort_order: None,
        }).unwrap();
        FeatureTask::update(&conn, &t3.id, &UpdateFeatureTask {
            title: None, status: Some("skipped".to_string()), sort_order: None,
        }).unwrap();

        assert!(FeatureTask::all_complete(&conn, &feature_id).unwrap());

        // Add a new todo task — should no longer be all complete
        FeatureTask::create(&conn, &CreateFeatureTask {
            feature_id: feature_id.clone(),
            title: "Task 4".to_string(),
            sort_order: Some(4),
        }).unwrap();
        assert!(!FeatureTask::all_complete(&conn, &feature_id).unwrap());
    }
}
