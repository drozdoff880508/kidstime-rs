use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use crate::config::ConfigManager;

pub type SharedConfig = Arc<Mutex<ConfigManager>>;

fn check_auth(config: &ConfigManager, auth_header: Option<&str>) -> bool {
    if let Some(auth) = auth_header {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return config.verify_token(token);
        }
    }
    false
}

async fn index() -> Html<&'static str> {
    Html(crate::web_html::WEB_HTML)
}

async fn api_status(
    State(cfg): State<SharedConfig>,
    auth: axum::http::HeaderMap,
) -> impl IntoResponse {
    let cfg = cfg.lock().unwrap_or_else(|e| e.into_inner());
    let auth = auth.get("authorization").and_then(|v| v.to_str().ok());
    if !check_auth(&cfg, auth) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response();
    }
    let s = &cfg.state;
    (
        StatusCode::OK,
        Json(json!({
            "remaining_daily": cfg.remaining_daily(),
            "remaining_session": cfg.remaining_session(),
            "used_today": s.minutes_used_today,
            "is_blocked": s.is_blocked,
            "current_window": s.current_window,
            "current_process": s.current_process,
        })),
    )
        .into_response()
}

async fn api_dashboard(
    State(cfg): State<SharedConfig>,
    auth: axum::http::HeaderMap,
) -> impl IntoResponse {
    let cfg = cfg.lock().unwrap_or_else(|e| e.into_inner());
    let auth = auth.get("authorization").and_then(|v| v.to_str().ok());
    if !check_auth(&cfg, auth) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response();
    }
    let s = &cfg.state;
    let c = &cfg.config;
    let apps: Vec<Value> = cfg
        .history
        .get_app_summary_today()
        .iter()
        .map(|a| json!({"app": a.app, "minutes": a.minutes}))
        .collect();
    let today_entries: Vec<Value> = cfg
        .history
        .get_today()
        .iter()
        .map(|e| json!({"time": e.time, "window": e.window, "process": e.process}))
        .collect();
    let recent = today_entries.iter().rev().take(30).cloned().collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(json!({
            "remaining_daily": cfg.remaining_daily(),
            "remaining_session": cfg.remaining_session(),
            "used_today": s.minutes_used_today,
            "daily_limit": c.daily_limit_minutes,
            "session_limit": c.session_limit_minutes,
            "is_blocked": s.is_blocked,
            "in_schedule": cfg.is_in_schedule(),
            "schedule_start": c.schedule_start,
            "schedule_end": c.schedule_end,
            "current_window": s.current_window,
            "current_process": s.current_process,
            "apps": apps,
            "recent_activity": recent,
        })),
    )
        .into_response()
}

async fn api_lock(
    State(cfg): State<SharedConfig>,
    auth: axum::http::HeaderMap,
) -> impl IntoResponse {
    let auth = auth.get("authorization").and_then(|v| v.to_str().ok());
    { // scope for lock
        let config = cfg.lock().unwrap_or_else(|e| e.into_inner());
        if !check_auth(&config, auth) {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response();
        }
    }
    std::thread::spawn(|| crate::winapi::lock_screen());
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

async fn api_add_time(
    State(cfg): State<SharedConfig>,
    auth: axum::http::HeaderMap,
    body: Option<axum::Json<Value>>,
) -> impl IntoResponse {
    let auth = auth.get("authorization").and_then(|v| v.to_str().ok());
    let mut cfg = cfg.lock().unwrap_or_else(|e| e.into_inner());
    if !check_auth(&cfg, auth) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response();
    }
    let mins = body
        .and_then(|b| b.get("minutes").and_then(|m| m.as_u64()))
        .unwrap_or(30) as u32;
    let mins = mins.clamp(1, 300);
    cfg.grant_extra_time(mins);
    (StatusCode::OK, Json(json!({"ok": true, "added": mins}))).into_response()
}

async fn api_history(
    State(cfg): State<SharedConfig>,
    auth: axum::http::HeaderMap,
) -> impl IntoResponse {
    let cfg = cfg.lock().unwrap_or_else(|e| e.into_inner());
    let auth = auth.get("authorization").and_then(|v| v.to_str().ok());
    if !check_auth(&cfg, auth) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response();
    }
    let today: Vec<Value> = cfg
        .history
        .get_today()
        .iter()
        .map(|e| json!({"time": e.time, "window": e.window, "process": e.process}))
        .collect();
    let summary: Vec<Value> = cfg
        .history
        .get_app_summary_today()
        .iter()
        .map(|a| json!({"app": a.app, "minutes": a.minutes}))
        .collect();
    (StatusCode::OK, Json(json!({"today": today, "summary": summary}))).into_response()
}

pub fn create_router(shared: SharedConfig) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/status", get(api_status))
        .route("/api/dashboard", get(api_dashboard))
        .route("/api/lock", post(api_lock))
        .route("/api/add-time", post(api_add_time))
        .route("/api/history", get(api_history))
        .with_state(shared)
}

pub async fn run_web_server(shared: SharedConfig, port: u16) {
    let app = create_router(shared);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    println!("[Web] Panel running on http://0.0.0.0:{}", port);
    if let Err(e) = axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
            eprintln!("[Web] Failed to bind port {}: {}", port, e);
            std::process::exit(1);
        }),
        app,
    )
    .await
    {
        eprintln!("[Web] Server error: {}", e);
    }
}
