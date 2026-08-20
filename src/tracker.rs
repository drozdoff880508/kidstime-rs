use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use crate::config::ConfigManager;
use crate::server::SharedConfig;
use crate::winapi;

pub fn start_tracking(shared: SharedConfig) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tracker runtime");
        rt.block_on(tracking_loop(shared));
    });
}

async fn tracking_loop(shared: SharedConfig) {
    // Start session if not started
    {
        let mut cfg = shared.lock().unwrap_or_else(|e| e.into_inner());
        if cfg.state.session_start.is_none() {
            cfg.start_session();
        }
    }

    let mut ticker = interval(Duration::from_secs(60));

    loop {
        ticker.tick().await;

        let should_lock;
        let should_break_session;

        {
            let mut cfg = shared.lock().unwrap_or_else(|e| e.into_inner());

            // Check schedule
            if !cfg.is_in_schedule() {
                cfg.state.is_blocked = true;
                cfg.save_state();
                drop(cfg);
                winapi::lock_screen();
                continue;
            }

            cfg.add_session_minute();

            let track_apps = cfg.config.track_apps;
            let daily_rem = cfg.remaining_daily();
            let session_rem = cfg.remaining_session();

            should_lock = daily_rem <= 0.0 && cfg.config.lock_on_limit;
            should_break_session = session_rem <= 0.0 && daily_rem > 0.0;

            if should_break_session {
                cfg.reset_session();
            }
            drop(cfg); // Release lock BEFORE Win32 call

            // Track active window — outside mutex to avoid blocking UI
            if track_apps {
                let info = winapi::get_active_window_info();
                if !info.window.is_empty() {
                    let mut cfg = shared.lock().unwrap_or_else(|e| e.into_inner());
                    cfg.update_active_window(&info.window, &info.process);
                    cfg.history.record(&info.window, &info.process);
                }
            }
        }

        if should_lock {
            println!("[Tracker] Daily limit reached, locking screen");
            winapi::lock_screen();
        }
    }
}
