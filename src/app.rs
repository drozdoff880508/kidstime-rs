use crate::config::format_time;
use crate::server::SharedConfig;
use eframe::egui;

// ── Tab enum ────────────────────────────────────────────────────────────

enum Tab {
    Dashboard,
    Settings,
    Activity,
}

// ── Snapshot structs (copy data out of MutexGuard to avoid holding lock) ──

struct SnapConfig {
    daily_limit: u32,
    session_limit: u32,
    schedule_enabled: bool,
    schedule_start: String,
    schedule_end: String,
    lock_on_limit: bool,
    web_enabled: bool,
    web_port: u16,
    track_apps: bool,
    web_token: String,
    duckdns_domain: String,
    duckdns_token: String,
    cloud_enabled: bool,
    cloud_relay_url: String,
    cloud_device_code: String,
}

struct SnapState {
    minutes_used_today: u32,
    session_minutes: u32,
    is_blocked: bool,
    current_window: String,
    current_process: String,
}

impl SnapConfig {
    fn from_mgr(mgr: &crate::config::ConfigManager) -> Self {
        let c = &mgr.config;
        Self {
            daily_limit: c.daily_limit_minutes,
            session_limit: c.session_limit_minutes,
            schedule_enabled: c.schedule_enabled,
            schedule_start: c.schedule_start.clone(),
            schedule_end: c.schedule_end.clone(),
            lock_on_limit: c.lock_on_limit,
            web_enabled: c.web_enabled,
            web_port: c.web_port,
            track_apps: c.track_apps,
            web_token: c.web_token.clone(),
            duckdns_domain: c.duckdns_domain.clone(),
            duckdns_token: c.duckdns_token.clone(),
            cloud_enabled: c.cloud_enabled,
            cloud_relay_url: c.cloud_relay_url.clone(),
            cloud_device_code: c.cloud_device_code.clone(),
        }
    }
}

impl SnapState {
    fn from_mgr(mgr: &crate::config::ConfigManager) -> Self {
        let s = &mgr.state;
        Self {
            minutes_used_today: s.minutes_used_today,
            session_minutes: s.session_minutes,
            is_blocked: s.is_blocked,
            current_window: s.current_window.clone(),
            current_process: s.current_process.clone(),
        }
    }
}

// ── App struct ──────────────────────────────────────────────────────────

pub struct KidsTimeApp {
    shared: SharedConfig,
    tab: Tab,
    // Auth
    authenticated: bool,
    pw_input: String,
    show_pw_error: bool,
    // First-run / password change
    new_pw: String,
    confirm_pw: String,
    setup_error: String,
    // Extra-time dialog
    extra_min_input: String,
    // Settings scratch fields (edited locally, applied on save)
    s_daily: String,
    s_session: String,
    s_warning: String,
    s_sched_start: String,
    s_sched_end: String,
    s_web_port: String,
    s_duckdns_domain: String,
    s_duckdns_token: String,
    s_cloud_relay: String,
    s_cloud_code: String,
    s_lock_on_limit: bool,
    s_schedule_enabled: bool,
    s_web_enabled: bool,
    s_track_apps: bool,
    s_cloud_enabled: bool,
    settings_dirty: bool,
    // Refresh clock (1 s tick)
    last_tick: std::time::Instant,
    // Status message (shown briefly)
    toast: String,
    toast_time: std::time::Instant,
}

impl KidsTimeApp {
    pub fn new(shared: SharedConfig) -> Self {
        let cfg = shared.lock().unwrap_or_else(|e| e.into_inner());
        let first = cfg.is_first_run();
        let sc = SnapConfig::from_mgr(&cfg);
        let warning = cfg.config.warning_minutes;
        drop(cfg);

        Self {
            shared,
            tab: Tab::Dashboard,
            authenticated: !first,
            pw_input: String::new(),
            show_pw_error: false,
            new_pw: String::new(),
            confirm_pw: String::new(),
            setup_error: String::new(),
            extra_min_input: "30".into(),
            s_daily: sc.daily_limit.to_string(),
            s_session: sc.session_limit.to_string(),
            s_warning: warning.to_string(),
            s_sched_start: sc.schedule_start,
            s_sched_end: sc.schedule_end,
            s_web_port: sc.web_port.to_string(),
            s_duckdns_domain: sc.duckdns_domain,
            s_duckdns_token: sc.duckdns_token,
            s_cloud_relay: sc.cloud_relay_url,
            s_cloud_code: sc.cloud_device_code,
            s_lock_on_limit: sc.lock_on_limit,
            s_schedule_enabled: sc.schedule_enabled,
            s_web_enabled: sc.web_enabled,
            s_track_apps: sc.track_apps,
            s_cloud_enabled: sc.cloud_enabled,
            settings_dirty: false,
            last_tick: std::time::Instant::now(),
            toast: String::new(),
            toast_time: std::time::Instant::now(),
        }
    }

    // ── helpers ────────────────────────────────────────────────────────

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = msg.into();
        self.toast_time = std::time::Instant::now();
    }

    fn toast_visible(&self) -> bool {
        self.toast_time.elapsed().as_secs() < 3
    }

    /// Snapshot everything we need from the shared config in one lock acquisition.
    fn snap(&self) -> (SnapConfig, SnapState, f64, f64, bool, String, String) {
        let cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let sc = SnapConfig::from_mgr(&cfg);
        let ss = SnapState::from_mgr(&cfg);
        let rd = cfg.remaining_daily();
        let rs = cfg.remaining_session();
        let sched = cfg.is_in_schedule();
        let local = cfg.get_local_url();
        let pub_url = cfg.get_public_url("");
        (sc, ss, rd, rs, sched, local, pub_url)
    }

    fn sync_scratch_from_config(&mut self) {
        let cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let c = &cfg.config;
        self.s_daily = c.daily_limit_minutes.to_string();
        self.s_session = c.session_limit_minutes.to_string();
        self.s_warning = c.warning_minutes.to_string();
        self.s_sched_start = c.schedule_start.clone();
        self.s_sched_end = c.schedule_end.clone();
        self.s_web_port = c.web_port.to_string();
        self.s_duckdns_domain = c.duckdns_domain.clone();
        self.s_duckdns_token = c.duckdns_token.clone();
        self.s_cloud_relay = c.cloud_relay_url.clone();
        self.s_cloud_code = c.cloud_device_code.clone();
        self.s_lock_on_limit = c.lock_on_limit;
        self.s_schedule_enabled = c.schedule_enabled;
        self.s_web_enabled = c.web_enabled;
        self.s_track_apps = c.track_apps;
        self.s_cloud_enabled = c.cloud_enabled;
        self.settings_dirty = false;
    }

    fn apply_scratch_to_config(&mut self) {
        let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let c = &mut cfg.config;
        c.daily_limit_minutes = self.s_daily.parse().unwrap_or(c.daily_limit_minutes);
        c.session_limit_minutes = self.s_session.parse().unwrap_or(c.session_limit_minutes);
        c.warning_minutes = self.s_warning.parse().unwrap_or(c.warning_minutes);
        c.schedule_start = self.s_sched_start.clone();
        c.schedule_end = self.s_sched_end.clone();
        c.web_port = self.s_web_port.parse().unwrap_or(c.web_port);
        c.duckdns_domain = self.s_duckdns_domain.clone();
        c.duckdns_token = self.s_duckdns_token.clone();
        c.cloud_relay_url = self.s_cloud_relay.clone();
        c.cloud_device_code = self.s_cloud_code.clone();
        c.lock_on_limit = self.s_lock_on_limit;
        c.schedule_enabled = self.s_schedule_enabled;
        c.web_enabled = self.s_web_enabled;
        c.track_apps = self.s_track_apps;
        c.cloud_enabled = self.s_cloud_enabled;
        cfg.save_config();
    }

    fn try_login(&mut self) {
        let cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if cfg.verify_password(&self.pw_input) {
            self.authenticated = true;
            self.show_pw_error = false;
            self.pw_input.clear();
        } else {
            self.show_pw_error = true;
        }
    }

    fn try_blocked_unlock(&mut self) {
        let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if cfg.verify_password(&self.pw_input) {
            cfg.grant_extra_time(5);
            self.pw_input.clear();
            self.show_pw_error = false;
        } else {
            self.show_pw_error = true;
        }
    }

    fn try_setup(&mut self) {
        if self.new_pw.len() < 4 {
            self.setup_error = "Password must be at least 4 characters.".into();
        } else if self.new_pw != self.confirm_pw {
            self.setup_error = "Passwords do not match.".into();
        } else {
            let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            cfg.set_password(&self.new_pw);
            drop(cfg);
            self.authenticated = true;
            self.setup_error.clear();
            self.new_pw.clear();
            self.confirm_pw.clear();
            // Start tracking after first-run setup
            crate::tracker::start_tracking(self.shared.clone());
        }
    }

    fn try_change_password(&mut self) {
        if self.new_pw.len() < 4 {
            self.setup_error = "Password must be at least 4 characters.".into();
        } else if self.new_pw != self.confirm_pw {
            self.setup_error = "Passwords do not match.".into();
        } else {
            let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            cfg.set_password(&self.new_pw);
            self.new_pw.clear();
            self.confirm_pw.clear();
            self.setup_error.clear();
            self.toast("Password updated.");
        }
    }

    fn regenerate_token(&mut self) -> String {
        let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        cfg.regenerate_token();
        let tok = cfg.config.web_token.clone();
        drop(cfg);
        self.toast("New token generated.");
        tok
    }

    fn grant_extra_time(&mut self) {
        let mins: u32 = self
            .extra_min_input
            .trim()
            .parse()
            .unwrap_or(30)
            .clamp(1, 300);
        {            let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            cfg.grant_extra_time(mins);
        }
        self.toast(format!("Granted {} extra minutes.", mins));
    }

    fn reset_session(&mut self) {
        {            let mut cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            cfg.reset_session();
        }
        self.toast("Session reset.");
    }

    // ── Screen builders ────────────────────────────────────────────────

    fn ui_first_run(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.20);
                ui.label(
                    egui::RichText::new("KidsTime Pro")
                        .size(32.0)
                        .strong()
                        .color(egui::Color32::from_rgb(66, 133, 244)),
                );
                ui.label(
                    egui::RichText::new("Parental Control Setup")
                        .size(16.0)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(20.0);
                ui.scope(|ui| {
                    ui.set_max_width(320.0);
                    ui.label("Create admin password:");
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_pw)
                            .password(true)
                            .hint_text("Password"),
                    );
                    ui.add_space(8.0);
                    ui.label("Confirm password:");
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.confirm_pw)
                            .password(true)
                            .hint_text("Confirm"),
                    );
                    if !self.setup_error.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(&self.setup_error)
                                .color(egui::Color32::RED)
                                .size(13.0),
                        );
                    }
                    ui.add_space(12.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new("Create Account"),
                        )
                        .clicked()
                    {
                        self.try_setup();
                    }
                });
            });
        });
    }

    fn ui_login(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.30);
                ui.label(
                    egui::RichText::new("KidsTime Pro")
                        .size(28.0)
                        .strong()
                        .color(egui::Color32::from_rgb(66, 133, 244)),
                );
                ui.add_space(16.0);
                ui.scope(|ui| {
                    ui.set_max_width(280.0);
                    ui.label("Enter admin password:");
                    ui.add_space(4.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.pw_input)
                            .password(true)
                            .hint_text("Password"),
                    );
                    resp.request_focus();
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.try_login();
                    }
                    if self.show_pw_error {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Incorrect password")
                                .color(egui::Color32::RED)
                                .size(13.0),
                        );
                    }
                    ui.add_space(8.0);
                    if ui
                        .add_sized([ui.available_width(), 32.0], egui::Button::new("Unlock"))
                        .clicked()
                    {
                        self.try_login();
                    }
                });
            });
        });
    }

    fn ui_blocked(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.15);
                ui.label(
                    egui::RichText::new("!")
                        .size(64.0)
                        .strong()
                        .color(egui::Color32::from_rgb(234, 67, 53)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Time's Up!")
                        .size(36.0)
                        .strong()
                        .color(egui::Color32::from_rgb(234, 67, 53)),
                );
                ui.label(
                    egui::RichText::new("Your screen time limit has been reached.")
                        .size(15.0)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(20.0);
                ui.scope(|ui| {
                    ui.set_max_width(280.0);
                    ui.label("Admin password to unlock:");
                    ui.add_space(4.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.pw_input)
                            .password(true)
                            .hint_text("Password"),
                    );
                    resp.request_focus();
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.try_blocked_unlock();
                    }
                    if self.show_pw_error {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Incorrect password")
                                .color(egui::Color32::RED)
                                .size(13.0),
                        );
                    }
                });
                ui.add_space(24.0);
                ui.label(
                    egui::RichText::new(
                        "Ask a parent or guardian to enter the password.\nThe screen will lock automatically.",
                    )
                    .size(13.0)
                    .color(egui::Color32::GRAY)
                    .wrap(),
                );
            });
        });
    }

    // ── Main authenticated UI ──────────────────────────────────────────

    fn ui_authenticated(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("KidsTime Pro")
                        .size(17.0)
                        .strong()
                        .color(egui::Color32::from_rgb(66, 133, 244)),
                );
                ui.separator();
                let tabs = [
                    (Tab::Dashboard, "Dashboard"),
                    (Tab::Settings, "Settings"),
                    (Tab::Activity, "Activity"),
                ];
                for (tab, label) in &tabs {
                    let selected = self.tab == *tab;
                    let btn =
                        egui::Button::new(egui::RichText::new(*label).size(14.0)).selected(selected);
                    if ui.add(btn).clicked() {
                        self.tab = *tab;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("Logout").small()).clicked() {
                        self.authenticated = false;
                        self.pw_input.clear();
                    }
                });
            });
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let (_, ss, _, _, in_schedule, _, _) = self.snap();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                if ss.is_blocked {
                    ui.label(
                        egui::RichText::new("BLOCKED")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(234, 67, 53))
                            .strong(),
                    );
                } else if !in_schedule {
                    ui.label(
                        egui::RichText::new("Outside Schedule")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(251, 188, 4))
                            .strong(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Active")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(52, 168, 83))
                            .strong(),
                    );
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "Now: {} | {}",
                        ss.current_process, ss.current_window,
                    ))
                    .size(11.0)
                    .color(egui::Color32::GRAY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let now = chrono::Local::now();
                    ui.label(
                        egui::RichText::new(now.format("%H:%M:%S").to_string())
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        });

        // Central content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Dashboard => self.ui_dashboard_tab(ui, ctx),
                Tab::Settings => self.ui_settings_tab(ui, ctx),
                Tab::Activity => self.ui_activity_tab(ui),
            }
        });

        // Toast overlay
        if self.toast_visible() {
            egui::Area::new(egui::Id::new("toast_overlay"))
                .anchor(egui::Align2::RIGHT_TOP, [10.0, 40.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_premultiplied(40, 40, 40, 220))
                        .rounding(8.0)
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&self.toast)
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            );
                        });
                });
        }

        frame.set_min_size(egui::vec2(640.0, 480.0));
    }

    // ── Dashboard tab ──────────────────────────────────────────────────

    fn ui_dashboard_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (sc, ss, rem_daily, rem_session, in_schedule, local_url, pub_url) = self.snap();
        let daily_frac = if sc.daily_limit > 0 {
            rem_daily / sc.daily_limit as f64
        } else {
            0.0
        };
        let sess_frac = if sc.session_limit > 0 {
            rem_session / sc.session_limit as f64
        } else {
            0.0
        };

        // Timer cards
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            self.timer_card(ui, "Daily Remaining", format_time(rem_daily), daily_frac);
            ui.add_space(12.0);
            self.timer_card(ui, "Session Remaining", format_time(rem_session), sess_frac);
        });

        ui.add_space(12.0);

        // Quick actions
        ui.horizontal(|ui| {
            if ui
                .add_sized([140.0, 32.0], egui::Button::new("+ Grant Time"))
                .clicked()
            {
                self.extra_time_dialog(ctx);
            }
            if ui
                .add_sized([140.0, 32.0], egui::Button::new("Lock Screen"))
                .clicked()
            {
                std::thread::spawn(crate::winapi::lock_screen);
            }
            if ui
                .add_sized([140.0, 32.0], egui::Button::new("Reset Session"))
                .clicked()
            {
                self.reset_session();
            }
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // Status details
        ui.heading("Status");
        ui.add_space(4.0);
        egui::Grid::new("dash_status_grid")
            .num_columns(2)
            .spacing([20.0, 6.0])
            .show(ui, |ui| {
                let label = |ui: &mut egui::Ui, name: &str, val: &str| {
                    ui.label(
                        egui::RichText::new(name)
                            .size(13.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.label(egui::RichText::new(val).size(13.0));
                    ui.end_row();
                };

                label(ui, "Used today:", &format!("{} min", ss.minutes_used_today));
                label(
                    ui,
                    "Session time:",
                    &format!("{} min", ss.session_minutes),
                );
                label(
                    ui,
                    "Schedule:",
                    if in_schedule {
                        "In schedule"
                    } else {
                        "Outside schedule"
                    },
                );
                if sc.schedule_enabled {
                    label(
                        ui,
                        "Allowed hours:",
                        &format!("{} - {}", sc.schedule_start, sc.schedule_end),
                    );
                }
                label(
                    ui,
                    "Block on limit:",
                    if sc.lock_on_limit { "Yes" } else { "No" },
                );
                label(ui, "Current app:", &ss.current_process);
                label(ui, "Active window:", &ss.current_window);
            });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // Web access info
        ui.heading("Web Access");
        ui.add_space(4.0);
        if sc.web_enabled {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Local:").size(13.0).color(egui::Color32::GRAY),
                );
                ui.label(egui::RichText::new(&local_url).size(13.0));
                if ui.add(egui::Button::new("Copy").small()).clicked() {
                    ui.output_mut(|o| o.copied_text = local_url.clone());
                    self.toast("Local URL copied.");
                }
            });

            if !pub_url.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Public:").size(13.0).color(egui::Color32::GRAY),
                    );
                    ui.label(egui::RichText::new(&pub_url).size(13.0));
                    if ui.add(egui::Button::new("Copy").small()).clicked() {
                        ui.output_mut(|o| o.copied_text = pub_url.clone());
                        self.toast("Public URL copied.");
                    }
                });
            }

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Token:").size(13.0).color(egui::Color32::GRAY),
                );
                ui.label(egui::RichText::new(mask_token(&sc.web_token)).size(13.0));
                if ui.add(egui::Button::new("Regenerate").small()).clicked() {
                    let tok = self.regenerate_token();
                    ui.output_mut(|o| o.copied_text = tok);
                }
            });
        } else {
            ui.label(
                egui::RichText::new("Web panel is disabled.")
                    .size(13.0)
                    .color(egui::Color32::GRAY),
            );
        }
    }

    fn timer_card(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        time_str: String,
        fraction: f64,
    ) {
        let card_width = (ui.available_width() - 12.0) / 2.0;
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(32, 33, 36))
            .rounding(10.0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_width(card_width);

                ui.label(
                    egui::RichText::new(title)
                        .size(12.0)
                        .color(egui::Color32::GRAY),
                );

                let color = if fraction > 0.5 {
                    egui::Color32::from_rgb(52, 168, 83)
                } else if fraction > 0.15 {
                    egui::Color32::from_rgb(251, 188, 4)
                } else {
                    egui::Color32::from_rgb(234, 67, 53)
                };

                ui.label(
                    egui::RichText::new(time_str)
                        .size(32.0)
                        .strong()
                        .color(color),
                );

                ui.add_space(6.0);

                let bar_height = 6.0;
                let available = ui.available_width();
                let (rect, _resp) =
                    ui.allocate_exact_size(egui::vec2(available, bar_height), egui::Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(
                    rect,
                    bar_height / 2.0,
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 20),
                );
                let fill_frac = fraction.clamp(0.0, 1.0);
                if fill_frac > 0.0 {
                    let fill_rect = egui::Rect::from_min_max(
                        rect.min,
                        egui::pos2(rect.min.x + rect.width() * fill_frac, rect.max.y),
                    );
                    painter.rect_filled(fill_rect, bar_height / 2.0, color);
                }
            });
    }

    fn extra_time_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("Grant Extra Time")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(240.0);
                ui.label("Minutes to add:");
                ui.add(egui::TextEdit::singleline(&mut self.extra_min_input).hint_text("30"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add_sized([100.0, 28.0], egui::Button::new("Grant")).clicked() {
                        self.grant_extra_time();
                        open = false;
                    }
                    if ui.add_sized([80.0, 28.0], egui::Button::new("Cancel")).clicked() {
                        open = false;
                    }
                });
            });
    }

    // ── Settings tab ───────────────────────────────────────────────────

    fn ui_settings_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Time Limits
            ui.heading("Time Limits");
            ui.add_space(4.0);
            egui::Grid::new("time_limits_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Daily limit (min):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.s_daily).desired_width(100.0),
                    );
                    ui.end_row();

                    ui.label("Session limit (min):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.s_session).desired_width(100.0),
                    );
                    ui.end_row();

                    ui.label("Warning at (min left):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.s_warning).desired_width(100.0),
                    );
                    ui.end_row();
                });

            ui.add_space(8.0);
            if ui
                .checkbox(&mut self.s_lock_on_limit, "Lock screen when limit is reached")
                .changed()
            {
                self.settings_dirty = true;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Schedule
            ui.heading("Schedule");
            ui.add_space(4.0);
            if ui
                .checkbox(&mut self.s_schedule_enabled, "Enable time schedule")
                .changed()
            {
                self.settings_dirty = true;
            }
            if self.s_schedule_enabled {
                ui.add_space(4.0);
                egui::Grid::new("sched_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Start time (HH:MM):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_sched_start)
                                .desired_width(100.0),
                        );
                        ui.end_row();
                        ui.label("End time (HH:MM):");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_sched_end)
                                .desired_width(100.0),
                        );
                        ui.end_row();
                    });
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // App tracking
            ui.heading("App Tracking");
            ui.add_space(4.0);
            if ui
                .checkbox(&mut self.s_track_apps, "Track active application usage")
                .changed()
            {
                self.settings_dirty = true;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Web panel
            ui.heading("Web Panel");
            ui.add_space(4.0);
            if ui
                .checkbox(&mut self.s_web_enabled, "Enable web panel")
                .changed()
            {
                self.settings_dirty = true;
            }
            if self.s_web_enabled {
                ui.add_space(4.0);
                egui::Grid::new("web_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Port:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_web_port)
                                .desired_width(100.0),
                        );
                        ui.end_row();
                    });
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Cloud / Remote access
            ui.heading("Remote Access");
            ui.add_space(4.0);
            if ui
                .checkbox(&mut self.s_cloud_enabled, "Enable cloud relay")
                .changed()
            {
                self.settings_dirty = true;
            }
            if self.s_cloud_enabled {
                ui.add_space(4.0);
                egui::Grid::new("cloud_grid")
                    .num_columns(1)
                    .spacing([0.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Relay URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_cloud_relay)
                                .desired_width(360.0),
                        );
                        ui.end_row();
                        ui.label("Device code:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_cloud_code)
                                .desired_width(360.0),
                        );
                        ui.end_row();
                    });
            }

            ui.add_space(4.0);
            ui.collapsing("DuckDNS (optional)", |ui| {
                egui::Grid::new("duckdns_grid")
                    .num_columns(1)
                    .spacing([0.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Domain:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_duckdns_domain)
                                .desired_width(360.0),
                        );
                        ui.end_row();
                        ui.label("Token:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.s_duckdns_token)
                                .desired_width(360.0)
                                .password(true),
                        );
                        ui.end_row();
                    });
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // Change password
            ui.collapsing("Change Admin Password", |ui| {
                ui.horizontal(|ui| {
                    ui.label("New password:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_pw)
                            .password(true)
                            .desired_width(160.0),
                    );
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Confirm:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.confirm_pw)
                            .password(true)
                            .desired_width(160.0),
                    );
                });
                if !self.setup_error.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(&self.setup_error)
                            .color(egui::Color32::RED)
                            .size(12.0),
                    );
                }
                ui.add_space(4.0);
                if ui.add(egui::Button::new("Update Password")).clicked() {
                    self.try_change_password();
                }
            });

            ui.add_space(16.0);

            // Save / Discard
            ui.horizontal(|ui| {
                if ui
                    .add_sized([120.0, 32.0], egui::Button::new("Save Settings"))
                    .clicked()
                {
                    self.apply_scratch_to_config();
                    self.settings_dirty = false;
                    self.toast("Settings saved.");
                }
                if ui
                    .add_sized([120.0, 32.0], egui::Button::new("Discard"))
                    .clicked()
                {
                    self.sync_scratch_from_config();
                    self.toast("Changes discarded.");
                }
            });

            ui.add_space(24.0);
        });
    }

    // ── Activity tab ───────────────────────────────────────────────────

    fn ui_activity_tab(&mut self, ui: &mut egui::Ui) {
        let summary: Vec<(String, u32)> = {
            let cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            cfg.history
                .get_app_summary_today()
                .into_iter()
                .map(|s| (s.app, s.minutes))
                .collect()
        };

        let today_entries: Vec<(String, String, String)> = {
            let cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            cfg.history
                .get_today()
                .iter()
                .map(|e| (e.time.clone(), e.process.clone(), e.window.clone()))
                .collect()
        };

        ui.horizontal(|ui| {
            ui.heading("Activity Log");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} entries today", today_entries.len()))
                        .size(12.0)
                        .color(egui::Color32::GRAY),
                );
            });
        });

        ui.add_space(8.0);

        // App usage summary
        ui.collapsing("App Usage Summary (Today)", |ui| {
            if summary.is_empty() {
                ui.label(
                    egui::RichText::new("No activity recorded today.")
                        .color(egui::Color32::GRAY)
                        .size(13.0),
                );
                return;
            }

            let max_min = summary.first().map(|s| s.1).unwrap_or(1).max(1);

            for (app, minutes) in &summary {
                ui.horizontal(|ui| {
                    ui.set_width(ui.available_width());
                    let bar_max_w = (ui.available_width() - 180.0).max(4.0);
                    let bar_width = (*minutes as f64 / max_min as f64) * bar_max_w;

                    ui.label(egui::RichText::new(app).size(12.0));
                    ui.add_space(8.0);

                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_width, 14.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        rect,
                        4.0,
                        egui::Color32::from_rgb(66, 133, 244),
                    );

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("{} min", minutes))
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                });
                ui.add_space(2.0);
            }
        });

        ui.add_space(12.0);

        // Recent activity table
        ui.collapsing("Recent Activity (Today)", |ui| {
            if today_entries.is_empty() {
                ui.label(
                    egui::RichText::new("No activity recorded today.")
                        .color(egui::Color32::GRAY)
                        .size(13.0),
                );
                return;
            }

            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                egui::Grid::new("activity_table")
                    .num_columns(3)
                    .spacing([12.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Time").strong().size(12.0));
                        ui.label(egui::RichText::new("Process").strong().size(12.0));
                        ui.label(egui::RichText::new("Window").strong().size(12.0));
                        ui.end_row();

                        for (time, process, window) in today_entries.iter().rev() {
                            let time_str = time
                                .split('T')
                                .nth(1)
                                .map(|t| t.split('.').next().unwrap_or(t))
                                .unwrap_or(time);

                            ui.label(
                                egui::RichText::new(time_str)
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );
                            ui.label(egui::RichText::new(process).size(11.0));
                            ui.label(
                                egui::RichText::new(window)
                                    .size(11.0)
                                    .color(egui::Color32::GRAY),
                            );
                            ui.end_row();
                        }
                    });
            });
        });
    }
}

// ── eframe::App impl ────────────────────────────────────────────────────

impl eframe::App for KidsTimeApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Tick every second to refresh display
        if self.last_tick.elapsed() >= std::time::Duration::from_secs(1) {
            self.last_tick = std::time::Instant::now();
            ctx.request_repaint();
        }

        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        let (blocked, first_run) = {
            let cfg = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            (cfg.state.is_blocked, cfg.is_first_run())
        };

        if first_run {
            self.ui_first_run(ctx);
        } else if blocked && !self.authenticated {
            self.ui_blocked(ctx);
        } else if !self.authenticated {
            self.ui_login(ctx);
        } else {
            self.ui_authenticated(ctx, frame);
        }
    }

    fn on_close_event(&mut self) -> bool {
        // Keep running -- this is a system tray app.  Return false so the
        // caller can hide the window instead of exiting.
        false
    }
}

// ── Utility ─────────────────────────────────────────────────────────────

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &token[..4], &token[token.len() - 4..])
}
