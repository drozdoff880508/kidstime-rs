mod app;mod config;mod history;mod relay;mod server;mod tracker;mod utils;mod web_html;mod winapi;use server::SharedConfig;use std::sync::{Arc, Mutex};fn main() -> eframe::Result<()> {
    // Shared state
    let config_mgr = config::ConfigManager::new();
    let shared: SharedConfig = Arc::new(Mutex::new(config_mgr));

    let shared_clone = shared.clone();

    // Start web server if enabled
    let web_enabled = {
        let cfg = shared_clone.lock().unwrap_or_else(|e| e.into_inner());
        cfg.config.web_enabled
    };
    if web_enabled {
        let port = {
            let cfg = shared_clone.lock().unwrap_or_else(|e| e.into_inner());
            cfg.config.web_port
        };
        let srv_shared = shared_clone.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create web server runtime");
            rt.block_on(server::run_web_server(srv_shared, port));
        });
    }

    // Start IP detection + DuckDNS loop
    let ip_shared = shared_clone.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(300));
            let ip = utils::get_public_ip();
            let (ddns_domain, ddns_token) = {
                let cfg = ip_shared.lock().unwrap_or_else(|e| e.into_inner());
                (cfg.config.duckdns_domain.clone(), cfg.config.duckdns_token.clone())
            };
            if !ddns_domain.is_empty() && !ddns_token.is_empty() && !ip.is_empty() {
                utils::update_duckdns(&ddns_domain, &ddns_token, &ip);
            }
        }
    });

    // Start cloud relay client
    let cloud_client = relay::CloudRelayClient::new();
    cloud_client.start(shared_clone.clone());

    // Start time tracking
    let is_first = {
        let cfg = shared_clone.lock().unwrap_or_else(|e| e.into_inner());
        cfg.is_first_run()
    };
    if !is_first {
        tracker::start_tracking(shared_clone.clone());
    }

    // Launch GUI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 600.0])
            .with_resizable(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "KidsTime Pro",
        options,
        Box::move |_cc| {
            Ok(Box::new(app::KidsTimeApp::new(shared)))
        }),
    )
}