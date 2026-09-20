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
}

impl AecApp {
    pub fn new(ui: &UiDecl) -> Self {
        let mut state = UiState::new();
        let widgets = build_widgets(&ui.screen.body, &mut state);

        Self {
            screen_title: ui.screen.title.clone(),
            widgets,
            state,
        }
    }
}

impl eframe::App for AecApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.screen_title);
            ui.add_space(16.0);

            // Clone widgets برای اجتناب از borrow issues
            let widgets = self.widgets.clone();
            render_widgets(ui, &widgets, &mut self.state);
        });
    }
}

fn render_widgets(ui: &mut egui::Ui, widgets: &[Widget], state: &mut UiState) {
    for widget in widgets {
        render_widget(ui, widget, state);
    }
}

fn render_widget(ui: &mut egui::Ui, widget: &Widget, state: &mut UiState) {
    match widget {
        Widget::Column(children) => {
            ui.vertical(|ui| {
                render_widgets(ui, children, state);
            });
        }

        Widget::Row(children) => {
            ui.horizontal(|ui| {
                render_widgets(ui, children, state);
            });
        }

        Widget::Card(children) => {
            egui::Frame::group(ui.style())
                .show(ui, |ui| {
                    render_widgets(ui, children, state);
                });
            ui.add_space(8.0);
        }

        Widget::Container(children) => {
            render_widgets(ui, children, state);
        }

        Widget::Text(text) => {
            ui.label(text);
        }

        Widget::Input { bind_target, placeholder } => {
            if let Some(target) = bind_target {
                let mut value = state.get_string(target);
                let mut text_edit = egui::TextEdit::singleline(&mut value);
                if let Some(ph) = placeholder {
                    text_edit = text_edit.hint_text(ph);
                }
                if ui.add(text_edit).changed() {
                    state.set_string(target, value);
                }
            } else {
                let mut dummy = String::new();
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
                    // TODO: فراخوانی تابع AEC
                    eprintln!("[UI] Button clicked → call AEC function: {}", fn_name);
                }
            }
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
