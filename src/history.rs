use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub time: String,
    pub window: String,
    pub process: String,
    pub event: String,
}

#[derive(Debug, Clone)]
pub struct ActivityHistory {
    log: VecDeque<HistoryEntry>,
    path: PathBuf,
}

impl ActivityHistory {
    pub fn new(path: PathBuf) -> Self {
        let mut history = Self {
            log: VecDeque::with_capacity(MAX_ENTRIES),
            path,
        };
        history.load();
        history
    }

    fn load(&mut self) {
        if let Ok(data) = fs::read_to_string(&self.path) {
            if let Ok(entries) = serde_json::from_str::<Vec<HistoryEntry>>(&data) {
                for entry in entries {
                    if self.log.len() < MAX_ENTRIES {
                        self.log.push_back(entry);
                    }
                }
            }
        }
    }

    fn save(&self) {
        let entries: Vec<&HistoryEntry> = self.log.iter().collect();
        if let Ok(json) = serde_json::to_string_pretty(&entries) {
            let _ = fs::write(&self.path, json);
        }
    }

    pub fn record(&mut self, window: &str, process: &str) {
        let entry = HistoryEntry {
            time: Local::now().to_rfc3339(),
            window: window.to_string(),
            process: process.to_string(),
            event: "active".to_string(),
        };
        if self.log.len() >= MAX_ENTRIES {
            self.log.pop_front();
        }
        self.log.push_back(entry);
        self.save();
    }

    pub fn get_today(&self) -> Vec<&HistoryEntry> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.log.iter().filter(|e| e.time.starts_with(&today)).collect()
    }

    pub fn get_app_summary_today(&self) -> Vec<AppSummary> {
        let entries = self.get_today();
        let mut app_time: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for e in &entries {
            *app_time.entry(e.process.clone()).or_insert(0) += 1;
        }
        let mut sorted: Vec<AppSummary> = app_time
            .into_iter()
            .map(|(app, minutes)| AppSummary { app, minutes })
            .collect();
        sorted.sort_by(|a, b| b.minutes.cmp(&a.minutes));
        sorted
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSummary {
    pub app: String,
    pub minutes: u32,
}
