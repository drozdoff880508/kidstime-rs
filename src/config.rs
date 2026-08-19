use chrono::Local;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub admin_password_hash: String,
    #[serde(default = "default_daily_limit")]
    pub daily_limit_minutes: u32,
    #[serde(default = "default_session_limit")]
    pub session_limit_minutes: u32,
    #[serde(default = "default_true")]
    pub schedule_enabled: bool,
    #[serde(default = "default_sched_start")]
    pub schedule_start: String,
    #[serde(default = "default_sched_end")]
    pub schedule_end: String,
    #[serde(default = "default_warning")]
    pub warning_minutes: u32,
    #[serde(default = "default_true")]
    pub lock_on_limit: bool,
    #[serde(default = "default_true")]
    pub web_enabled: bool,
    #[serde(default = "default_port")]
    pub web_port: u16,
    #[serde(default = "default_true")]
    pub track_apps: bool,
    #[serde(default)]
    pub web_token: String,
    #[serde(default)]
    pub duckdns_domain: String,
    #[serde(default)]
    pub duckdns_token: String,
    #[serde(default)]
    pub cloud_enabled: bool,
    #[serde(default)]
    pub cloud_relay_url: String,
    #[serde(default)]
    pub cloud_device_code: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimerState {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub minutes_used_today: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_start: Option<String>,
    #[serde(default)]
    pub session_minutes: u32,
    #[serde(default)]
    pub is_blocked: bool,
    #[serde(default)]
    pub current_window: String,
    #[serde(default)]
    pub current_process: String,
}

fn default_daily_limit() -> u32 { 120 }
fn default_session_limit() -> u32 { 60 }
fn default_true() -> bool { true }
fn default_sched_start() -> String { "09:00".to_string() }
fn default_sched_end() -> String { "21:00".to_string() }
fn default_warning() -> u32 { 5 }
fn default_port() -> u16 { 8080 }

impl Default for Config {
    fn default() -> Self {
        Self {
            admin_password_hash: String::new(),
            daily_limit_minutes: default_daily_limit(),
            session_limit_minutes: default_session_limit(),
            schedule_enabled: true,
            schedule_start: default_sched_start(),
            schedule_end: default_sched_end(),
            warning_minutes: default_warning(),
            lock_on_limit: true,
            web_enabled: true,
            web_port: default_port(),
            track_apps: true,
            web_token: String::new(),
            duckdns_domain: String::new(),
            duckdns_token: String::new(),
            cloud_enabled: false,
            cloud_relay_url: String::new(),
            cloud_device_code: String::new(),
        }
    }
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            date: String::new(),
            minutes_used_today: 0,
            session_start: None,
            session_minutes: 0,
            is_blocked: false,
            current_window: String::new(),
            current_process: String::new(),
        }
    }
}

pub struct ConfigManager {
    pub config: Config,
    pub state: TimerState,
    pub history: crate::history::ActivityHistory,
    base_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let base_dir = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."))
            .parent()
            .unwrap_or(PathBuf::from("."))
            .to_path_buf();

        let config_path = base_dir.join("kidstime_config.json");
        let state_path = base_dir.join("kidstime_state.json");

        let config: Config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let mut state: TimerState = fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let history = crate::history::ActivityHistory::new(
            base_dir.join("kidstime_history.json"),
        );

        let mut mgr = Self {
            config,
            state,
            history,
            base_dir,
        };

        // Generate web token if missing
        if mgr.config.web_token.is_empty() {
            mgr.config.web_token = generate_token();
            mgr.save_config();
        }

        // Check daily reset
        let today = Local::now().format("%Y-%m-%d").to_string();
        if mgr.state.date != today {
            mgr.state.date = today;
            mgr.state.minutes_used_today = 0;
            mgr.state.session_minutes = 0;
            mgr.state.is_blocked = false;
            mgr.state.session_start = None;
            mgr.save_state();
        }

        mgr
    }

    pub fn save_config(&self) {
        let path = self.base_dir.join("kidstime_config.json");
        let _ = fs::write(&path, serde_json::to_string_pretty(&self.config).unwrap_or_default());
    }

    pub fn save_state(&self) {
        let path = self.base_dir.join("kidstime_state.json");
        let _ = fs::write(&path, serde_json::to_string_pretty(&self.state).unwrap_or_default());
    }

    pub fn is_first_run(&self) -> bool {
        self.config.admin_password_hash.is_empty()
    }

    pub fn verify_password(&self, password: &str) -> bool {
        hash_password(password) == self.config.admin_password_hash
    }

    pub fn verify_token(&self, token: &str) -> bool {
        token == self.config.web_token
    }

    pub fn set_password(&mut self, password: &str) {
        self.config.admin_password_hash = hash_password(password);
        self.save_config();
    }

    pub fn remaining_daily(&self) -> f64 {
        self.config.daily_limit_minutes as f64
            - self.state.minutes_used_today as f64
    }

    pub fn remaining_session(&self) -> f64 {
        self.config.session_limit_minutes as f64
            - self.state.session_minutes as f64
    }

    pub fn is_in_schedule(&self) -> bool {
        if !self.config.schedule_enabled {
            return true;
        }
        let now = Local::now().time();
        let start = parse_time(&self.config.schedule_start);
        let end = parse_time(&self.config.schedule_end);
        if start <= end {
            now >= start && now <= end
        } else {
            now >= start || now <= end
        }
    }

    pub fn add_session_minute(&mut self) {
        self.state.session_minutes += 1;
        self.state.minutes_used_today += 1;
        self.save_state();
    }

    pub fn start_session(&mut self) {
        self.state.session_start = Some(Local::now().to_rfc3339());
        self.state.session_minutes = 0;
        self.save_state();
    }

    pub fn reset_session(&mut self) {
        self.state.session_minutes = 0;
        self.state.session_start = Some(Local::now().to_rfc3339());
        self.save_state();
    }

    pub fn grant_extra_time(&mut self, minutes: u32) {
        self.state.minutes_used_today =
            self.state.minutes_used_today.saturating_sub(minutes);
        self.state.is_blocked = false;
        self.save_state();
    }

    pub fn update_active_window(&mut self, window: &str, process: &str) {
        self.state.current_window = window.to_string();
        self.state.current_process = process.to_string();
        self.save_state();
    }

    pub fn get_local_url(&self) -> String {
        let ip = crate::utils::get_local_ip();
        format!("http://{}:{}", ip, self.config.web_port)
    }

    pub fn get_public_url(&self, public_ip: &str) -> String {
        let port = self.config.web_port;
        if self.config.cloud_enabled {
            let url = self.config.cloud_relay_url.trim_end_matches('/');
            let code = &self.config.cloud_device_code;
            if !url.is_empty() && !code.is_empty() {
                return format!("{}/{}", url, code);
            }
        }
        let ddns = self.config.duckdns_domain.trim();
        if !ddns.is_empty() {
            let domain = if ddns.contains('.') {
                ddns.to_string()
            } else {
                format!("{}.duckdns.org", ddns)
            };
            return format!("http://{}:{}", domain, port);
        }
        if !public_ip.is_empty() {
            return format!("http://{}:{}", public_ip, port);
        }
        String::new()
    }

    pub fn regenerate_token(&mut self) {
        self.config.web_token = generate_token();
        self.save_config();
    }
}

pub fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"KidsTime2024");
    hasher.update(password.as_bytes());
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn generate_token() -> String {
    Uuid::new_v4().to_simple().to_string()
}

pub fn format_time(minutes: f64) -> String {
    let total = minutes as i64;
    let h = total / 60;
    let m = total % 60;
    format!("{:02}:{:02}", h, m)
}

fn parse_time(s: &str) -> chrono::NaiveTime {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let h: u32 = parts[0].parse().unwrap_or(0);
        let m: u32 = parts[1].parse().unwrap_or(0);
        chrono::NaiveTime::from_hms_opt(h, m).unwrap_or(chrono::NaiveTime::from_hms_opt(0, 0).unwrap())
    } else {
        chrono::NaiveTime::from_hms_opt(0, 0).unwrap()
    }
}
