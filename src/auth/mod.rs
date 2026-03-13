use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::state::AppState;

// ── Config types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct UserConfig {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub users: Vec<UserConfig>,
    pub session_ttl_hours: Option<u64>,
}

// ── Session table migration ──────────────────────────────────────

pub fn create_sessions_table(db: &DbPool) -> Result<(), rusqlite::Error> {
    let conn = db.lock().unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            token TEXT PRIMARY KEY,
            username TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL
        );"
    )?;
    Ok(())
}

// ── Password functions ───────────────────────────────────────────

pub fn verify_password(hash: &str, password: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("failed to hash password")
        .to_string()
}

// ── Session management ───────────────────────────────────────────

pub fn create_session(db: &DbPool, username: &str, ttl_hours: u64) -> Result<String, rusqlite::Error> {
    let token: String = {
        let mut rng = rand::thread_rng();
        (0..64)
            .map(|_| {
                let idx = rng.gen_range(0..36);
                if idx < 10 {
                    (b'0' + idx) as char
                } else {
                    (b'a' + idx - 10) as char
                }
            })
            .collect()
    };

    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO sessions (token, username, expires_at) VALUES (?1, ?2, datetime('now', '+' || ?3 || ' hours'))",
        rusqlite::params![token, username, ttl_hours],
    )?;
    Ok(token)
}

pub fn validate_session(db: &DbPool, token: &str) -> Result<Option<String>, rusqlite::Error> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT username FROM sessions WHERE token = ?1 AND expires_at > datetime('now')"
    )?;
    let mut rows = stmt.query(rusqlite::params![token])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn delete_session(db: &DbPool, token: &str) -> Result<(), rusqlite::Error> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM sessions WHERE token = ?1", rusqlite::params![token])?;
    Ok(())
}

// ── Axum middleware ──────────────────────────────────────────────

fn extract_token(req: &Request<Body>) -> Option<String> {
    // Try Authorization: Bearer <token> header first
    if let Some(auth) = req.headers().get("authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    // Fall back to session cookie
    if let Some(cookie) = req.headers().get("cookie") {
        if let Ok(val) = cookie.to_str() {
            for part in val.split(';') {
                let part = part.trim();
                if let Some(token) = part.strip_prefix("session=") {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

fn is_public_path(path: &str) -> bool {
    path == "/api/auth/login"
        || path == "/api/health"
        || !path.starts_with("/api/")
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    if is_public_path(&path) {
        return next.run(req).await;
    }

    let token = match extract_token(&req) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match validate_session(&state.db, &token) {
        Ok(Some(_username)) => next.run(req).await,
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

// ── Login / Logout handlers ──────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Response {
    let auth_config = match &state.auth_config {
        Some(c) => c,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let user = auth_config.users.iter().find(|u| u.username == body.username);
    let user = match user {
        Some(u) => u,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };

    if !verify_password(&user.password_hash, &body.password) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let ttl = auth_config.session_ttl_hours.unwrap_or(24);
    match create_session(&state.db, &body.username, ttl) {
        Ok(token) => Json(LoginResponse { token }).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn logout(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    if let Some(token) = extract_token(&req) {
        let _ = delete_session(&state.db, &token);
    }
    StatusCode::OK.into_response()
}
