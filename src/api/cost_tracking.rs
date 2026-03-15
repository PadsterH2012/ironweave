use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use crate::models::cost_tracking::{CostTrackingEntry, CostSummary, DailySpend};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CostQuery {
    pub period: Option<String>,
    pub days: Option<i64>,
}

pub async fn list_costs(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<CostQuery>,
) -> Result<Json<Vec<CostTrackingEntry>>, StatusCode> {
    let conn = state.conn()?;
    CostTrackingEntry::list_by_project(&conn, &pid, query.period.as_deref())
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_summary(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<CostQuery>,
) -> Result<Json<CostSummary>, StatusCode> {
    let conn = state.conn()?;
    let days = query.days.unwrap_or(30);
    CostTrackingEntry::project_summary(&conn, &pid, days)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_daily_spend(
    State(state): State<AppState>,
    Path(pid): Path<String>,
    Query(query): Query<CostQuery>,
) -> Result<Json<Vec<DailySpend>>, StatusCode> {
    let conn = state.conn()?;
    let days = query.days.unwrap_or(30);
    CostTrackingEntry::daily_spend(&conn, &pid, days)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn aggregate_now(
    State(state): State<AppState>,
    Path(pid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    CostTrackingEntry::aggregate_daily(&conn, &pid)
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
