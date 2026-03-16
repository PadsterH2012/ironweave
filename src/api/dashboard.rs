use axum::{extract::{Query, State}, Json};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::project::Project;
use crate::models::issue::Issue;
use crate::models::activity_log::{ActivityLogEntry, DailyMetric};

#[derive(Serialize)]
pub struct DashboardStats {
    pub project_count: usize,
    pub active_agents: usize,
    pub open_issues: usize,
    pub in_progress_issues: usize,
    pub closed_issues: usize,
    pub running_workflows: usize,
    pub current_work: Vec<CurrentWorkItem>,
}

#[derive(Serialize)]
pub struct CurrentWorkItem {
    pub issue_id: String,
    pub title: String,
    pub role: String,
    pub status: String,
    pub project_name: String,
    pub agent_runtime: Option<String>,
    pub agent_state: Option<String>,
    pub updated_at: Option<String>,
}

pub async fn stats(State(state): State<AppState>) -> Result<Json<DashboardStats>, StatusCode> {
    let project_count;
    let open_issues;
    let in_progress_issues;
    let closed_issues;
    let running_workflows;
    let current_work;
    {
        let conn = state.conn()?;
        project_count = Project::list(&conn).map(|p| p.len()).unwrap_or(0);

        let all_issues = Issue::list(&conn).unwrap_or_default();
        open_issues = all_issues.iter().filter(|i| i.status == "open").count();
        in_progress_issues = all_issues.iter().filter(|i| i.status == "in_progress").count();
        closed_issues = all_issues.iter().filter(|i| i.status == "closed").count();

        running_workflows = conn
            .prepare("SELECT COUNT(*) FROM workflow_instances WHERE state = 'running'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0) as usize;

        // Get in-progress issues with their agent info
        current_work = conn.prepare(
            "SELECT i.id, i.title, i.role, i.status, p.name as project_name, i.updated_at,
                    s.runtime as agent_runtime, s.state as agent_state
             FROM issues i
             JOIN projects p ON i.project_id = p.id
             LEFT JOIN agent_sessions s ON s.claimed_task_id = i.id AND s.state IN ('idle', 'working', 'blocked')
             WHERE i.status IN ('in_progress', 'review')
             ORDER BY i.updated_at DESC
             LIMIT 20"
        ).and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| {
                Ok(CurrentWorkItem {
                    issue_id: row.get("id")?,
                    title: row.get("title")?,
                    role: row.get("role")?,
                    status: row.get("status")?,
                    project_name: row.get("project_name")?,
                    updated_at: row.get("updated_at")?,
                    agent_runtime: row.get("agent_runtime")?,
                    agent_state: row.get("agent_state")?,
                })
            })?;
            let mut items = Vec::new();
            for row in rows {
                if let Ok(item) = row {
                    items.push(item);
                }
            }
            Ok(items)
        }).unwrap_or_default();
    }
    let active_agents = state.process_manager.list_active().await.len();

    Ok(Json(DashboardStats {
        project_count,
        active_agents,
        open_issues,
        in_progress_issues,
        closed_issues,
        running_workflows,
        current_work,
    }))
}

// --- Activity endpoint ---

#[derive(Deserialize)]
pub struct ActivityQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub project_id: Option<String>,
}

pub async fn activity(
    State(state): State<AppState>,
    Query(params): Query<ActivityQuery>,
) -> Result<Json<Vec<ActivityLogEntry>>, StatusCode> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let conn = state.conn()?;

    let entries = if let Some(ref project_id) = params.project_id {
        ActivityLogEntry::list_by_project(&conn, project_id, limit).unwrap_or_default()
    } else {
        ActivityLogEntry::list_recent(&conn, limit, offset).unwrap_or_default()
    };

    Ok(Json(entries))
}

// --- Metrics endpoint ---

#[derive(Deserialize)]
pub struct MetricsQuery {
    pub days: Option<i64>,
}

#[derive(Serialize)]
pub struct MergeStats {
    pub total: i64,
    pub clean: i64,
    pub conflicted: i64,
    pub escalated: i64,
}

#[derive(Serialize)]
pub struct MetricsResponse {
    pub daily: Vec<DailyMetric>,
    pub merge_stats: MergeStats,
    pub avg_resolution_hours: f64,
}

pub async fn metrics(
    State(state): State<AppState>,
    Query(params): Query<MetricsQuery>,
) -> Result<Json<MetricsResponse>, StatusCode> {
    let days = params.days.unwrap_or(7);
    let conn = state.conn()?;

    let daily = ActivityLogEntry::daily_metrics(&conn, days).unwrap_or_default();

    let merge_stats = {
        let total = conn
            .prepare("SELECT COUNT(*) FROM activity_log WHERE event_type LIKE 'merge_%'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0);
        let clean = conn
            .prepare("SELECT COUNT(*) FROM activity_log WHERE event_type = 'merge_clean'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0);
        let conflicted = conn
            .prepare("SELECT COUNT(*) FROM activity_log WHERE event_type = 'merge_conflicted'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0);
        let escalated = conn
            .prepare("SELECT COUNT(*) FROM activity_log WHERE event_type = 'merge_escalated'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0);
        MergeStats { total, clean, conflicted, escalated }
    };

    let avg_resolution_hours: f64 = conn
        .prepare(
            "SELECT AVG((julianday(updated_at) - julianday(created_at)) * 24.0)
             FROM issues WHERE status = 'closed' AND updated_at IS NOT NULL"
        )
        .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, Option<f64>>(0)))
        .unwrap_or(None)
        .unwrap_or(0.0);

    Ok(Json(MetricsResponse {
        daily,
        merge_stats,
        avg_resolution_hours,
    }))
}

// --- System endpoint ---

#[derive(Serialize)]
pub struct SystemHealth {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: u64,
    pub disk_total_gb: u64,
    pub agent_process_count: usize,
}

pub async fn system(State(state): State<AppState>) -> Json<SystemHealth> {
    use sysinfo::{System, Disks};

    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage_percent = sys.global_cpu_usage();
    let memory_used_mb = sys.used_memory() / (1024 * 1024);
    let memory_total_mb = sys.total_memory() / (1024 * 1024);

    let disks = Disks::new_with_refreshed_list();
    let (disk_used_gb, disk_total_gb) = disks.list().iter().fold((0u64, 0u64), |(used, total), d| {
        let t = d.total_space() / (1024 * 1024 * 1024);
        let a = d.available_space() / (1024 * 1024 * 1024);
        (used + (t - a), total + t)
    });

    let agent_process_count = state.process_manager.list_active().await.len();

    Json(SystemHealth {
        cpu_usage_percent,
        memory_used_mb,
        memory_total_mb,
        disk_used_gb,
        disk_total_gb,
        agent_process_count,
    })
}
