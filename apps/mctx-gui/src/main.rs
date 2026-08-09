//! mctx-gui — a desktop notepad for `.mctx` memory files with two views:
//!
//!   * **Human** — the memory rendered as readable Markdown (headings + raw
//!     body text), so a person sees exactly what an agent reads.
//!   * **AI** — the computable view: the raw `.mctx` source, and a structured
//!     JSON breakdown (sections, tiers, versions, byte offsets, bodies).
//!
//! Both views are live off the same in-memory buffer, so what you save is
//! what both humans and agents parse. Native file dialogs (rfd) are used for
//! open/save, and the binary is registered so `.mctx` files can be opened
//! straight from a file manager (see the packaging scripts).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    let path = std::env::args().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([940.0, 660.0])
            .with_min_inner_size([640.0, 400.0])
            .with_title("mctx"),
        ..Default::default()
    };

    eframe::run_native(
        "mctx",
        options,
        Box::new(move |cc| Ok(Box::new(MctxApp::new(cc, path)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Human,
    Ai,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AiPane {
    Raw,
    Json,
}

struct MctxApp {
    path: Option<PathBuf>,
    source: String,
    saved_text: String,
    error: Option<String>,
    status: Option<String>,
    view: View,
    ai_pane: AiPane,
}

impl MctxApp {
    fn new(_cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut app = MctxApp {
            path: None,
            source: new_document(),
            saved_text: String::new(),
            error: None,
            status: None,
            view: View::Human,
            ai_pane: AiPane::Raw,
        };
        if let Some(path) = path {
            app.open(path);
        }
        app
    }

    fn dirty(&self) -> bool {
        self.source != self.saved_text
    }

    fn open(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.path = Some(path);
                self.source = text;
                self.saved_text = self.source.clone();
                self.error = None;
                self.status = Some("opened".to_string());
            }
            Err(e) => {
                self.error = Some(format!("open {}: {e}", path.display()));
            }
        }
    }

    fn save(&mut self) {
        if self.path.is_none() {
            return self.save_as();
        }
        self.write_current();
    }

    fn save_as(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("mctx", &["mctx"]);
        if let Some(path) = &self.path {
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy().to_string());
            }
        } else {
            dialog = dialog.set_file_name("memory.mctx");
        }
        if let Some(path) = dialog.save_file() {
            self.path = Some(path);
            self.write_current();
        }
    }

    fn write_current(&mut self) {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => return,
        };
        match std::fs::write(&path, &self.source) {
            Ok(()) => {
                self.saved_text = self.source.clone();
                self.error = None;
                self.status = Some(format!("saved {}", path.display()));
            }
            Err(e) => {
                self.error = Some(format!("save {}: {e}", path.display()));
            }
        }
    }

    fn ai_json(&self) -> String {
        mctx::render_json(&self.source)
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("topbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Open…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("mctx", &["mctx"])
                        .add_filter("text", &["txt", "md"])
                        .pick_file()
                    {
                        self.open(path);
                    }
                }
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Save As…").clicked() {
                    self.save_as();
                }
                ui.separator();
                if ui.button("New").clicked() {
                    self.source = new_document();
                    self.path = None;
                    self.saved_text = String::new();
                    self.error = None;
                }

                ui.separator();
                if ui.selectable_label(self.view == View::Human, "Human").clicked() {
                    self.view = View::Human;
                }
                if ui.selectable_label(self.view == View::Ai, "AI").clicked() {
                    self.view = View::Ai;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.dirty() {
                        ui.colored_label(egui::Color32::YELLOW, "● unsaved");
                    }
                    if let Some(path) = &self.path {
                        ui.label(
                            egui::RichText::new(path.display().to_string())
                                .weak()
                                .small(),
                        );
                    } else {
                        ui.label(egui::RichText::new("no file").weak().small());
                    }
                });
            });
            ui.add_space(4.0);
        });
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let parsed = mctx::parse_content(&self.source);
                ui.label(
                    egui::RichText::new(format!(
                        "mctx v1.1 · {} sections · {} bytes",
                        parsed.sections.len(),
                        self.source.len()
                    ))
                    .weak()
                    .small(),
                );
                ui.separator();
                if let Some(status) = &self.status {
                    ui.label(egui::RichText::new(status).small());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(error) = &self.error {
                        ui.colored_label(egui::Color32::RED, error);
                    }
                });
            });
            ui.add_space(2.0);
        });
    }

    fn human_view(&mut self, ui: &mut egui::Ui) {
        let parsed = mctx::parse_content(&self.source);
        let header = parsed.header.trim();
        if !header.is_empty() {
            ui.label(egui::RichText::new(header).size(22.0).strong());
            ui.add_space(6.0);
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt("human_scroll")
            .show(ui, |ui| {
                for (i, section) in parsed.sections.iter().enumerate() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("## {}", section.name))
                                .heading()
                                .strong(),
                        );
                        let color = match section.tier.as_str() {
                            "!fixed" => egui::Color32::from_rgb(160, 200, 255),
                            "!volatile" => egui::Color32::from_rgb(255, 190, 140),
                            _ => egui::Color32::from_rgb(160, 230, 170),
                        };
                        ui.label(
                            egui::RichText::new(section.tier.clone())
                                .monospace()
                                .color(color)
                                .small(),
                        );
                        ui.label(
                            egui::RichText::new(format!("v{}", section.version))
                                .weak()
                                .small(),
                        );
                    });
                    if let Some((_, body)) = parsed.bodies.get(i) {
                        ui.label(egui::RichText::new(body.trim_end_matches('\n')).monospace());
                    }
                }
                if parsed.sections.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("No sections yet — open an .mctx file or edit in the AI view.")
                            .weak(),
                    );
                }
                ui.add_space(12.0);
            });
    }

    fn ai_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.ai_pane == AiPane::Raw, "Raw .mctx").clicked() {
                self.ai_pane = AiPane::Raw;
            }
            if ui.selectable_label(self.ai_pane == AiPane::Json, "JSON structure").clicked() {
                self.ai_pane = AiPane::Json;
            }
            ui.separator();
            ui.label(
                egui::RichText::new(
                    "this is the source of truth — humans see it rendered on the Human tab",
                )
                .weak()
                .small(),
            );
        });
        ui.separator();

        match self.ai_pane {
            AiPane::Raw => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt("raw_scroll")
                    .show(ui, |ui| {
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut self.source)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(34),
                        );
                        if response.changed() {
                            self.status = Some("editing…".to_string());
                        }
                    });
            }
            AiPane::Json => {
                let json = self.ai_json();
                ui.horizontal(|ui| {
                    if ui.button("Copy JSON").clicked() {
                        ui.ctx().copy_text(json.clone());
                        self.status = Some("JSON copied to clipboard".to_string());
                    }
                    ui.label(
                        egui::RichText::new("what an agent can parse from this buffer")
                            .weak()
                            .small(),
                    );
                });
                ui.add_space(4.0);
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .id_salt("json_scroll")
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(json).monospace().small());
                    });
            }
        }
    }
}

impl eframe::App for MctxApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                self.open(path);
            }
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S)
        }) {
            self.save_as();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }

        self.top_bar(ui);
        self.bottom_bar(ui);

        egui::CentralPanel::default().show(ui, |ui| match self.view {
            View::Human => self.human_view(ui),
            View::Ai => self.ai_view(ui),
        });
    }
}

/// A fresh `.mctx` skeleton with a valid header and an empty checkpoint.
fn new_document() -> String {
    format!(
        "{}\n%%INDEX\n%%END-INDEX\n%%@checkpoint !volatile v:1\n(notes…)\n%%END\n",
        mctx::make_header()
    )
}
