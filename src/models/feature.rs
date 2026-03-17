use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};
use crate::models::knowledge_pattern::keyword_overlap_score;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub prd_content: Option<String>,
    pub implementation_notes: Option<String>,
    pub parked_at: Option<String>,
    pub parked_reason: Option<String>,
    pub priority: i64,
    pub keywords: String,
    pub gap_issue_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFeature {
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub prd_content: Option<String>,
    pub priority: Option<i64>,
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeature {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub prd_content: Option<String>,
    pub implementation_notes: Option<String>,
    pub priority: Option<i64>,
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct FeatureSummary {
    pub project_id: String,
    pub project_name: String,
    pub idea: i64,
    pub designed: i64,
    pub in_progress: i64,
    pub implemented: i64,
    pub verified: i64,
    pub parked: i64,
}

impl Feature {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: row.get("status")?,
            prd_content: row.get("prd_content")?,
            implementation_notes: row.get("implementation_notes")?,
            parked_at: row.get("parked_at")?,
            parked_reason: row.get("parked_reason")?,
            priority: row.get("priority")?,
            keywords: row.get("keywords")?,
            gap_issue_id: row.get("gap_issue_id").ok(),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateFeature) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let status = input.status.as_deref().unwrap_or("idea");
        let description = input.description.as_deref().unwrap_or("");
        let priority = input.priority.unwrap_or(5);
        let keywords_json = serde_json::to_string(
            &input.keywords.as_deref().unwrap_or(&[])
        ).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "INSERT INTO features (id, project_id, title, description, status, prd_content, priority, keywords)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, input.project_id, input.title, description, status, input.prd_content, priority, keywords_json],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM features WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("feature {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(
        conn: &Connection,
        project_id: &str,
        status_filter: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Self>> {
        let mut sql = "SELECT * FROM features WHERE project_id = ?1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id.to_string())];
        let mut idx = 2;

        if let Some(status) = status_filter {
            sql.push_str(&format!(" AND status = ?{}", idx));
            param_values.push(Box::new(status.to_string()));
            idx += 1;
        }

        sql.push_str(&format!(" ORDER BY priority DESC, created_at DESC LIMIT ?{}", idx));
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

    pub fn update(conn: &Connection, id: &str, input: &UpdateFeature) -> Result<Self> {
        let mut sets = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref title) = input.title {
            sets.push(format!("title = ?{}", idx));
            param_values.push(Box::new(title.clone()));
            idx += 1;
        }
        if let Some(ref description) = input.description {
            sets.push(format!("description = ?{}", idx));
            param_values.push(Box::new(description.clone()));
            idx += 1;
        }
        if let Some(ref status) = input.status {
            sets.push(format!("status = ?{}", idx));
            param_values.push(Box::new(status.clone()));
            idx += 1;
        }
        if let Some(ref prd_content) = input.prd_content {
            sets.push(format!("prd_content = ?{}", idx));
            param_values.push(Box::new(prd_content.clone()));
            idx += 1;
        }
        if let Some(ref implementation_notes) = input.implementation_notes {
            sets.push(format!("implementation_notes = ?{}", idx));
            param_values.push(Box::new(implementation_notes.clone()));
            idx += 1;
        }
        if let Some(priority) = input.priority {
            sets.push(format!("priority = ?{}", idx));
            param_values.push(Box::new(priority));
            idx += 1;
        }
        if let Some(ref keywords) = input.keywords {
            let kw_json = serde_json::to_string(keywords).unwrap_or_else(|_| "[]".to_string());
            sets.push(format!("keywords = ?{}", idx));
            param_values.push(Box::new(kw_json));
            idx += 1;
        }

        if sets.is_empty() {
            return Self::get_by_id(conn, id);
        }

        sets.push("updated_at = datetime('now')".to_string());

        let sql = format!(
            "UPDATE features SET {} WHERE id = ?{}",
            sets.join(", "),
            idx
        );
        param_values.push(Box::new(id.to_string()));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let changes = conn.execute(&sql, params_refs.as_slice())?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE features SET status = 'abandoned', updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn park(conn: &Connection, id: &str, reason: Option<&str>) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE features SET status = 'parked', parked_at = datetime('now'), parked_reason = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![reason, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn verify(conn: &Connection, id: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE features SET status = 'verified', updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn update_status(conn: &Connection, id: &str, status: &str) -> Result<Self> {
        let changes = conn.execute(
            "UPDATE features SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("feature {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn summary(conn: &Connection) -> Result<Vec<FeatureSummary>> {
        let mut stmt = conn.prepare(
            "SELECT f.project_id, p.name as project_name, f.status, COUNT(*) as cnt
             FROM features f
             JOIN projects p ON p.id = f.project_id
             GROUP BY f.project_id, f.status
             ORDER BY f.project_id"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>("project_id")?,
                row.get::<_, String>("project_name")?,
                row.get::<_, String>("status")?,
                row.get::<_, i64>("cnt")?,
            ))
        })?;

        let mut map: std::collections::HashMap<String, FeatureSummary> = std::collections::HashMap::new();
        for row in rows {
            let (project_id, project_name, status, cnt) = row?;
            let entry = map.entry(project_id.clone()).or_insert_with(|| FeatureSummary {
                project_id: project_id.clone(),
                project_name: project_name.clone(),
                idea: 0,
                designed: 0,
                in_progress: 0,
                implemented: 0,
                verified: 0,
                parked: 0,
            });
            match status.as_str() {
                "idea" => entry.idea = cnt,
                "designed" => entry.designed = cnt,
                "in_progress" => entry.in_progress = cnt,
                "implemented" => entry.implemented = cnt,
                "verified" => entry.verified = cnt,
                "parked" => entry.parked = cnt,
                _ => {}
            }
        }

        let mut result: Vec<FeatureSummary> = map.into_values().collect();
        result.sort_by(|a, b| a.project_id.cmp(&b.project_id));
        Ok(result)
    }

    pub fn find_related_parked(
        conn: &Connection,
        project_id: &str,
        keywords: &[String],
        min_score: f64,
    ) -> Result<Vec<Feature>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM features WHERE project_id = ?1 AND status = 'parked'"
        )?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;

        let mut results = Vec::new();
        for row in rows {
            let feature = row?;
            let feature_keywords: Vec<String> = serde_json::from_str(&feature.keywords).unwrap_or_default();
            let score = keyword_overlap_score(keywords, &feature_keywords);
            if score >= min_score {
                results.push(feature);
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute("INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'ProjectAlpha', '/tmp/alpha', 'work')", []).unwrap();
        conn.execute("INSERT INTO projects (id, name, directory, context) VALUES ('p2', 'ProjectBeta', '/tmp/beta', 'work')", []).unwrap();
        conn
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup();
        let input = CreateFeature {
            project_id: "p1".to_string(),
            title: "Auth system".to_string(),
            description: Some("Build authentication".to_string()),
            status: None,
            prd_content: Some("PRD content here".to_string()),
            priority: Some(8),
            keywords: Some(vec!["auth".to_string(), "login".to_string()]),
        };
        let feature = Feature::create(&conn, &input).unwrap();
        assert_eq!(feature.project_id, "p1");
        assert_eq!(feature.title, "Auth system");
        assert_eq!(feature.description, "Build authentication");
        assert_eq!(feature.status, "idea");
        assert_eq!(feature.prd_content, Some("PRD content here".to_string()));
        assert_eq!(feature.priority, 8);

        let kw: Vec<String> = serde_json::from_str(&feature.keywords).unwrap();
        assert_eq!(kw, vec!["auth", "login"]);

        let fetched = Feature::get_by_id(&conn, &feature.id).unwrap();
        assert_eq!(fetched.id, feature.id);
        assert_eq!(fetched.title, feature.title);
    }

    #[test]
    fn test_list_with_status_filter() {
        let conn = setup();
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Feature A".to_string(),
            description: None,
            status: Some("idea".to_string()),
            prd_content: None,
            priority: Some(5),
            keywords: None,
        }).unwrap();
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Feature B".to_string(),
            description: None,
            status: Some("designed".to_string()),
            prd_content: None,
            priority: Some(7),
            keywords: None,
        }).unwrap();
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Feature C".to_string(),
            description: None,
            status: Some("idea".to_string()),
            prd_content: None,
            priority: Some(9),
            keywords: None,
        }).unwrap();

        let all = Feature::list_by_project(&conn, "p1", None, 10).unwrap();
        assert_eq!(all.len(), 3);
        // Ordered by priority DESC: C(9), B(7), A(5)
        assert_eq!(all[0].title, "Feature C");
        assert_eq!(all[1].title, "Feature B");

        let ideas = Feature::list_by_project(&conn, "p1", Some("idea"), 10).unwrap();
        assert_eq!(ideas.len(), 2);

        let designed = Feature::list_by_project(&conn, "p1", Some("designed"), 10).unwrap();
        assert_eq!(designed.len(), 1);
        assert_eq!(designed[0].title, "Feature B");
    }

    #[test]
    fn test_park_and_resume() {
        let conn = setup();
        let feature = Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Parkable feature".to_string(),
            description: None,
            status: Some("idea".to_string()),
            prd_content: None,
            priority: None,
            keywords: None,
        }).unwrap();

        let parked = Feature::park(&conn, &feature.id, Some("Low priority")).unwrap();
        assert_eq!(parked.status, "parked");
        assert!(parked.parked_at.is_some());
        assert_eq!(parked.parked_reason, Some("Low priority".to_string()));

        let resumed = Feature::update_status(&conn, &feature.id, "idea").unwrap();
        assert_eq!(resumed.status, "idea");
    }

    #[test]
    fn test_soft_delete_to_abandoned() {
        let conn = setup();
        let feature = Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Doomed feature".to_string(),
            description: None,
            status: Some("idea".to_string()),
            prd_content: None,
            priority: None,
            keywords: None,
        }).unwrap();

        let deleted = Feature::delete(&conn, &feature.id).unwrap();
        assert_eq!(deleted.status, "abandoned");
        // Still retrievable
        let fetched = Feature::get_by_id(&conn, &feature.id).unwrap();
        assert_eq!(fetched.status, "abandoned");
    }

    #[test]
    fn test_summary() {
        let conn = setup();
        // Project 1: 2 ideas, 1 designed
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "P1 Idea 1".to_string(),
            description: None, status: Some("idea".to_string()),
            prd_content: None, priority: None, keywords: None,
        }).unwrap();
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "P1 Idea 2".to_string(),
            description: None, status: Some("idea".to_string()),
            prd_content: None, priority: None, keywords: None,
        }).unwrap();
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "P1 Designed".to_string(),
            description: None, status: Some("designed".to_string()),
            prd_content: None, priority: None, keywords: None,
        }).unwrap();

        // Project 2: 1 in_progress, 1 verified
        Feature::create(&conn, &CreateFeature {
            project_id: "p2".to_string(),
            title: "P2 InProgress".to_string(),
            description: None, status: Some("in_progress".to_string()),
            prd_content: None, priority: None, keywords: None,
        }).unwrap();
        Feature::create(&conn, &CreateFeature {
            project_id: "p2".to_string(),
            title: "P2 Verified".to_string(),
            description: None, status: Some("verified".to_string()),
            prd_content: None, priority: None, keywords: None,
        }).unwrap();

        let summaries = Feature::summary(&conn).unwrap();
        assert_eq!(summaries.len(), 2);

        let p1 = summaries.iter().find(|s| s.project_id == "p1").unwrap();
        assert_eq!(p1.project_name, "ProjectAlpha");
        assert_eq!(p1.idea, 2);
        assert_eq!(p1.designed, 1);
        assert_eq!(p1.in_progress, 0);

        let p2 = summaries.iter().find(|s| s.project_id == "p2").unwrap();
        assert_eq!(p2.project_name, "ProjectBeta");
        assert_eq!(p2.in_progress, 1);
        assert_eq!(p2.verified, 1);
        assert_eq!(p2.idea, 0);
    }

    #[test]
    fn test_find_related_parked() {
        let conn = setup();
        // Create a parked feature with keywords
        let f = Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Parked auth feature".to_string(),
            description: None,
            status: Some("idea".to_string()),
            prd_content: None,
            priority: None,
            keywords: Some(vec!["auth".to_string(), "login".to_string(), "oauth".to_string()]),
        }).unwrap();
        Feature::park(&conn, &f.id, Some("Deferred")).unwrap();

        // Create a non-parked feature (should not appear)
        Feature::create(&conn, &CreateFeature {
            project_id: "p1".to_string(),
            title: "Active feature".to_string(),
            description: None,
            status: Some("idea".to_string()),
            prd_content: None,
            priority: None,
            keywords: Some(vec!["auth".to_string(), "login".to_string()]),
        }).unwrap();

        // Search with overlapping keywords
        let query_kw = vec!["auth".to_string(), "login".to_string()];
        let found = Feature::find_related_parked(&conn, "p1", &query_kw, 0.5).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Parked auth feature");

        // Search with non-overlapping keywords
        let no_match_kw = vec!["database".to_string(), "migration".to_string()];
        let not_found = Feature::find_related_parked(&conn, "p1", &no_match_kw, 0.5).unwrap();
        assert_eq!(not_found.len(), 0);
    }
}
