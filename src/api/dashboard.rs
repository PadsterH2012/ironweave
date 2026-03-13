use axum::{extract::State, Json};
use serde::Serialize;
use crate::state::AppState;
use crate::models::project::Project;
use crate::models::issue::Issue;

#[derive(Serialize)]
pub struct DashboardStats {
    pub project_count: usize,
    pub active_agents: usize,
    pub open_issues: usize,
    pub running_workflows: usize,
}

pub async fn stats(State(state): State<AppState>) -> Json<DashboardStats> {
    let project_count;
    let open_issues;
    let running_workflows;
    {
        let conn = state.db.lock().unwrap();
        project_count = Project::list(&conn).map(|p| p.len()).unwrap_or(0);
        open_issues = Issue::list(&conn)
            .map(|issues| issues.into_iter().filter(|i| i.status == "open").count())
            .unwrap_or(0);
        running_workflows = conn
            .prepare("SELECT COUNT(*) FROM workflow_instances WHERE state = 'running'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0) as usize;
    }
    let active_agents = state.process_manager.list_active().await.len();

    Json(DashboardStats {
        project_count,
        active_agents,
        open_issues,
        running_workflows,
    })
}
