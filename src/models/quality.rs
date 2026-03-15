use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityTier {
    pub tier: i32,
    pub label: String,
    pub example_models: String,
    pub cost_range: String,
    pub max_context_tokens: i64,
    pub max_output_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRange {
    pub tier_floor: i32,
    pub tier_ceiling: i32,
}

#[derive(Debug, Deserialize)]
pub struct SetTierRange {
    pub tier_floor: Option<i32>,
    pub tier_ceiling: Option<i32>,
}

impl QualityTier {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            tier: row.get("tier")?,
            label: row.get("label")?,
            example_models: row.get("example_models")?,
            cost_range: row.get("cost_range")?,
            max_context_tokens: row.get("max_context_tokens").unwrap_or(200000),
            max_output_tokens: row.get("max_output_tokens").unwrap_or(8192),
        })
    }

    pub fn list(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM quality_tiers ORDER BY tier")?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut tiers = Vec::new();
        for row in rows {
            tiers.push(row?);
        }
        Ok(tiers)
    }

    pub fn get(conn: &Connection, tier: i32) -> Result<Self> {
        let t = conn.query_row(
            "SELECT * FROM quality_tiers WHERE tier = ?1",
            params![tier],
            Self::from_row,
        )?;
        Ok(t)
    }
}

impl TierRange {
    /// Get effective tier range for a project
    pub fn for_project(conn: &Connection, project_id: &str) -> Result<Self> {
        let (floor, ceiling) = conn.query_row(
            "SELECT tier_floor, tier_ceiling FROM projects WHERE id = ?1",
            params![project_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)),
        )?;
        Ok(Self { tier_floor: floor, tier_ceiling: ceiling })
    }

    /// Get effective tier range for a team (inherits from project if null)
    pub fn for_team(conn: &Connection, team_id: &str) -> Result<Self> {
        let (team_floor, team_ceiling, proj_floor, proj_ceiling) = conn.query_row(
            "SELECT t.tier_floor, t.tier_ceiling, p.tier_floor, p.tier_ceiling
             FROM teams t JOIN projects p ON t.project_id = p.id
             WHERE t.id = ?1",
            params![team_id],
            |row| Ok((
                row.get::<_, Option<i32>>(0)?,
                row.get::<_, Option<i32>>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
            )),
        )?;
        Ok(Self {
            tier_floor: team_floor.unwrap_or(proj_floor),
            tier_ceiling: team_ceiling.unwrap_or(proj_ceiling),
        })
    }

    /// Set tier range on a project
    pub fn set_project(conn: &Connection, project_id: &str, input: &SetTierRange) -> Result<Self> {
        if let Some(floor) = input.tier_floor {
            conn.execute(
                "UPDATE projects SET tier_floor = ?1 WHERE id = ?2",
                params![floor, project_id],
            )?;
        }
        if let Some(ceiling) = input.tier_ceiling {
            conn.execute(
                "UPDATE projects SET tier_ceiling = ?1 WHERE id = ?2",
                params![ceiling, project_id],
            )?;
        }
        Self::for_project(conn, project_id)
    }

    /// Reset project tier range to defaults (1-5)
    pub fn reset_project(conn: &Connection, project_id: &str) -> Result<Self> {
        conn.execute(
            "UPDATE projects SET tier_floor = 1, tier_ceiling = 5 WHERE id = ?1",
            params![project_id],
        )?;
        Self::for_project(conn, project_id)
    }

    /// Set tier range on a team (null = inherit from project)
    pub fn set_team(conn: &Connection, team_id: &str, input: &SetTierRange) -> Result<Self> {
        conn.execute(
            "UPDATE teams SET tier_floor = ?1, tier_ceiling = ?2 WHERE id = ?3",
            params![input.tier_floor, input.tier_ceiling, team_id],
        )?;
        Self::for_team(conn, team_id)
    }

    /// Check if a given tier is allowed within this range
    pub fn allows(&self, tier: i32) -> bool {
        tier >= self.tier_floor && tier <= self.tier_ceiling
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'Test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO teams (id, name, project_id) VALUES ('t1', 'Dev', 'p1')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_list_quality_tiers() {
        let conn = setup_db();
        let tiers = QualityTier::list(&conn).unwrap();
        assert_eq!(tiers.len(), 5);
        assert_eq!(tiers[0].label, "Free/Local");
        assert_eq!(tiers[4].label, "Premium");
    }

    #[test]
    fn test_project_default_tier_range() {
        let conn = setup_db();
        let range = TierRange::for_project(&conn, "p1").unwrap();
        assert_eq!(range.tier_floor, 1);
        assert_eq!(range.tier_ceiling, 5);
    }

    #[test]
    fn test_set_project_tier_range() {
        let conn = setup_db();
        let range = TierRange::set_project(&conn, "p1", &SetTierRange {
            tier_floor: Some(2),
            tier_ceiling: Some(4),
        }).unwrap();
        assert_eq!(range.tier_floor, 2);
        assert_eq!(range.tier_ceiling, 4);
        assert!(range.allows(3));
        assert!(!range.allows(1));
        assert!(!range.allows(5));
    }

    #[test]
    fn test_team_inherits_project_tiers() {
        let conn = setup_db();
        // Team has null tiers — should inherit project defaults
        let range = TierRange::for_team(&conn, "t1").unwrap();
        assert_eq!(range.tier_floor, 1);
        assert_eq!(range.tier_ceiling, 5);

        // Set project to 2-4
        TierRange::set_project(&conn, "p1", &SetTierRange {
            tier_floor: Some(2),
            tier_ceiling: Some(4),
        }).unwrap();

        let range = TierRange::for_team(&conn, "t1").unwrap();
        assert_eq!(range.tier_floor, 2);
        assert_eq!(range.tier_ceiling, 4);
    }

    #[test]
    fn test_team_override_tiers() {
        let conn = setup_db();
        TierRange::set_team(&conn, "t1", &SetTierRange {
            tier_floor: Some(3),
            tier_ceiling: Some(5),
        }).unwrap();

        let range = TierRange::for_team(&conn, "t1").unwrap();
        assert_eq!(range.tier_floor, 3);
        assert_eq!(range.tier_ceiling, 5);
    }

    #[test]
    fn test_reset_project_tiers() {
        let conn = setup_db();
        TierRange::set_project(&conn, "p1", &SetTierRange {
            tier_floor: Some(3),
            tier_ceiling: Some(3),
        }).unwrap();
        let range = TierRange::reset_project(&conn, "p1").unwrap();
        assert_eq!(range.tier_floor, 1);
        assert_eq!(range.tier_ceiling, 5);
    }
}
