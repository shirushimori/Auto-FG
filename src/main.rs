use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;

use eframe::egui::{self, Color32, ProgressBar, RichText, ScrollArea, TextEdit};

use fitgirl_auto_downloader_mori::{download_all, scrape_links, write_input_file, DownloadItem, DownloadStatus};

mod extractor;

static PART_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\.part(\d+)").unwrap()
});

const TECH_STACK: &[&str] = &[
    "eframe / egui",
    "reqwest",
    "scraper",
    "wry",
    "regex",
    "zip",
    "unrar",
    "dialoguer",
    "colored",
];

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([700.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Fitgirl Easy Downloader",
        options,
        Box::new(|_cc| Ok(Box::new(FitgirlApp::default()))),
    )
}

#[derive(PartialEq)]
enum Tab {
    Links,
    Downloads,
    Extractor,
    About,
}

#[derive(Clone, PartialEq)]
enum LinkCategory {
    Part(u32),
    Optional,
    Other,
}

struct LinkEntry {
    url: String,
    selected: bool,
    category: LinkCategory,
}

struct FitgirlApp {
    tab: Tab,
    scrape_url: String,
    manual_text: String,
    links: Vec<LinkEntry>,
    status_text: String,
    scraping: bool,
    saved_to_file: bool,
    download_items: Arc<Mutex<Vec<DownloadItem>>>,
    download_game_name: String,
    downloading: bool,
    download_done: bool,
    paused: bool,
    pause_flag: Arc<AtomicBool>,
    sidebar_width: f32,
    collapse_url: bool,
    collapse_manual: bool,
    collapse_parts: bool,
    collapse_optionals: bool,
    links_version: usize,
    cached_part_idx: Vec<usize>,
    cached_opt_idx: Vec<usize>,
    cached_other_idx: Vec<usize>,
    extractor_items: Arc<Mutex<Vec<extractor::ArchiveItem>>>,
    extracting: bool,
    extraction_done: bool,
    extraction_cancel: Arc<AtomicBool>,
    download_dir: String,
    games_found: Vec<extractor::GameFolder>,
}

impl Default for FitgirlApp {
    fn default() -> Self {
        let saved_links = load_input_file();
        let name = extract_game_name(&saved_links);
        let links = categorize_entries(saved_links);
        let (p, o, x) = build_categories(&links);
        let has_links = !links.is_empty();
        Self {
            tab: Tab::Links,
            scrape_url: String::new(),
            manual_text: String::new(),
            links,
            status_text: if has_links {
                "Links loaded from input.txt".into()
            } else {
                "Load links from a URL or paste them manually.".into()
            },
            scraping: false,
            saved_to_file: false,
            download_items: Arc::new(Mutex::new(Vec::new())),
            download_game_name: name,
            downloading: false,
            download_done: false,
            paused: false,
            pause_flag: Arc::new(AtomicBool::new(false)),
            sidebar_width: 320.0,
            collapse_url: false,
            collapse_manual: true,
            collapse_parts: false,
            collapse_optionals: false,
            links_version: 1,
            cached_part_idx: p,
            cached_opt_idx: o,
            cached_other_idx: x,
            extractor_items: Arc::new(Mutex::new(Vec::new())),
            extracting: false,
            extraction_done: false,
            extraction_cancel: Arc::new(AtomicBool::new(false)),
            download_dir: String::new(),
            games_found: Vec::new(),
        }
    }
}

impl FitgirlApp {
    fn refresh_indices(&mut self) {
        let (p, o, x) = build_categories(&self.links);
        self.cached_part_idx = p;
        self.cached_opt_idx = o;
        self.cached_other_idx = x;
    }

    fn bump_links(&mut self) {
        self.links_version += 1;
        self.saved_to_file = false;
        self.refresh_indices();
    }
}

impl eframe::App for FitgirlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Links, "📋  Get Links");
                ui.selectable_value(&mut self.tab, Tab::Downloads, "⬇  Downloads");
                ui.selectable_value(&mut self.tab, Tab::Extractor, "📦  Extractor");
                ui.selectable_value(&mut self.tab, Tab::About, "ℹ  About");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("🎮 Games").size(14.0))
                                .fill(Color32::from_rgb(60, 80, 120))
                                .min_size(egui::vec2(100.0, 26.0)),
                        )
                        .clicked()
                    {
                        let _ = open::that("https://fitgirl-repacks.site/pop-repacks/");
                    }
                });
            });
        });

        match self.tab {
            Tab::Links => {
                egui::SidePanel::left("links_sidebar")
                    .resizable(true)
                    .default_width(self.sidebar_width)
                    .width_range(220.0..=500.0)
                    .show(ctx, |ui| self.show_sidebar(ui, ctx));
                egui::CentralPanel::default().show(ctx, |ui| self.show_links_details(ui));
            }
            Tab::Downloads => {
                egui::CentralPanel::default().show(ctx, |ui| self.show_downloads_tab(ui, ctx));
            }
            Tab::Extractor => {
                egui::CentralPanel::default().show(ctx, |ui| self.show_extractor_tab(ui, ctx));
            }
            Tab::About => {
                egui::CentralPanel::default().show(ctx, |ui| self.show_about_tab(ui));
            }
        }
    }
}

impl FitgirlApp {
    // ── sidebar ──────────────────────────────────────────────

    fn show_sidebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Link Management");
        ui.separator();
        ui.add_space(6.0);

        self.collapse_url = egui::CollapsingHeader::new("🌐  Scrape from URL")
            .default_open(!self.collapse_url)
            .show(ui, |ui| self.ui_scrape_section(ui, ctx))
            .header_response
            .clicked();
        ui.add_space(4.0);

        self.collapse_manual = egui::CollapsingHeader::new("✏️  Manual Paste")
            .default_open(!self.collapse_manual)
            .show(ui, |ui| self.ui_manual_section(ui))
            .header_response
            .clicked();
        ui.add_space(4.0);

        ui.separator();
        self.ui_link_list(ui);
    }

    fn ui_scrape_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add(
            TextEdit::singleline(&mut self.scrape_url)
                .desired_width(f32::INFINITY)
                .hint_text("https://fitgirl-repacks.site/…"),
        );
        let scrape_btn = ui.add_enabled(
            !self.scraping && !self.scrape_url.trim().is_empty(),
            egui::Button::new("Scrape"),
        );
        if scrape_btn.clicked() {
            let url = self.scrape_url.trim().to_string();
            self.scraping = true;
            self.saved_to_file = false;
            let ctx_clone = ctx.clone();
            thread::spawn(move || {
                let results = scrape_links(&url);
                ctx_clone.data_mut(|d| {
                    d.insert_temp(egui::Id::new("scrape"), ScrapeResult(results));
                });
            });
        }

        if let Some(result) =
            ctx.data_mut(|d| d.remove_temp::<ScrapeResult>(egui::Id::new("scrape")))
        {
            self.scraping = false;
            if result.0.is_empty() {
                self.status_text = "No fuckingfast.co links found at that URL.".into();
            } else {
                self.links = categorize_entries(result.0);
                self.status_text =
                    format!("Found {} links.", self.links.len());
                self.bump_links();
            }
        }

        if self.scraping {
            ui.label(RichText::new("Scraping…").size(11.0).color(Color32::YELLOW));
        }
    }

    fn ui_manual_section(&mut self, ui: &mut egui::Ui) {
        ui.add(
            TextEdit::multiline(&mut self.manual_text)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("https://fuckingfast.co/…"),
        );
        if ui.button("Add Links").clicked() {
            let new: Vec<String> = self
                .manual_text
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !new.is_empty() {
                    let existing: Vec<String> = self.links.iter().map(|e| e.url.clone()).collect();
                    for url in new {
                        if !existing.contains(&url) {
                            self.links.push(LinkEntry {
                                category: classify_link(&url),
                                url,
                                selected: true,
                            });
                        }
                    }
                self.manual_text.clear();
                self.status_text = format!("{} links loaded.", self.links.len());
                self.bump_links();
            }
        }
    }

    // ── link list (categorized) ──────────────────────────────

    fn ui_link_list(&mut self, ui: &mut egui::Ui) {
        let total = self.links.len();
        if total == 0 {
            ui.label(RichText::new("No links yet.").size(12.0).color(Color32::GRAY));
            return;
        }

        let part_entries = &self.cached_part_idx;
        let opt_entries = &self.cached_opt_idx;
        let other_entries = &self.cached_other_idx;

        ui.horizontal(|ui| {
            ui.label(format!("{} links", total));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("None").clicked() {
                    for e in &mut self.links {
                        e.selected = false;
                    }
                }
                if ui.button("All").clicked() {
                    for e in &mut self.links {
                        e.selected = true;
                    }
                }
            });
        });
        ui.add_space(4.0);

        ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                if !part_entries.is_empty() {
                    let all_part_sel = part_entries.iter().all(|&i| self.links[i].selected);
                    egui::CollapsingHeader::new(format!(
                        "📦  Part Files ({})",
                        part_entries.len()
                    ))
                    .default_open(!self.collapse_parts)
                    .show(ui, |ui| {
                        let mut tmp = all_part_sel;
                        if ui.checkbox(&mut tmp, "select all").clicked() {
                            for &i in part_entries {
                                self.links[i].selected = tmp;
                            }
                        }
                        for &i in part_entries {
                            let name = display_name(&self.links[i].url);
                            ui.checkbox(&mut self.links[i].selected, name);
                        }
                    });
                    ui.add_space(2.0);
                }

                if !opt_entries.is_empty() {
                    let all_opt_sel = opt_entries.iter().all(|&i| self.links[i].selected);
                    egui::CollapsingHeader::new(format!(
                        "🎵  Optional Files ({})",
                        opt_entries.len()
                    ))
                    .default_open(!self.collapse_optionals)
                    .show(ui, |ui| {
                        let mut tmp = all_opt_sel;
                        if ui.checkbox(&mut tmp, "select all").clicked() {
                            for &i in opt_entries {
                                self.links[i].selected = tmp;
                            }
                        }
                        for &i in opt_entries {
                            let name = display_name(&self.links[i].url);
                            ui.checkbox(&mut self.links[i].selected, name);
                        }
                    });
                    ui.add_space(2.0);
                }

                if !other_entries.is_empty() {
                    egui::CollapsingHeader::new(format!("📄  Other ({})", other_entries.len()))
                        .default_open(true)
                        .show(ui, |ui| {
                            for &i in other_entries {
                                let name = display_name(&self.links[i].url);
                                ui.checkbox(&mut self.links[i].selected, name);
                            }
                        });
                }
            });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        let selected_count = self.links.iter().filter(|e| e.selected).count();
        ui.horizontal(|ui| {
            ui.label(format!("{selected_count} selected"));
            let save_btn = ui.add_enabled(
                selected_count > 0 && !self.saved_to_file,
                egui::Button::new("Save to input.txt"),
            );
            if save_btn.clicked() {
                let selected: Vec<String> = self
                    .links
                    .iter()
                    .filter(|e| e.selected)
                    .map(|e| e.url.clone())
                    .collect();
                write_input_file(&selected);
                self.saved_to_file = true;
                self.status_text = format!("Saved {} links to input.txt", selected.len());
                self.download_items = Arc::new(Mutex::new(
                    selected
                        .iter()
                        .map(|u| DownloadItem {
                            link: u.clone(),
                            file_name: String::new(),
                            status: DownloadStatus::Pending,
                        })
                        .collect(),
                ));
                self.download_game_name = extract_game_name(&selected);
                self.downloading = false;
                self.download_done = false;
                self.tab = Tab::Downloads;
            }
        });

        ui.add_space(4.0);
        ui.label(
            RichText::new(&self.status_text)
                .size(11.0)
                .color(Color32::GRAY),
        );
    }

    // ── central: links details ───────────────────────────────

    fn show_links_details(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(16, 10))
            .show(ui, |ui| {
                ui.heading("Selected Links");
                ui.separator();
                ui.add_space(6.0);

                let selected: Vec<&LinkEntry> =
                    self.links.iter().filter(|e| e.selected).collect();

                if selected.is_empty() {
                    ui.label(
                        RichText::new("No links selected — check boxes in the sidebar.")
                            .size(13.0)
                            .color(Color32::GRAY),
                    );
                    return;
                }

                ui.label(format!(
                    "{} link{} selected.",
                    selected.len(),
                    if selected.len() == 1 { "" } else { "s" }
                ));
                ui.add_space(4.0);

                let parts: Vec<&&LinkEntry> =
                    selected.iter().filter(|e| matches!(e.category, LinkCategory::Part(_))).collect();
                let opts: Vec<&&LinkEntry> =
                    selected.iter().filter(|e| matches!(e.category, LinkCategory::Optional)).collect();
                let others: Vec<&&LinkEntry> =
                    selected.iter().filter(|e| matches!(e.category, LinkCategory::Other)).collect();

                ScrollArea::vertical()
                    .max_height(ui.available_height() - 10.0)
                    .show(ui, |ui| {
                        if !parts.is_empty() {
                            ui.label(RichText::new(format!("📦  Part Files  ({})", parts.len())).strong());
                            for e in &parts {
                                ui.label(display_name(&e.url));
                            }
                            ui.add_space(4.0);
                        }
                        if !opts.is_empty() {
                            ui.label(RichText::new(format!("🎵  Optional Files  ({})", opts.len())).strong());
                            for e in &opts {
                                ui.label(display_name(&e.url));
                            }
                            ui.add_space(4.0);
                        }
                        if !others.is_empty() {
                            ui.label(RichText::new(format!("📄  Other  ({})", others.len())).strong());
                            for e in &others {
                                ui.label(display_name(&e.url));
                            }
                        }
                    });
            });
    }

    // ── downloads tab ────────────────────────────────────────

    fn show_downloads_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.download_items.lock().unwrap().is_empty() {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    ui.heading("Downloads");
                    ui.separator();
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("📭 No links saved").size(18.0).color(Color32::GRAY));
                        ui.label(RichText::new("Go to the Get Links tab, select your links, and click \"Save to input.txt\".").size(13.0).color(Color32::DARK_GRAY));
                    });
                });
            return;
        }
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(16, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Downloads");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.downloading && !self.paused {
                            let pause_btn = ui.add(
                                egui::Button::new(RichText::new("⏸  Pause").size(18.0))
                                    .min_size(egui::vec2(120.0, 40.0))
                            );
                            if pause_btn.clicked() {
                                self.paused = true;
                                self.pause_flag.store(true, Ordering::Relaxed);
                            }
                        } else if self.downloading && self.paused {
                            let resume_btn = ui.add(
                                egui::Button::new(RichText::new("▶  Resume").size(18.0))
                                    .min_size(egui::vec2(120.0, 40.0))
                            );
                            if resume_btn.clicked() {
                                self.paused = false;
                                self.pause_flag.store(false, Ordering::Relaxed);
                            }
                        } else {
                            let q_btn = ui.add(
                                egui::Button::new(RichText::new("Download it").size(18.0))
                                    .min_size(egui::vec2(120.0, 40.0))
                            );
                            if q_btn.clicked() {
                                let links: Vec<String> = {
                                    let guard = self.download_items.lock().unwrap();
                                    guard.iter().map(|i| i.link.clone()).collect()
                                };
                                if !links.is_empty() && !self.downloading {
                                    let items = self.download_items.clone();
                                    let game_name = self.download_game_name.clone();
                                    let pause_flag = self.pause_flag.clone();
                                    self.downloading = true;
                                    self.download_done = false;
                                    self.paused = false;
                                    self.pause_flag.store(false, Ordering::Relaxed);
                                    thread::spawn(move || {
                                        download_all(links, game_name, items, pause_flag);
                                    });
                                }
                            }
                        }
                        if self.downloading && self.paused {
                            ui.label(RichText::new("⏸ Paused").color(Color32::YELLOW).size(11.0));
                        } else if self.downloading {
                            ui.label(RichText::new("▶ Downloading…").color(Color32::YELLOW).size(11.0));
                        } else if self.download_done {
                            ui.label(RichText::new("✅ Done").color(Color32::GREEN).size(11.0));
                        } else {
                            ui.label(RichText::new("⏸ Idle").color(Color32::GRAY).size(11.0));
                        }
                    });
                });
                ui.separator();
                ui.add_space(6.0);

                let guard = self.download_items.lock().unwrap();
                let total = guard.len();

                if total > 0 {
                    let done = guard
                        .iter()
                        .filter(|i| matches!(i.status, DownloadStatus::Done))
                        .count();
                    let failed = guard
                        .iter()
                        .filter(|i| matches!(i.status, DownloadStatus::Failed(_)))
                        .count();

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Game: {}",
                                if self.download_game_name.is_empty() {
                                    "—"
                                } else {
                                    &self.download_game_name
                                }
                            ))
                            .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("Done: {done}/{total}  Failed: {failed}"));
                        });
                    });
                    ui.add_space(8.0);

                    let mut part_items = Vec::new();
                    let mut opt_items = Vec::new();
                    for (i, item) in guard.iter().enumerate() {
                        match classify_link(&item.link) {
                            LinkCategory::Part(_) => part_items.push(i),
                            LinkCategory::Optional => opt_items.push(i),
                            _ => {}
                        }
                    }
                    drop(guard);

                    ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            if !part_items.is_empty() {
                                ui.label(
                                    RichText::new(format!("📦  Part Files  ({})", part_items.len()))
                                        .strong(),
                                );
                                let mut guard = self.download_items.lock().unwrap();
                                for &i in &part_items {
                                    Self::draw_download_item(ui, &mut guard[i]);
                                }
                                drop(guard);
                                ui.add_space(4.0);
                            }
                            if !opt_items.is_empty() {
                                ui.label(
                                    RichText::new(format!(
                                        "🎵  Optional Files  ({})",
                                        opt_items.len()
                                    ))
                                    .strong(),
                                );
                                let mut guard = self.download_items.lock().unwrap();
                                for &i in &opt_items {
                                    Self::draw_download_item(ui, &mut guard[i]);
                                }
                                drop(guard);
                                ui.add_space(4.0);
                            }
                        });
                } else {
                    ui.label(
                        "No links loaded. Go to the Links tab and save a link list first.",
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                let links: Vec<String> = {
                    let guard = self.download_items.lock().unwrap();
                    guard.iter().map(|i| i.link.clone()).collect()
                };
                let can_start = !links.is_empty() && !self.downloading;
                ui.horizontal(|ui| {
                    let start_btn = ui.add_enabled(can_start, egui::Button::new("▶  Start Download"));
                    if start_btn.clicked() {
                        let items = self.download_items.clone();
                        let game_name = self.download_game_name.clone();
                        let pause_flag = self.pause_flag.clone();
                        self.downloading = true;
                        self.download_done = false;
                        self.paused = false;
                        self.pause_flag.store(false, Ordering::Relaxed);
                        thread::spawn(move || {
                            download_all(links, game_name, items, pause_flag);
                        });
                    }
                    if self.downloading {
                        let _ = ui.button("⏹  Stop (close app)");
                    }
                    if self.download_done {
                        ui.label(RichText::new("✅  All downloads finished!").color(Color32::GREEN));
                    }
                });

                if self.downloading {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(if self.paused { "⏸ Paused" } else { "▶ Downloading…" })
                            .size(12.0)
                            .color(Color32::YELLOW),
                    );
                }
            });

        if self.downloading {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            let (all_done, has_done) = {
                let guard = self.download_items.lock().unwrap();
                let all_done = guard.iter().all(|i| matches!(i.status, DownloadStatus::Done | DownloadStatus::Failed(_)));
                let has_done = guard.iter().any(|i| matches!(i.status, DownloadStatus::Done));
                (all_done, has_done)
            };

            // remove completed items from the live list so the UI only shows remaining/failed
            if has_done {
                let mut guard = self.download_items.lock().unwrap();
                guard.retain(|i| !matches!(i.status, DownloadStatus::Done));
            }

            if all_done {
                self.downloading = false;
                self.download_done = true;
                let dir = format!("downloads/{}", self.download_game_name);
                self.download_dir = dir.clone();
                self.games_found = extractor::search_games();
                let items = extractor::scan_archives(std::path::Path::new(&dir));
                *self.extractor_items.lock().unwrap() = items;
            }
        }

        if self.extracting {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            let guard = self.extractor_items.lock().unwrap();
            let all_done = guard.iter().all(|a| matches!(a.status, extractor::ExtractStatus::Done | extractor::ExtractStatus::Failed(_)));
            if all_done {
                self.extracting = false;
                self.extraction_done = true;
            }
        }
    }

    fn show_extractor_tab(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(16, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("📦  Extractor");
                    if ui.button("🔍 Search").clicked() {
                        self.games_found = extractor::search_games();
                        self.extractor_items.lock().unwrap().clear();
                    }
                });
                ui.separator();
                ui.add_space(8.0);

                if self.games_found.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(
                            "Press Search to find game download folders with archives.",
                        )
                        .size(13.0)
                        .color(Color32::GRAY),
                    );
                    return;
                }

                ui.label(RichText::new(format!("Games found: {}", self.games_found.len())).strong());
                ui.add_space(6.0);
                let scroll_h = ui.available_height() - 160.0;
                egui::ScrollArea::vertical()
                    .max_height(scroll_h.max(80.0))
                    .show(ui, |ui| {
                        for game in &self.games_found {
                            let frame = egui::Frame::NONE
                                .fill(if self.download_dir == game.path.to_str().unwrap_or("") {
                                    Color32::from_rgb(40, 60, 80)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .corner_radius(egui::CornerRadius::same(4));
                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut self.download_dir,
                                        game.path.to_str().unwrap_or("").to_string(),
                                        &game.name,
                                    );
                                    if ui.small_button("📂").clicked() {
                                        let _ = open::that(&game.path);
                                    }
                                });
                                let n_archives = game.archives.len();
                                ui.label(
                                    RichText::new(format!("  {n_archives} archive(s)"))
                                        .size(11.0)
                                        .color(Color32::GRAY),
                                );
                            });
                        }
                    });

                // ── selected game archives ─────────────────────
                let selected = self.games_found.iter().find(|g| {
                    g.path.to_str().map(|s| s.to_string()) == Some(self.download_dir.clone())
                });

                if let Some(game) = selected {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.strong(format!("{} — Archives", game.name));
                        if ui.button("📂 Open Folder").clicked() {
                            let _ = open::that(&game.path);
                        }
                        if ui.button("🔄 Rescan").clicked() {
                            let items = extractor::scan_archives(&game.path);
                            *self.extractor_items.lock().unwrap() = items;
                            self.download_dir = game.path.to_str().unwrap_or("").to_string();
                        }
                    });

                    // If we just selected a game but haven't loaded its archives, load them
                    if self.extractor_items.lock().unwrap().is_empty() {
                        let items = extractor::scan_archives(&game.path);
                        *self.extractor_items.lock().unwrap() = items;
                        self.download_dir = game.path.to_str().unwrap_or("").to_string();
                    }

                    ui.add_space(4.0);
                    let items = self.extractor_items.lock().unwrap();
                    let total = items.len();
                    if total > 0 {
                        for item in items.iter() {
                            let (icon, color) = extract_status_icon(&item.status);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).color(color));
                                ui.label(&item.name);
                                if let extractor::ExtractStatus::Failed(ref e) = item.status {
                                    ui.label(RichText::new(e).color(Color32::RED).size(11.0));
                                }
                            });
                        }
                    }
                    drop(items);

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let can_extract = {
                            let guard = self.extractor_items.lock().unwrap();
                            !guard.is_empty()
                                && !self.extracting
                                && guard
                                    .iter()
                                    .any(|a| a.status == extractor::ExtractStatus::Pending)
                        };
                        let btn = ui.add_enabled(
                            can_extract,
                            egui::Button::new(RichText::new("▶  Extract All").size(16.0))
                                .min_size(egui::vec2(160.0, 36.0)),
                        );
                        if btn.clicked() {
                            let items = self.extractor_items.clone();
                            let cancel = self.extraction_cancel.clone();
                            self.extracting = true;
                            self.extraction_done = false;
                            self.extraction_cancel.store(false, Ordering::Relaxed);
                            thread::spawn(move || {
                                loop {
                                    let idx = {
                                        let guard = items.lock().unwrap();
                                        guard.iter().position(|a| a.status == extractor::ExtractStatus::Pending)
                                    };
                                    let idx = match idx {
                                        Some(i) => i,
                                        None => break,
                                    };
                                    if cancel.load(Ordering::Relaxed) {
                                        return;
                                    }
                                    let archive_path = {
                                        let guard = items.lock().unwrap();
                                        guard[idx].path.clone()
                                    };
                                    {
                                        let mut guard = items.lock().unwrap();
                                        guard[idx].status = extractor::ExtractStatus::Extracting;
                                    }
                                    extractor::extract_archive(&archive_path, &items, idx, &cancel);
                                }
                            });
                        }

                        if self.extracting {
                            if ui.button("⏹  Cancel").clicked() {
                                self.extraction_cancel.store(true, Ordering::Relaxed);
                            }
                        }

                        if self.extraction_done {
                            ui.label(
                                RichText::new("✅  Extraction complete!").color(Color32::GREEN),
                            );
                        }
                    });

                    if self.extracting {
                        ui.add_space(4.0);
                        let done_count = {
                            let guard = self.extractor_items.lock().unwrap();
                            guard
                                .iter()
                                .filter(|a| matches!(a.status, extractor::ExtractStatus::Done))
                                .count()
                        };
                        let total = {
                            let guard = self.extractor_items.lock().unwrap();
                            guard.len()
                        };
                        ui.label(
                            RichText::new(format!("⏳ Extracting... {done_count}/{total} done"))
                                .size(12.0)
                                .color(Color32::YELLOW),
                        );
                    }
                }
            });
    }

    fn show_about_tab(&mut self, ui: &mut egui::Ui) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(24, 16))
            .show(ui, |ui| {
                ui.heading(RichText::new("About Auto-FG").size(22.0));
                ui.separator();
                ui.add_space(16.0);

                ui.label(RichText::new("Developed By:").strong().size(14.0));
                ui.label(RichText::new("Shirushi Mori").size(14.0));
                ui.add_space(12.0);

                ui.label(RichText::new("State While Making:").strong().size(14.0));
                ui.label(RichText::new("Learning").size(14.0));
                ui.add_space(12.0);

                ui.label(RichText::new("Forked of and changed from:").strong().size(14.0));
                ui.hyperlink_to(
                    "https://github.com/JoyNath1337/Fitgirl-Easy-Downloader",
                    "https://github.com/JoyNath1337/Fitgirl-Easy-Downloader",
                );
                ui.add_space(12.0);

                ui.label(RichText::new("Technology Used:").strong().size(14.0));
                ui.add_space(4.0);
                for tech in TECH_STACK {
                    ui.label(RichText::new(format!("  • {}", tech)).size(13.0));
                }
                ui.add_space(16.0);

                ui.separator();
                ui.add_space(8.0);
                ui.label(
                    RichText::new("This tool automates downloading and extracting FitGirl repacks.")
                        .size(12.0)
                        .color(Color32::GRAY),
                );
                ui.label(
                    RichText::new("For educational purposes only.")
                        .size(12.0)
                        .color(Color32::GRAY),
                );
            });
    }

    fn draw_download_item(ui: &mut egui::Ui, item: &mut DownloadItem) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon, color) = status_icon(&item.status);
                    ui.label(RichText::new(icon).color(color));
                    let name = if item.file_name.is_empty() {
                        display_name(&item.link)
                    } else {
                        item.file_name.clone()
                    };
                    ui.label(name);
                });
                if let DownloadStatus::Downloading {
                    total,
                    downloaded,
                    speed,
                } = item.status
                {
                    let pct = if total > 0 {
                        downloaded as f32 / total as f32
                    } else {
                        0.0
                    };
                    let mb_dl = downloaded as f64 / 1_048_576.0;
                    let mb_tot = total as f64 / 1_048_576.0;
                    let mb_s = speed / 1_048_576.0;
                    let eta = if speed > 0.0 && total > downloaded {
                        let secs = (total - downloaded) as f64 / speed;
                        if secs >= 3600.0 {
                            format!("{}h {:2}m", secs as u64 / 3600, (secs as u64 % 3600) / 60)
                        } else if secs >= 60.0 {
                            format!("{}m {:2}s", secs as u64 / 60, secs as u64 % 60)
                        } else {
                            format!("{}s", secs as u64)
                        }
                    } else {
                        "—".to_string()
                    };
                    let avail = ui.available_width() - 10.0;
                    ui.add(
                        ProgressBar::new(pct)
                            .text(format!(
                                "{:.1}% · {:.1} MB / {:.1} MB · {:.1} MB/s · ETA: {}",
                                pct * 100.0, mb_dl, mb_tot, mb_s, eta
                            ))
                            .desired_width(avail.max(100.0)),
                    );
                }
                if let DownloadStatus::Failed(ref err) = item.status {
                    ui.label(RichText::new(err).color(Color32::RED).size(11.0));
                }
            });
        ui.add_space(2.0);
    }
}

// ── helpers ──────────────────────────────────────────────────

fn extract_status_icon(status: &extractor::ExtractStatus) -> (&'static str, Color32) {
    match status {
        extractor::ExtractStatus::Pending => ("⏳", Color32::GRAY),
        extractor::ExtractStatus::Extracting => ("⬇", Color32::BLUE),
        extractor::ExtractStatus::Done => ("✅", Color32::GREEN),
        extractor::ExtractStatus::Failed(_) => ("❌", Color32::RED),
    }
}

fn status_icon(status: &DownloadStatus) -> (&'static str, Color32) {
    match status {
        DownloadStatus::Pending => ("⏳", Color32::GRAY),
        DownloadStatus::FetchingPage => ("🌐", Color32::LIGHT_BLUE),
        DownloadStatus::Downloading { .. } => ("⬇", Color32::BLUE),
        DownloadStatus::Done => ("✅", Color32::GREEN),
        DownloadStatus::Failed(_) => ("❌", Color32::RED),
    }
}

fn display_name(url: &str) -> String {
    url.split('#')
        .nth(1)
        .unwrap_or(url.rsplit('/').next().unwrap_or(url))
        .to_string()
}

fn build_categories(links: &[LinkEntry]) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let mut p = Vec::new();
    let mut o = Vec::new();
    let mut x = Vec::new();
    for (i, e) in links.iter().enumerate() {
        match e.category {
            LinkCategory::Part(_) => p.push(i),
            LinkCategory::Optional => o.push(i),
            LinkCategory::Other => x.push(i),
        }
    }
    (p, o, x)
}

fn classify_link(url: &str) -> LinkCategory {
    if let Some(fragment) = url.split('#').nth(1) {
        if fragment.contains("fg-optional-") {
            return LinkCategory::Optional;
        }
        if let Some(cap) = PART_RE.captures(fragment) {
            let n: u32 = cap[1].parse().unwrap_or(0);
            return LinkCategory::Part(n);
        }
    }
    LinkCategory::Other
}

fn categorize_entries(urls: Vec<String>) -> Vec<LinkEntry> {
    let mut entries: Vec<LinkEntry> = urls
        .into_iter()
        .map(|url| {
            let category = classify_link(&url);
            LinkEntry {
                url,
                selected: true,
                category,
            }
        })
        .collect();

    // sort: parts by number, then optionals, then others
    entries.sort_by(|a, b| match (&a.category, &b.category) {
        (LinkCategory::Part(na), LinkCategory::Part(nb)) => na.cmp(nb),
        (LinkCategory::Part(_), _) => std::cmp::Ordering::Less,
        (_, LinkCategory::Part(_)) => std::cmp::Ordering::Greater,
        (LinkCategory::Optional, LinkCategory::Optional) => a.url.cmp(&b.url),
        (LinkCategory::Optional, _) => std::cmp::Ordering::Less,
        (_, LinkCategory::Optional) => std::cmp::Ordering::Greater,
        (LinkCategory::Other, LinkCategory::Other) => a.url.cmp(&b.url),
    });

    entries
}

#[derive(Clone, Default)]
struct ScrapeResult(Vec<String>);

fn load_input_file() -> Vec<String> {
    match std::fs::read_to_string("input.txt") {
        Ok(c) => c
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn extract_game_name(links: &[String]) -> String {
    for link in links {
        if let Ok(parsed) = url::Url::parse(link) {
            if let Some(fragment) = parsed.fragment() {
                let name = fragment.split("--").next().unwrap_or("").trim_matches('_');
                if !name.is_empty() {
                    return name.trim().to_string();
                }
            }
        }
    }
    String::new()
}
