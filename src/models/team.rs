use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{IronweaveError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub coordination_mode: String,
    pub max_agents: i64,
    pub token_budget: Option<i64>,
    pub cost_budget_daily: Option<f64>,
    pub is_template: bool,
    pub auto_pickup_types: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeam {
    pub name: String,
    pub project_id: String,
    pub coordination_mode: Option<String>,
    pub max_agents: Option<i64>,
    pub token_budget: Option<i64>,
    pub cost_budget_daily: Option<f64>,
    pub is_template: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamAgentSlot {
    pub id: String,
    pub team_id: String,
    pub role: String,
    pub runtime: String,
    pub model: Option<String>,
    pub config: String,
    pub slot_order: i64,
    pub is_lead: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamAgentSlot {
    #[serde(default)]
    pub team_id: String,
    pub role: String,
    pub runtime: String,
    pub model: Option<String>,
    pub config: Option<String>,
    pub slot_order: Option<i64>,
    pub is_lead: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamAgentSlot {
    pub role: Option<String>,
    pub runtime: Option<String>,
    pub model: Option<Option<String>>,
    pub slot_order: Option<i64>,
}

impl Team {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            project_id: row.get("project_id")?,
            coordination_mode: row.get("coordination_mode")?,
            max_agents: row.get("max_agents")?,
            token_budget: row.get("token_budget")?,
            cost_budget_daily: row.get("cost_budget_daily")?,
            is_template: row.get::<_, i64>("is_template")? != 0,
            auto_pickup_types: row.get("auto_pickup_types")?,
            is_active: row.get::<_, i64>("is_active")? != 0,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateTeam) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let coordination_mode = input.coordination_mode.as_deref().unwrap_or("pipeline");
        let max_agents = input.max_agents.unwrap_or(5);
        let is_template: i64 = if input.is_template.unwrap_or(false) { 1 } else { 0 };
        conn.execute(
            "INSERT INTO teams (id, name, project_id, coordination_mode, max_agents, token_budget, cost_budget_daily, is_template)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, input.name, input.project_id, coordination_mode, max_agents, input.token_budget, input.cost_budget_daily, is_template],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM teams WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("team {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM teams WHERE project_id = ?1 ORDER BY name")?;
        let rows = stmt.query_map(params![project_id], Self::from_row)?;
        let mut teams = Vec::new();
        for row in rows {
            teams.push(row?);
        }
        Ok(teams)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM teams WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("team {}", id)));
        }
        Ok(())
    }

    pub fn set_active(conn: &Connection, id: &str, active: bool) -> Result<Self> {
        let val: i64 = if active { 1 } else { 0 };
        let changes = conn.execute(
            "UPDATE teams SET is_active = ?1 WHERE id = ?2",
            params![val, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("team {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn get_auto_pickup_types(&self) -> Vec<String> {
        serde_json::from_str(&self.auto_pickup_types).unwrap_or_default()
    }

    pub fn update_auto_pickup_types(conn: &Connection, id: &str, types: &[&str]) -> Result<Self> {
        let json = serde_json::to_string(types)
            .map_err(|e| IronweaveError::Internal(e.to_string()))?;
        let changes = conn.execute(
            "UPDATE teams SET auto_pickup_types = ?1 WHERE id = ?2",
            params![json, id],
        )?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("team {}", id)));
        }
        Self::get_by_id(conn, id)
    }

    pub fn list_active(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM teams WHERE is_active = 1 AND is_template = 0 ORDER BY name"
        )?;
        let rows = stmt.query_map([], Self::from_row)?;
        let mut teams = Vec::new();
        for row in rows {
            teams.push(row?);
        }
        Ok(teams)
    }

    pub fn list_templates(conn: &Connection, project_id: Option<&str>) -> Result<Vec<Self>> {
        let mut templates = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT * FROM teams WHERE project_id = '__global__' AND is_template = 1 ORDER BY name"
        )?;
        let rows = stmt.query_map([], Self::from_row)?;
        for row in rows {
            templates.push(row?);
        }
        if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT * FROM teams WHERE project_id = ?1 AND is_template = 1 ORDER BY name"
            )?;
            let rows = stmt.query_map(params![pid], Self::from_row)?;
            for row in rows {
                templates.push(row?);
            }
        }
        Ok(templates)
    }

    pub fn clone_into_project(conn: &Connection, template_id: &str, project_id: &str) -> Result<Self> {
        let template = Self::get_by_id(conn, template_id)?;
        let new_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO teams (id, name, project_id, coordination_mode, max_agents, token_budget, cost_budget_daily, is_template)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![new_id, template.name, project_id, template.coordination_mode, template.max_agents, template.token_budget, template.cost_budget_daily],
        )?;

        let slots = TeamAgentSlot::list_by_team(conn, template_id)?;
        for slot in slots {
            let slot_id = Uuid::new_v4().to_string();
            let is_lead_val: i64 = if slot.is_lead { 1 } else { 0 };
            conn.execute(
                "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, config, slot_order, is_lead)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![slot_id, new_id, slot.role, slot.runtime, slot.model, slot.config, slot.slot_order, is_lead_val],
            )?;
        }

        Self::get_by_id(conn, &new_id)
    }
}

impl TeamAgentSlot {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            team_id: row.get("team_id")?,
            role: row.get("role")?,
            runtime: row.get("runtime")?,
            model: row.get("model")?,
            config: row.get("config")?,
            slot_order: row.get("slot_order")?,
            is_lead: row.get::<_, i64>("is_lead").unwrap_or(0) != 0,
        })
    }

    pub fn create(conn: &Connection, input: &CreateTeamAgentSlot) -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let config = input.config.as_deref().unwrap_or("{}");
        let slot_order = input.slot_order.unwrap_or(0);
        let is_lead: i64 = if input.is_lead.unwrap_or(false) { 1 } else { 0 };
        conn.execute(
            "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, config, slot_order, is_lead)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, input.team_id, input.role, input.runtime, input.model, config, slot_order, is_lead],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Self> {
        conn.query_row(
            "SELECT * FROM team_agent_slots WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => IronweaveError::NotFound(format!("team_agent_slot {}", id)),
            other => IronweaveError::Database(other),
        })
    }

    pub fn list_by_team(conn: &Connection, team_id: &str) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare("SELECT * FROM team_agent_slots WHERE team_id = ?1 ORDER BY slot_order")?;
        let rows = stmt.query_map(params![team_id], Self::from_row)?;
        let mut slots = Vec::new();
        for row in rows {
            slots.push(row?);
        }
        Ok(slots)
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        let changes = conn.execute("DELETE FROM team_agent_slots WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("team_agent_slot {}", id)));
        }
        Ok(())
    }

    pub fn update(conn: &Connection, id: &str, input: &UpdateTeamAgentSlot) -> Result<Self> {
        let mut sets = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref role) = input.role {
            sets.push("role = ?");
            values.push(Box::new(role.clone()));
        }
        if let Some(ref runtime) = input.runtime {
            sets.push("runtime = ?");
            values.push(Box::new(runtime.clone()));
        }
        if let Some(ref model) = input.model {
            sets.push("model = ?");
            values.push(Box::new(model.clone()));
        }
        if let Some(slot_order) = input.slot_order {
            sets.push("slot_order = ?");
            values.push(Box::new(slot_order));
        }

        if sets.is_empty() {
            return Self::get_by_id(conn, id);
        }

        let sql = format!("UPDATE team_agent_slots SET {} WHERE id = ?", sets.join(", "));
        values.push(Box::new(id.to_string()));

        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        let changes = conn.execute(&sql, params.as_slice())?;
        if changes == 0 {
            return Err(IronweaveError::NotFound(format!("team_agent_slot {}", id)));
        }
        Self::get_by_id(conn, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{CreateProject, Project};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn create_test_project(conn: &Connection) -> Project {
        Project::create(conn, &CreateProject {
            name: "Test".to_string(),
            directory: "/tmp/test".to_string(),
            context: "homelab".to_string(),
            obsidian_vault_path: None,
            obsidian_project: None,
            git_remote: None,
            mount_id: None,
        }).unwrap()
    }

    #[test]
    fn test_team_create_and_get() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Backend".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        assert_eq!(team.name, "Backend");
        assert_eq!(team.coordination_mode, "pipeline");

        let fetched = Team::get_by_id(&conn, &team.id).unwrap();
        assert_eq!(fetched.id, team.id);
    }

    #[test]
    fn test_team_list_by_project() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        Team::create(&conn, &CreateTeam {
            name: "Alpha".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        Team::create(&conn, &CreateTeam {
            name: "Beta".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();

        let teams = Team::list_by_project(&conn, &project.id).unwrap();
        assert_eq!(teams.len(), 2);
        assert_eq!(teams[0].name, "Alpha");
    }

    #[test]
    fn test_team_delete() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Temp".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        Team::delete(&conn, &team.id).unwrap();
        assert!(Team::get_by_id(&conn, &team.id).is_err());
    }

    #[test]
    fn test_agent_slot_create_and_list() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Dev".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();

        TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "coder".to_string(),
            runtime: "claude".to_string(),
            model: None,
            config: None,
            slot_order: Some(1),
            is_lead: None,
        }).unwrap();

        TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "reviewer".to_string(),
            runtime: "gemini".to_string(),
            model: None,
            config: None,
            slot_order: Some(2),
            is_lead: None,
        }).unwrap();

        let slots = TeamAgentSlot::list_by_team(&conn, &team.id).unwrap();
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].role, "coder");
        assert_eq!(slots[1].role, "reviewer");
    }

    #[test]
    fn test_agent_slot_delete() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Dev".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        let slot = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "coder".to_string(),
            runtime: "claude".to_string(),
            model: None,
            config: None,
            slot_order: None,
            is_lead: None,
        }).unwrap();
        TeamAgentSlot::delete(&conn, &slot.id).unwrap();
        assert!(TeamAgentSlot::get_by_id(&conn, &slot.id).is_err());
    }

    #[test]
    fn test_slot_with_model() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Dev".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();

        let slot = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "coder".to_string(),
            runtime: "claude".to_string(),
            model: Some("claude-sonnet-4-6".to_string()),
            config: None,
            slot_order: Some(1),
            is_lead: None,
        }).unwrap();

        assert_eq!(slot.model.as_deref(), Some("claude-sonnet-4-6"));

        let slot2 = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "reviewer".to_string(),
            runtime: "claude".to_string(),
            model: None,
            config: None,
            slot_order: Some(2),
            is_lead: None,
        }).unwrap();
        assert!(slot2.model.is_none());
    }

    #[test]
    fn test_slot_update() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Dev".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();

        let slot = TeamAgentSlot::create(&conn, &CreateTeamAgentSlot {
            team_id: team.id.clone(),
            role: "coder".to_string(),
            runtime: "claude".to_string(),
            model: Some("claude-sonnet-4-6".to_string()),
            config: None,
            slot_order: Some(1),
            is_lead: None,
        }).unwrap();

        let updated = TeamAgentSlot::update(&conn, &slot.id, &UpdateTeamAgentSlot {
            role: Some("architect".to_string()),
            runtime: None,
            model: Some(Some("claude-opus-4-6".to_string())),
            slot_order: None,
        }).unwrap();

        assert_eq!(updated.role, "architect");
        assert_eq!(updated.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(updated.runtime, "claude");
    }

    #[test]
    fn test_list_templates() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('__global__', '__global__', '', 'work')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO teams (id, name, project_id, coordination_mode, is_template) VALUES ('t1', 'Template', '__global__', 'pipeline', 1)",
            [],
        ).unwrap();

        let templates = Team::list_templates(&conn, None).unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "Template");
    }

    #[test]
    fn test_clone_template() {
        let conn = setup_db();
        let project = create_test_project(&conn);

        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('__global__', '__global__', '', 'work')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO teams (id, name, project_id, coordination_mode, is_template, max_agents) VALUES ('t1', 'Template', '__global__', 'pipeline', 1, 3)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO team_agent_slots (id, team_id, role, runtime, model, slot_order) VALUES ('s1', 't1', 'coder', 'claude', 'claude-sonnet-4-6', 0)",
            [],
        ).unwrap();

        let cloned = Team::clone_into_project(&conn, "t1", &project.id).unwrap();
        assert_eq!(cloned.name, "Template");
        assert_eq!(cloned.project_id, project.id);
        assert!(!cloned.is_template);

        let slots = TeamAgentSlot::list_by_team(&conn, &cloned.id).unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].role, "coder");
        assert_eq!(slots[0].model.as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn test_team_activate_deactivate() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Active".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();

        assert!(!team.is_active);

        let activated = Team::set_active(&conn, &team.id, true).unwrap();
        assert!(activated.is_active);

        let deactivated = Team::set_active(&conn, &team.id, false).unwrap();
        assert!(!deactivated.is_active);
    }

    #[test]
    fn test_team_auto_pickup_types() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        let team = Team::create(&conn, &CreateTeam {
            name: "Pickup".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();

        // Default includes all three types
        let types = team.get_auto_pickup_types();
        assert!(types.contains(&"task".to_string()));
        assert!(types.contains(&"bug".to_string()));
        assert!(types.contains(&"feature".to_string()));

        // Update to only bugs
        let updated = Team::update_auto_pickup_types(&conn, &team.id, &["bug"]).unwrap();
        let types = updated.get_auto_pickup_types();
        assert_eq!(types, vec!["bug".to_string()]);
        assert!(!types.contains(&"task".to_string()));
    }

    #[test]
    fn test_list_active_teams() {
        let conn = setup_db();
        let project = create_test_project(&conn);
        Team::create(&conn, &CreateTeam {
            name: "Inactive".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        let active_team = Team::create(&conn, &CreateTeam {
            name: "Active".to_string(),
            project_id: project.id.clone(),
            coordination_mode: None,
            max_agents: None,
            token_budget: None,
            cost_budget_daily: None,
            is_template: None,
        }).unwrap();
        Team::set_active(&conn, &active_team.id, true).unwrap();

        let active = Team::list_active(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Active");
    }
}
