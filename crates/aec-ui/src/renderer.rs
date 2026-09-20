//! Native renderer — egui-based

use aec_ast::{Program, UiDecl, Span};
use aec_runtime::{Interpreter, Value};
use crate::widgets::{build_widgets, Widget, WidgetStyle, UiState, UiValue};
use eframe::egui;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct AecApp {
    pub program: Program,
    pub interpreter: Arc<Mutex<Interpreter>>,
    pub ui_decl: UiDecl,
    pub widgets: Vec<Widget>,
    pub state: UiState,
    pub state_var_names: HashSet<String>,
    pub fonts_loaded: bool,
}

impl AecApp {
    pub fn new(program: Program, ui: UiDecl) -> Result<Self, String> {
        let mut interpreter = Interpreter::new();
        interpreter.run(&program).map_err(|e| e.to_string())?;

        let mut state = UiState::new();
        let widgets = build_widgets(&ui.screen.body, &mut state);
        let state_var_names: HashSet<String> = state.values.keys().cloned().collect();

        sync_state_to_interpreter(&state, &mut interpreter, &state_var_names);

        Ok(Self {
            program: program.clone(),
            interpreter: Arc::new(Mutex::new(interpreter)),
            ui_decl: ui.clone(),
            widgets,
            state,
            state_var_names,
            fonts_loaded: false,
        })
    }

    fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded { return; }
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Vazirmatn".to_owned(),
            egui::FontData::from_static(include_bytes!(
                "../assets/fonts/Vazirmatn-Regular.ttf"
            )).into(),
        );
        fonts.families.entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Vazirmatn".to_owned());
        fonts.families.entry(egui::FontFamily::Monospace)
            .or_default()
            .push("Vazirmatn".to_owned());
        ctx.set_fonts(fonts);
        self.fonts_loaded = true;
    }

    fn execute_event(&mut self, fn_name: &str) {
        {
            let mut interp = self.interpreter.lock().unwrap();
            sync_state_to_interpreter(&self.state, &mut interp, &self.state_var_names);
        }
        let result = {
            let mut interp = self.interpreter.lock().unwrap();
            interp.call_function(fn_name, vec![], Span::dummy())
        };
        match result {
            Ok(_) => eprintln!("[UI] ✅ Executed: {}", fn_name),
            Err(e) => eprintln!("[UI] ❌ Error in {}: {}", fn_name, e),
        }
        {
            let interp = self.interpreter.lock().unwrap();
            sync_state_from_interpreter(&mut self.state, &interp, &self.state_var_names);
        }
        let mut rebuild_state = self.state.clone();
        self.widgets = build_widgets(&self.ui_decl.screen.body, &mut rebuild_state);
    }
}

fn sync_state_to_interpreter(state: &UiState, interp: &mut Interpreter, var_names: &HashSet<String>) {
    let mut env = interp.global.borrow_mut();
    for name in var_names {
        if let Some(v) = state.values.get(name) {
            env.set(name.clone(), ui_value_to_aec_value(v));
        }
    }
}

fn sync_state_from_interpreter(state: &mut UiState, interp: &Interpreter, var_names: &HashSet<String>) {
    let env = interp.global.borrow();
    for name in var_names {
        if let Some(value) = env.get(name) {
            state.values.insert(name.clone(), aec_value_to_ui_value(&value));
        }
    }
}

fn ui_value_to_aec_value(v: &UiValue) -> Value {
    match v {
        UiValue::String(s) => Value::String(s.clone()),
        UiValue::Int(n) => Value::Int(*n),
        UiValue::Float(f) => Value::Float(*f),
        UiValue::Bool(b) => Value::Bool(*b),
        UiValue::Array(arr) => Value::Array(arr.iter().map(ui_value_to_aec_value).collect()),
        UiValue::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in obj { map.insert(k.clone(), ui_value_to_aec_value(v)); }
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
        Value::Array(arr) => UiValue::Array(arr.iter().map(aec_value_to_ui_value).collect()),
        Value::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in obj { map.insert(k.clone(), aec_value_to_ui_value(v)); }
            UiValue::Object(map)
        }
        _ => UiValue::String(String::new()),
    }
}

fn is_rtl(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{0600}'..='\u{06FF}' |
            '\u{0750}'..='\u{077F}' |
            '\u{08A0}'..='\u{08FF}' |
            '\u{FB50}'..='\u{FDFF}' |
            '\u{FE70}'..='\u{FEFF}'
        )
    })
}

fn make_text(text: &str, style: &WidgetStyle) -> egui::RichText {
    let _ = is_rtl(text);
    let mut rt = egui::RichText::new(text);
    if let Some(color_str) = style.get_string("color").or_else(|| style.get_string("text_color")) {
        if let Some(color) = parse_color(&color_str) { rt = rt.color(color); }
    }
    if let Some(size_str) = style.get_string("size").or_else(|| style.get_string("font_size")) {
        if let Ok(size) = size_str.parse::<f32>() { rt = rt.size(size); }
    }
    if let Some(weight) = style.get_string("weight").or_else(|| style.get_string("font_weight")) {
        match weight.as_str() {
            "bold" | "700" => rt = rt.strong(),
            "italic" => rt = rt.italics(),
            _ => {}
        }
    }
    rt
}

fn parse_color(s: &str) -> Option<egui::Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        return Some(egui::Color32::from_rgb(r, g, b));
    }
    if s.len() == 8 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        let a = u8::from_str_radix(&s[6..8], 16).ok()?;
        return Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    match s.to_lowercase().as_str() {
        "red" => Some(egui::Color32::RED),
        "green" => Some(egui::Color32::GREEN),
        "blue" => Some(egui::Color32::BLUE),
        "white" => Some(egui::Color32::WHITE),
        "black" => Some(egui::Color32::BLACK),
        "gray" | "grey" => Some(egui::Color32::GRAY),
        _ => None,
    }
}

impl eframe::App for AecApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.load_fonts(ctx);
        let mut pending = Vec::new();

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(&self.ui_decl.screen.title);
                ui.add_space(16.0);
                let widgets = self.widgets.clone();
                render_widgets(ui, &widgets, &mut self.state, &mut pending);
            });
        });

        for event in pending {
            self.execute_event(&event);
        }
    }
}

fn render_widgets(ui: &mut egui::Ui, widgets: &[Widget], state: &mut UiState, pending: &mut Vec<String>) {
    for widget in widgets {
        render_widget(ui, widget, state, pending);
    }
}

fn render_widget(ui: &mut egui::Ui, widget: &Widget, state: &mut UiState, pending: &mut Vec<String>) {
    match widget {
        Widget::Column(children, style) => {
            let mut frame = egui::Frame::default();
            if let Some(bg) = style.get_string("background") {
                if let Some(c) = parse_color(&bg) { frame = frame.fill(c); }
            }
            frame.show(ui, |ui| {
                ui.vertical(|ui| {
                    render_widgets(ui, children, state, pending);
                });
            });
        }

        Widget::Row(children, style) => {
            let mut frame = egui::Frame::default();
            if let Some(bg) = style.get_string("background") {
                if let Some(c) = parse_color(&bg) { frame = frame.fill(c); }
            }
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    render_widgets(ui, children, state, pending);
                });
            });
        }

        Widget::Card(children, style) => {
            let mut frame = egui::Frame::group(ui.style());
            if let Some(bg) = style.get_string("background") {
                if let Some(c) = parse_color(&bg) { frame = frame.fill(c); }
            }
            frame.show(ui, |ui| {
                render_widgets(ui, children, state, pending);
            });
            ui.add_space(8.0);
        }

        Widget::Container(children) => {
            render_widgets(ui, children, state, pending);
        }

        Widget::Text(text, style) => {
            ui.label(make_text(text, style));
        }

        Widget::Display { var_name, style } => {
            let value = state.get_string(var_name);
            if value.is_empty() {
                ui.label(egui::RichText::new("(empty)").italics().weak());
            } else {
                ui.label(make_text(&value, style));
            }
        }

        Widget::Input { bind_target, placeholder, value, style } => {
            let _ = style;
            if let Some(target) = bind_target {
                let mut val = state.get_string(target);
                let mut text_edit = egui::TextEdit::singleline(&mut val);
                if let Some(ph) = placeholder {
                    text_edit = text_edit.hint_text(ph.as_str());
                }
                if ui.add(text_edit).changed() {
                    state.set_string(target, val);
                }
            } else {
                let mut dummy = value.clone();
                ui.add(egui::TextEdit::singleline(&mut dummy));
            }
        }

        Widget::Button { label, on_click, style } => {
            let bg = style.get_string("background")
                .or_else(|| style.get_string("background_color"))
                .and_then(|s| parse_color(&s));

            let text_color = style.get_string("color")
                .or_else(|| style.get_string("text_color"))
                .and_then(|s| parse_color(&s));

            let padding = style.get_string("padding")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(8.0);

            let radius = style.get_string("radius")
                .or_else(|| style.get_string("border_radius"))
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(6.0);

            let mut text = egui::RichText::new(label);
            if let Some(c) = text_color {
                text = text.color(c);
            }
            if let Some(weight) = style.get_string("weight") {
                if weight == "bold" {
                    text = text.strong();
                }
            }

            let button = egui::Button::new(text)
                .min_size(egui::vec2(100.0, 0.0))
                .rounding(egui::Rounding::same(radius));

            let button = if let Some(bg_color) = bg {
                button.fill(bg_color)
            } else {
                button
            };

            let response = ui.add(button);

            if response.clicked() {
                if let Some(fn_name) = on_click {
                    pending.push(fn_name.clone());
                }
            }
        }

        Widget::MessagesList { source: _, items, style: _ } => {
            if items.is_empty() {
                ui.label(egui::RichText::new("No messages yet").italics().weak());
            }
            for item in items {
                let is_user = item.role == "user";
                let (label, color) = if is_user {
                    ("👤 You", egui::Color32::from_rgb(102, 126, 234))
                } else {
                    ("🤖 AI", egui::Color32::from_rgb(168, 224, 32))
                };
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(label).strong().color(color));
                    ui.label(egui::RichText::new(&item.content));
                });
                ui.add_space(4.0);
            }
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

        Widget::Divider => { ui.separator(); }
        Widget::Heading(text, style) => { ui.heading(make_text(text, style)); }
        Widget::Spacer => { ui.add_space(8.0); }
    }
}

pub fn run_ui(program: &Program, ui: &UiDecl) -> Result<(), eframe::Error> {
    let app = AecApp::new(program.clone(), ui.clone())
        .map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other, e
        ))))?;

    let title = ui.screen.title.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 800.0])
            .with_title(&title),
        ..Default::default()
    };

    eframe::run_native(&title, options, Box::new(|_cc| Ok(Box::new(app))))
}
