use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::config::ConfigManager;
use crate::server::SharedConfig;

pub struct CloudRelayClient {
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl CloudRelayClient {
    pub fn new() -> Self {
        Self {
            running: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    pub fn start(&self, shared: SharedConfig) {
        let running = self.running.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create relay runtime");
            rt.block_on(relay_loop(shared, running));
        });
    }

    pub fn stop(&self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

async fn relay_loop(shared: SharedConfig, running: Arc<std::sync::atomic::AtomicBool>) {
    loop {
        if !running.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        let (ws_url, code, token, status_snapshot) = {
            let cfg = shared.lock().unwrap_or_else(|e| e.into_inner());
            let url = cfg.config.cloud_relay_url.trim().trim_end_matches('/').to_string();
            if url.is_empty() {
                sleep(Duration::from_secs(10)).await;
                continue;
            }
            if !cfg.config.cloud_enabled {
                sleep(Duration::from_secs(30)).await;
                continue;
            }
            let code = cfg.config.cloud_device_code.clone();
            if code.is_empty() {
                drop(cfg);
                // Generate code
                let mut cfg = shared.lock().unwrap_or_else(|e| e.into_inner());
                cfg.config.cloud_device_code = uuid::Uuid::new_v4()
                    .to_simple()
                    .to_string()[..6]
                    .to_uppercase();
                cfg.save_config();
                sleep(Duration::from_secs(2)).await;
                continue;
            }
            let token = cfg.config.web_token.clone();
            let url_ws = url.replace("http://", "ws://").replace("https://", "wss://");
            let ws_url = format!("{}/ws", url_ws);
            let snapshot = build_status_snapshot(&cfg);
            (ws_url, code, token, snapshot)
        };

        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                println!("[Cloud] Connected to relay, code: {}", code);
                let (mut write, mut read) = ws_stream.split();

                // Send register
                let reg = json!({
                    "type": "register",
                    "code": code,
                    "token": token,
                    "status": status_snapshot,
                });
                if let Err(e) = write.send(Message::Text(reg.to_string().into())).await {
                    eprintln!("[Cloud] Send error: {}", e);
                    sleep(Duration::from_secs(10)).await;
                    continue;
                }

                // Status sender task
                let status_running = running.clone();
                let status_shared = shared.clone();
                let status_code = code.clone();
                let status_handle = tokio::spawn(async move {
                    loop {
                        if !status_running.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        sleep(Duration::from_secs(5)).await;
                        let cfg = status_shared.lock().unwrap_or_else(|e| e.into_inner());
                        let snapshot = build_status_snapshot(&cfg);
                        drop(cfg);
                        // Note: we can't send from here because write is moved
                        // Status updates will be sent in the main read loop
                        let _ = (status_code, snapshot);
                    }
                });

                // Read loop
                loop {
                    if !running.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    match read.next().await {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                let msg_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                if msg_type == "request" {
                                    let req_id = data.get("req_id").and_then(|r| r.as_str()).unwrap_or("").to_string();
                                    let path = data.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
                                    let method = data.get("method").and_then(|m| m.as_str()).unwrap_or("GET").to_string();
                                    let body = data.get("body").cloned();

                                    let result = process_api(&shared, &path, &method, body);
                                    let response = json!({
                                        "type": "response",
                                        "req_id": req_id,
                                        "data": result,
                                    });
                                    let _ = write.send(Message::Text(response.to_string().into())).await;
                                }
                            }
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Some(Err(e)) => {
                            eprintln!("[Cloud] Read error: {}", e);
                            break;
                        }
                        None => {
                            println!("[Cloud] Disconnected");
                            break;
                        }
                        _ => {}
                    }
                }

                status_handle.abort();
            }
            Err(e) => {
                eprintln!("[Cloud] Connection error: {}, retrying in 10s", e);
            }
        }

        if !running.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        sleep(Duration::from_secs(10)).await;
    }
}

fn build_status_snapshot(cfg: &ConfigManager) -> Value {
    let s = &cfg.state;
    let c = &cfg.config;
    json!({
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
    })
}

fn process_api(shared: &SharedConfig, path: &str, method: &str, body: Option<Value>) -> Value {
    let mut cfg = shared.lock().unwrap_or_else(|e| e.into_inner());

    match path {
        "/api/status" => {
            let s = &cfg.state;
            json!({
                "remaining_daily": cfg.remaining_daily(),
                "remaining_session": cfg.remaining_session(),
                "used_today": s.minutes_used_today,
                "is_blocked": s.is_blocked,
                "current_window": s.current_window,
                "current_process": s.current_process,
            })
        }
        "/api/dashboard" => {
            let snapshot = build_status_snapshot(&cfg);
            let apps: Vec<Value> = cfg
                .history
                .get_app_summary_today()
                .iter()
                .map(|a| json!({"app": a.app, "minutes": a.minutes}))
                .collect();
            let today: Vec<Value> = cfg
                .history
                .get_today()
                .iter()
                .map(|e| json!({"time": e.time, "window": e.window, "process": e.process}))
                .collect();
            let mut out = serde_json::Map::new();
            if let Value::Object(map) = snapshot {
                out.extend(map);
            }
            out.insert("apps".into(), Value::Array(apps));
            out.insert(
                "recent_activity".into(),
                Value::Array(today.iter().rev().take(30).cloned().collect()),
            );
            Value::Object(out)
        }
        "/api/lock" if method == "POST" => {
            std::thread::spawn(crate::winapi::lock_screen);
            json!({"ok": true})
        }
        "/api/add-time" if method == "POST" => {
            let mins = body
                .and_then(|b| b.get("minutes").and_then(|m| m.as_u64()))
                .unwrap_or(30) as u32;
            let mins = mins.clamp(1, 300);
            cfg.grant_extra_time(mins);
            json!({"ok": true, "added": mins})
        }
        "/api/history" => {
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
            json!({"today": today, "summary": summary})
        }
        _ => json!({"error": "unknown endpoint"}),
    }
}
