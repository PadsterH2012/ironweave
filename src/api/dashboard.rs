use axum::{extract::{Query, State}, Json};
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
) -> Json<Vec<ActivityLogEntry>> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let conn = state.db.lock().unwrap();

    let entries = if let Some(ref project_id) = params.project_id {
        ActivityLogEntry::list_by_project(&conn, project_id, limit).unwrap_or_default()
    } else {
        ActivityLogEntry::list_recent(&conn, limit, offset).unwrap_or_default()
    };

    Json(entries)
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
) -> Json<MetricsResponse> {
    let days = params.days.unwrap_or(7);
    let conn = state.db.lock().unwrap();

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

    Json(MetricsResponse {
        daily,
        merge_stats,
        avg_resolution_hours,
    })
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
