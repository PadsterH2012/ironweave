use axum::{extract::{Path, Query, State}, Json, http::StatusCode};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::project::Project;
use crate::models::test_run::{TestRun, CreateTestRun};

#[derive(Deserialize)]
pub struct TriggerRunRequest {
    pub test_type: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
}

pub async fn trigger_run(
    Path(pid): Path<String>,
    State(state): State<AppState>,
    body: Option<Json<TriggerRunRequest>>,
) -> Result<(StatusCode, Json<TestRun>), StatusCode> {
    let test_type = body
        .and_then(|b| b.0.test_type)
        .unwrap_or_else(|| "e2e".to_string());

    let (directory, app_url) = {
        let conn = state.conn()?;
        let project = Project::get_by_id(&conn, &pid).map_err(|_| StatusCode::NOT_FOUND)?;
        (project.directory, project.app_url)
    };

    let run = {
        let conn = state.conn()?;
        TestRun::create(&conn, &CreateTestRun {
            project_id: pid.clone(),
            test_type: test_type.clone(),
            target_url: app_url.clone(),
            triggered_by: "manual".to_string(),
        }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let run_id = run.id.clone();
    let db = state.db.clone();

    tokio::spawn(async move {
        // Update status to running
        {
            let conn = db.lock().unwrap();
            let _ = TestRun::update_status(&conn, &run_id, "running");
        }

        let start = std::time::Instant::now();

        // Find the test directory: check project dir first, then parent dirs
        let test_dir = find_test_dir(&directory);

        let mut cmd = tokio::process::Command::new("npx");
        cmd.arg("playwright").arg("test").arg("--reporter=json");

        if let Some(ref td) = test_dir {
            cmd.arg("--config").arg(td.join("playwright.config.ts"));
            cmd.current_dir(td);
        } else {
            cmd.current_dir(&directory);
        }

        if let Some(ref url) = app_url {
            cmd.env("BASE_URL", url);
        }

        let result = cmd.output().await;

        let elapsed = start.elapsed().as_secs_f64();

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = if stderr.is_empty() {
                    stdout.to_string()
                } else {
                    format!("{}\n{}", stdout, stderr)
                };

                // Parse JSON output
                let (status, total, passed, failed, skipped, failed_tests) =
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        let stats = &json["stats"];
                        let expected = stats["expected"].as_i64().unwrap_or(0);
                        let unexpected = stats["unexpected"].as_i64().unwrap_or(0);
                        let skipped_count = stats["skipped"].as_i64().unwrap_or(0);
                        let total_count = expected + unexpected + skipped_count;

                        // Collect failed test names (recursing into nested suites)
                        let mut failed_names: Vec<String> = Vec::new();
                        if let Some(suites) = json["suites"].as_array() {
                            collect_failed_tests(suites, &mut failed_names);
                        }

                        let status_str = if unexpected > 0 { "failed" } else { "passed" };
                        let ft_json = serde_json::to_string(&failed_names).unwrap_or_else(|_| "[]".to_string());
                        (status_str, total_count, expected, unexpected, skipped_count, ft_json)
                    } else {
                        // Could not parse JSON — treat as error
                        ("error", 0i64, 0i64, 0i64, 0i64, "[]".to_string())
                    };

                let conn = db.lock().unwrap();
                let _ = TestRun::complete(
                    &conn,
                    &run_id,
                    status,
                    total,
                    passed,
                    failed,
                    skipped,
                    elapsed,
                    Some(&combined),
                    Some(&failed_tests),
                );
            }
            Err(e) => {
                let conn = db.lock().unwrap();
                let _ = TestRun::complete(
                    &conn,
                    &run_id,
                    "error",
                    0,
                    0,
                    0,
                    0,
                    elapsed,
                    Some(&format!("Failed to execute: {}", e)),
                    None,
                );
            }
        }
    });

    Ok((StatusCode::CREATED, Json(run)))
}

pub async fn list_runs(
    Path(pid): Path<String>,
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<TestRun>>, StatusCode> {
    let limit = query.limit.unwrap_or(50);
    let conn = state.conn()?;
    TestRun::list_by_project(&conn, &pid, limit)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_run(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<TestRun>, StatusCode> {
    let conn = state.conn()?;
    TestRun::get_by_id(&conn, &id)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn latest_run(
    Path(pid): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Option<TestRun>>, StatusCode> {
    let conn = state.conn()?;
    TestRun::latest_by_project(&conn, &pid)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn stop_run(
    Path((_pid, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let conn = state.conn()?;
    TestRun::update_status(&conn, &id, "error")
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

/// Find the test directory by searching for tests/e2e/playwright.config.ts.
/// Checks: project dir, then walks up parent dirs (handles mounts pointing into repos).
fn find_test_dir(project_dir: &str) -> Option<std::path::PathBuf> {
    let project_path = std::path::Path::new(project_dir);

    // Check project_dir/tests/e2e/
    let candidate = project_path.join("tests/e2e");
    if candidate.join("playwright.config.ts").exists() {
        return Some(candidate);
    }

    // Walk up parent directories (max 5 levels) looking for tests/e2e/
    let mut current = project_path.to_path_buf();
    for _ in 0..5 {
        if let Some(parent) = current.parent() {
            let candidate = parent.join("tests/e2e");
            if candidate.join("playwright.config.ts").exists() {
                return Some(candidate);
            }
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    None
}

/// Recursively walk Playwright JSON suites to collect failed test names.
fn collect_failed_tests(suites: &[serde_json::Value], names: &mut Vec<String>) {
    for suite in suites {
        // Check specs at this level
        if let Some(specs) = suite["specs"].as_array() {
            for spec in specs {
                if let Some(tests) = spec["tests"].as_array() {
                    for test in tests {
                        if let Some(results) = test["results"].as_array() {
                            for r in results {
                                if r["status"].as_str() == Some("failed") {
                                    if let Some(title) = spec["title"].as_str() {
                                        names.push(title.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Recurse into nested suites
        if let Some(sub_suites) = suite["suites"].as_array() {
            collect_failed_tests(sub_suites, names);
        }
    }
}
