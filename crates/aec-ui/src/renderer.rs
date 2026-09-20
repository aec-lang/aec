//! Native renderer — تبدیل widget tree به egui
//! با اتصال به Interpreter AEC

use aec_ast::{Program, UiDecl, Span};
use aec_runtime::{Interpreter, Value};
use crate::widgets::{build_widgets, Widget, UiState, UiValue};
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct AecApp {
    pub program: Program,
    pub interpreter: Arc<Mutex<Interpreter>>,
    pub ui_decl: UiDecl,
    pub widgets: Vec<Widget>,
    pub state: UiState,
    pub pending_events: Vec<String>,
}

impl AecApp {
    pub fn new(program: Program, ui: UiDecl) -> Result<Self, String> {
        let mut interpreter = Interpreter::new();

        // ثبت توابع
        interpreter.run(&program).map_err(|e| e.to_string())?;

        let mut state = UiState::new();

        // ساخت widgets اولیه
        let widgets = build_widgets(&ui.screen.body, &mut state);

        // State ها رو به interpreter پاس بده
        sync_state_to_interpreter(&state, &mut interpreter);

        Ok(Self {
            program: program.clone(),
            interpreter: Arc::new(Mutex::new(interpreter)),
            ui_decl: ui.clone(),
            widgets,
            state,
            pending_events: Vec::new(),
        })
    }

    fn execute_event(&mut self, fn_name: &str) {
        // 1. State رو به interpreter بفرست (فقط متغیرهای state)
        {
            let mut interp = self.interpreter.lock().unwrap();
            sync_state_to_interpreter(&self.state, &mut interp);
        }

        // 2. تابع AEC رو اجرا کن
        let result = {
            let mut interp = self.interpreter.lock().unwrap();
            interp.call_function(fn_name, vec![], Span::dummy())
        };

        match result {
            Ok(_) => {
                eprintln!("[UI] ✅ Executed AEC function: {}", fn_name);
            }
            Err(e) => {
                eprintln!("[UI] ❌ Error executing {}: {}", fn_name, e);
            }
        }

        // 3. State رو از interpreter بگیر (فقط متغیرهای state)
        {
            let interp = self.interpreter.lock().unwrap();
            sync_state_from_interpreter(&mut self.state, &interp);
        }

        // 4. Widget ها رو دوباره بساز
        let mut new_state = self.state.clone();
        self.widgets = build_widgets(&self.ui_decl.screen.body, &mut new_state);
        self.state = new_state;
    }
}

/// State رو از UiState به Interpreter پاس بده
fn sync_state_to_interpreter(state: &UiState, interp: &mut Interpreter) {
    for (name, value) in &state.values {
        let aec_value = ui_value_to_aec_value(value);
        interp.global.borrow_mut().set(name.clone(), aec_value);
    }
}

/// State رو از Interpreter به UiState بگیر
fn sync_state_from_interpreter(state: &mut UiState, interp: &Interpreter) {
    let env = interp.global.borrow();

    // فقط متغیرهایی که توی UiState هستن رو update کن
    let state_names: Vec<String> = state.values.keys().cloned().collect();

    for name in state_names {
        if let Some(value) = env.get(&name) {
            let ui_value = aec_value_to_ui_value(&value);
            state.values.insert(name.clone(), ui_value);
        }
    }
}

fn ui_value_to_aec_value(v: &UiValue) -> Value {
    match v {
        UiValue::String(s) => Value::String(s.clone()),
        UiValue::Int(n) => Value::Int(*n),
        UiValue::Float(f) => Value::Float(*f),
        UiValue::Bool(b) => Value::Bool(*b),
        UiValue::Array(arr) => {
            let items: Vec<Value> = arr.iter().map(ui_value_to_aec_value).collect();
            Value::Array(items)
        }
        UiValue::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), ui_value_to_aec_value(v));
            }
            Value::Object(map)
        }
    }
}

fn aec_value_to_ui_value(v: &Value) -> UiValue {
    match v {
        Value::String(s) => UiValue::String(s.clone()),
        Value::Int(n) => UiValue::Int(*n),
        Value::Float(f) => UiValue::Float(*f),
        Value::Bool(b) => UiValue::Bool(*b),
        Value::Array(arr) => {
            let items: Vec<UiValue> = arr.iter().map(aec_value_to_ui_value).collect();
            UiValue::Array(items)
        }
        Value::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), aec_value_to_ui_value(v));
            }
            UiValue::Object(map)
        }
        _ => UiValue::String(String::new()),
    }
}

impl eframe::App for AecApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending = Vec::new();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.ui_decl.screen.title);
            ui.add_space(16.0);

            let widgets = self.widgets.clone();
            render_widgets(ui, &widgets, &mut self.state, &mut pending);
        });

        // اگه event داریم، همون‌جا اجرا کن
        for event in pending {
            self.execute_event(&event);
        }
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
pub fn run_ui(program: &Program, ui: &UiDecl) -> Result<(), eframe::Error> {
    let app = AecApp::new(program.clone(), ui.clone())
        .map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other, e
        ))))?;

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
