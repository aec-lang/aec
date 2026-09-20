//! Native renderer — تبدیل widget tree به egui
//!
//! از egui استفاده می‌کنه (بالای wgpu).
//! GPU-accelerated، cross-platform.

use aec_ast::UiDecl;
use crate::widgets::{build_widgets, Widget, UiState};
use eframe::egui;

pub struct AecApp {
    pub screen_title: String,
    pub widgets: Vec<Widget>,
    pub state: UiState,
    pub pending_calls: Vec<String>,
}

impl AecApp {
    pub fn new(ui: &UiDecl) -> Self {
        let mut state = UiState::new();
        let widgets = build_widgets(&ui.screen.body, &mut state);

        Self {
            screen_title: ui.screen.title.clone(),
            widgets,
            state,
            pending_calls: Vec::new(),
        }
    }
}

impl eframe::App for AecApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.screen_title);
            ui.add_space(16.0);

            let widgets = self.widgets.clone();
            let mut pending = Vec::new();
            render_widgets(ui, &widgets, &mut self.state, &mut pending);

            // Event ها رو جمع کن
            self.pending_calls.extend(pending);

            // اگه event داریم، تابع AEC رو صدا بزن
            if !self.pending_calls.is_empty() {
                let calls = std::mem::take(&mut self.pending_calls);
                for call in calls {
                    eprintln!("[UI] Calling AEC function: {}", call);
                }
            }
        });
    }
}

fn render_widgets(
    ui: &mut egui::Ui,
    widgets: &[Widget],
    state: &mut UiState,
    pending: &mut Vec<String>,
) {
    for widget in widgets {
        render_widget(ui, widget, state, pending);
    }
}

fn render_widget(
    ui: &mut egui::Ui,
    widget: &Widget,
    state: &mut UiState,
    pending: &mut Vec<String>,
) {
    match widget {
        Widget::Column(children) => {
            ui.vertical(|ui| {
                render_widgets(ui, children, state, pending);
            });
        }

        Widget::Row(children) => {
            ui.horizontal(|ui| {
                render_widgets(ui, children, state, pending);
            });
        }

        Widget::Card(children) => {
            egui::Frame::group(ui.style())
                .show(ui, |ui| {
                    render_widgets(ui, children, state, pending);
                });
            ui.add_space(8.0);
        }

        Widget::Container(children) => {
            render_widgets(ui, children, state, pending);
        }

        Widget::Text(text) => {
            ui.label(text);
        }

        Widget::Input { bind_target, placeholder, value } => {
            if let Some(target) = bind_target {
                let mut val = state.get_string(target);
                let mut text_edit = egui::TextEdit::singleline(&mut val);
                if let Some(ph) = placeholder {
                    text_edit = text_edit.hint_text(ph);
                }
                if ui.add(text_edit).changed() {
                    state.set_string(target, val);
                }
            } else {
                let mut dummy = value.clone();
                let mut text_edit = egui::TextEdit::singleline(&mut dummy);
                if let Some(ph) = placeholder {
                    text_edit = text_edit.hint_text(ph);
                }
                ui.add(text_edit);
            }
        }

        Widget::Button { label, on_click } => {
            if ui.button(label).clicked() {
                if let Some(fn_name) = on_click {
                    pending.push(fn_name.clone());
                }
            }
        }

        Widget::MessagesList { source: _, items } => {
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for item in items {
                        let is_user = item.role == "user";
                        let prefix = if is_user { "👤 You" } else { "🤖 AI" };

                        egui::Frame::group(ui.style())
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(prefix).strong());
                                });
                                ui.label(&item.content);
                            });
                        ui.add_space(4.0);
                    }
                });
        }

        Widget::If { condition, then_branch, else_branch } => {
            if *condition {
                render_widgets(ui, then_branch, state, pending);
            } else if let Some(else_b) = else_branch {
                render_widgets(ui, else_b, state, pending);
            }
        }

        Widget::For { variable: _, items } => {
            render_widgets(ui, items, state, pending);
        }
    }
}

/// اجرای یه UI
pub fn run_ui(ui: &UiDecl) -> Result<(), eframe::Error> {
    let app = AecApp::new(ui);
    let title = ui.screen.title.clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title(&title),
        ..Default::default()
    };

    eframe::run_native(
        &title,
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
