//! Native renderer — egui-based

use crate::widgets::{
    build_widgets_with_themes, ComponentRegistry, EventAction, Themes, UiState, UiValue, Widget,
    WidgetStyle,
};
use aec_ast::{Program, TopLevelItem, UiDecl, UiStatement};
use aec_runtime::{Interpreter, Value};
use eframe::egui;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct AecApp {
    pub program: Program,
    #[allow(clippy::arc_with_non_send_sync)]
    pub interpreter: Arc<Mutex<Interpreter>>,
    pub ui_decl: UiDecl,
    pub components: ComponentRegistry,
    pub themes: Themes,
    pub widgets: Vec<Widget>,
    pub state: UiState,
    pub state_var_names: HashSet<String>,
    pub fonts_loaded: bool,
}

#[allow(clippy::arc_with_non_send_sync)]
impl AecApp {
    pub fn new(program: Program, ui: UiDecl) -> Result<Self, String> {
        let mut interpreter = Interpreter::new();
        interpreter.run(&program).map_err(|e| e.to_string())?;

        let mut components = ComponentRegistry::new();
        for (index, item) in program.items.iter().enumerate() {
            let TopLevelItem::Component(component) = item else {
                continue;
            };
            let module = program.item_modules.get(index).cloned().flatten();
            if let Some(scope) = module {
                components.insert(
                    format!("{}::{}", scope, component.name.name),
                    component.clone(),
                );
                if component.is_public {
                    components.insert(
                        format!("{}.{}", scope, component.name.name),
                        component.clone(),
                    );
                }
            } else {
                components.insert(component.name.name.clone(), component.clone());
            }
        }

        let themes = Themes::from_items_with_modules(&program.items, &program.item_modules);

        let mut state = UiState::new();
        let widgets = build_widgets_with_themes(&ui.screen.body, &mut state, &components, &themes);
        let state_var_names = collect_state_names(&ui.screen.body);

        sync_state_to_interpreter(&state, &mut interpreter, &state_var_names);

        Ok(Self {
            program: program.clone(),
            interpreter: Arc::new(Mutex::new(interpreter)),
            ui_decl: ui.clone(),
            components,
            themes,
            widgets,
            state,
            state_var_names,
            fonts_loaded: false,
        })
    }

    fn load_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_loaded {
            return;
        }
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Vazirmatn".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/Vazirmatn-Regular.ttf")),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Vazirmatn".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("Vazirmatn".to_owned());
        ctx.set_fonts(fonts);
        self.fonts_loaded = true;
    }

    #[cfg(test)]
    fn execute_event(&mut self, fn_name: &str) {
        self.execute_action(EventAction {
            handler: aec_ast::Expr::Call(Box::new(aec_ast::CallExpr {
                callee: aec_ast::Expr::Identifier(aec_ast::Identifier::new(
                    fn_name,
                    aec_ast::Span::dummy(),
                )),
                args: Vec::new(),
                span: aec_ast::Span::dummy(),
            })),
            span: aec_ast::Span::dummy(),
            scope: None,
        });
    }

    fn execute_action(&mut self, action: EventAction) {
        {
            let mut interp = self
                .interpreter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            sync_state_to_interpreter(&self.state, &mut interp, &self.state_var_names);
            if let Some(scope) = &action.scope {
                let prefix = format!("{}::", scope);
                let local_values: Vec<(String, UiValue)> = self
                    .state
                    .values
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix(&prefix)
                            .map(|name| (name.to_string(), value.clone()))
                    })
                    .collect();
                let mut environment = interp.global.borrow_mut();
                for (name, value) in local_values {
                    environment.set(name, ui_value_to_aec_value(&value));
                }
            }
        }
        let result = {
            let mut interp = self
                .interpreter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let global = interp.global.clone();
            interp.eval_expr(&action.handler, global)
        };
        match result {
            Ok(_) => eprintln!("[UI] ✅ Executed event"),
            Err(error) => eprintln!("[UI] ❌ Event error: {}", error),
        }
        {
            let interp = self
                .interpreter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            sync_state_from_interpreter(&mut self.state, &interp, &self.state_var_names);
            if let Some(scope) = &action.scope {
                let prefix = format!("{}::", scope);
                let names: Vec<String> = self
                    .state
                    .values
                    .keys()
                    .filter_map(|key| key.strip_prefix(&prefix).map(str::to_string))
                    .collect();
                let environment = interp.global.borrow();
                for name in names {
                    if let Some(value) = environment.get(&name) {
                        self.state
                            .values
                            .insert(format!("{}{}", prefix, name), aec_value_to_ui_value(&value));
                    }
                }
            }
        }
        let mut rebuild_state = self.state.clone();
        self.widgets = build_widgets_with_themes(
            &self.ui_decl.screen.body,
            &mut rebuild_state,
            &self.components,
            &self.themes,
        );
        self.state = rebuild_state;
        let mut interp = self
            .interpreter
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sync_state_to_interpreter(&self.state, &mut interp, &self.state_var_names);
    }
}

fn collect_state_names(statements: &[UiStatement]) -> HashSet<String> {
    let mut names = HashSet::new();
    for statement in statements {
        match statement {
            UiStatement::State(state) => {
                names.insert(state.name.name.clone());
            }
            UiStatement::Element(element) => {
                if let Some(children) = &element.children {
                    names.extend(collect_state_names(children));
                }
            }
            UiStatement::If(branch) => {
                names.extend(collect_state_names(&branch.then_body));
                if let Some(otherwise) = &branch.else_body {
                    names.extend(collect_state_names(otherwise));
                }
            }
            UiStatement::For(_) | UiStatement::Component(_) => {}
        }
    }
    names
}

fn sync_state_to_interpreter(
    state: &UiState,
    interp: &mut Interpreter,
    var_names: &HashSet<String>,
) {
    let mut env = interp.global.borrow_mut();
    for name in var_names {
        if let Some(v) = state.values.get(name) {
            env.set(name.clone(), ui_value_to_aec_value(v));
        }
    }
}

fn sync_state_from_interpreter(
    state: &mut UiState,
    interp: &Interpreter,
    var_names: &HashSet<String>,
) {
    let env = interp.global.borrow();
    for name in var_names {
        if let Some(value) = env.get(name) {
            state
                .values
                .insert(name.clone(), aec_value_to_ui_value(&value));
        }
    }
}

fn ui_value_to_aec_value(v: &UiValue) -> Value {
    match v {
        UiValue::None => Value::None,
        UiValue::String(s) => Value::String(s.clone()),
        UiValue::Int(n) => Value::Int(*n),
        UiValue::Float(f) => Value::Float(*f),
        UiValue::Bool(b) => Value::Bool(*b),
        UiValue::Array(arr) => Value::Array(arr.iter().map(ui_value_to_aec_value).collect()),
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
        Value::None => UiValue::None,
        Value::String(s) => UiValue::String(s.clone()),
        Value::Int(n) => UiValue::Int(*n),
        Value::Float(f) => UiValue::Float(*f),
        Value::Bool(b) => UiValue::Bool(*b),
        Value::Array(arr) => UiValue::Array(arr.iter().map(aec_value_to_ui_value).collect()),
        Value::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), aec_value_to_ui_value(v));
            }
            UiValue::Object(map)
        }
        // A result has no UI counterpart; show it the way the runtime prints it.
        Value::Result(Ok(v)) => UiValue::String(format!("ok({})", v)),
        Value::Result(Err(e)) => UiValue::String(format!("err({})", e)),
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

fn style_value_color(value: &aec_ast::StyleValue) -> Option<egui::Color32> {
    match value {
        aec_ast::StyleValue::String(s) => parse_color(s),
        aec_ast::StyleValue::Ident(s) => parse_color(s),
        _ => None,
    }
}

fn make_text(text: &str, style: &WidgetStyle) -> egui::RichText {
    let _ = is_rtl(text);
    let mut rt = egui::RichText::new(text);
    if let Some(color_str) = style
        .get_string("color")
        .or_else(|| style.get_string("text_color"))
    {
        if let Some(color) = parse_color(&color_str) {
            rt = rt.color(color);
        }
    }
    if let Some(size_str) = style
        .get_string("size")
        .or_else(|| style.get_string("font_size"))
    {
        if let Ok(size) = size_str.parse::<f32>() {
            rt = rt.size(size);
        }
    }
    if let Some(weight) = style
        .get_string("weight")
        .or_else(|| style.get_string("font_weight"))
    {
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
    if (s.len() == 6 || s.len() == 8) && s.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        if s.len() == 6 {
            return Some(egui::Color32::from_rgb(r, g, b));
        }
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
        let mut rebuild_state = self.state.clone();
        self.widgets = build_widgets_with_themes(
            &self.ui_decl.screen.body,
            &mut rebuild_state,
            &self.components,
            &self.themes,
        );
        self.state = rebuild_state;
        let mut pending: Vec<EventAction> = Vec::new();

        // Default background taken from the active theme
        let background = self
            .themes
            .active()
            .and_then(|t| t.get("color.background"))
            .and_then(style_value_color);

        let mut panel = egui::CentralPanel::default();
        if let Some(color) = background {
            panel = panel.frame(egui::Frame::default().fill(color));
        }
        panel.show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(&self.ui_decl.screen.title);
                ui.add_space(16.0);
                let widgets = self.widgets.clone();
                render_widgets(ui, &widgets, &mut self.state, &mut pending);
            });
        });

        for event in pending {
            self.execute_action(event);
        }
    }
}

fn render_widgets(
    ui: &mut egui::Ui,
    widgets: &[Widget],
    state: &mut UiState,
    pending: &mut Vec<EventAction>,
) {
    for widget in widgets {
        render_widget(ui, widget, state, pending);
    }
}

fn render_label(ui: &mut egui::Ui, text: &str, style: &WidgetStyle) {
    if is_rtl(text) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            ui.label(make_text(text, style));
        });
    } else {
        ui.label(make_text(text, style));
    }
}

fn render_widget(
    ui: &mut egui::Ui,
    widget: &Widget,
    state: &mut UiState,
    pending: &mut Vec<EventAction>,
) {
    match widget {
        Widget::Column(children, style) => {
            let mut frame = egui::Frame::default();
            if let Some(bg) = style.get_string("background") {
                if let Some(c) = parse_color(&bg) {
                    frame = frame.fill(c);
                }
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
                if let Some(c) = parse_color(&bg) {
                    frame = frame.fill(c);
                }
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
                if let Some(c) = parse_color(&bg) {
                    frame = frame.fill(c);
                }
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
            render_label(ui, text, style);
        }

        Widget::Display { var_name, style } => {
            let value = state.get_string(var_name);
            if value.is_empty() {
                ui.label(egui::RichText::new("(empty)").italics().weak());
            } else {
                render_label(ui, &value, style);
            }
        }

        Widget::Input {
            bind_target,
            placeholder,
            value,
            style,
        } => {
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

        Widget::Button {
            label,
            on_click,
            style,
        } => {
            let bg = style
                .get_string("background")
                .or_else(|| style.get_string("background_color"))
                .and_then(|s| parse_color(&s));

            let text_color = style
                .get_string("color")
                .or_else(|| style.get_string("text_color"))
                .and_then(|s| parse_color(&s));

            let padding = style
                .get_string("padding")
                .and_then(|s| s.parse::<f32>().ok());

            let radius = style
                .get_string("radius")
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

            let response = if let Some(padding) = padding {
                // egui has no per-button padding on `Button` itself, so scope the
                // setting over this one widget.
                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(padding, padding * 0.5);
                    ui.add(button)
                })
                .inner
            } else {
                ui.add(button)
            };

            if response.clicked() {
                if let Some(action) = on_click {
                    pending.push(action.clone());
                }
            }
        }

        Widget::MessagesList { source, style: _ } => {
            // Pull the items live from state
            let items: Vec<(String, String)> = state
                .get_value(source)
                .map(|v| v.as_array())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|item| {
                    if let UiValue::Object(o) = item {
                        let role = o
                            .get("role")
                            .map(|v| v.as_string())
                            .unwrap_or_else(|| "user".to_string());
                        let content = o.get("content").map(|v| v.as_string()).unwrap_or_default();
                        Some((role, content))
                    } else {
                        None
                    }
                })
                .collect();

            if items.is_empty() {
                ui.label(egui::RichText::new("No messages yet").italics().weak());
            }

            egui::ScrollArea::vertical()
                .id_salt(format!("messages_{}", source))
                .max_height(400.0)
                .show(ui, |ui| {
                    for (role, content) in items {
                        let is_user = role == "user";
                        let (label, color) = if is_user {
                            ("👤 You", egui::Color32::from_rgb(102, 126, 234))
                        } else {
                            ("🤖 AI", egui::Color32::from_rgb(168, 224, 32))
                        };
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.label(egui::RichText::new(label).strong().color(color));
                            ui.label(egui::RichText::new(&content));
                        });
                        ui.add_space(4.0);
                    }
                });
        }

        Widget::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if *condition {
                render_widgets(ui, then_branch, state, pending);
            } else if let Some(else_b) = else_branch {
                render_widgets(ui, else_b, state, pending);
            }
        }

        Widget::For { variable: _, items } => {
            render_widgets(ui, items, state, pending);
        }

        Widget::Divider => {
            ui.separator();
        }
        Widget::Heading(text, style) => {
            if is_rtl(text) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.heading(make_text(text, style));
                });
            } else {
                ui.heading(make_text(text, style));
            }
        }
        Widget::Spacer => {
            ui.add_space(8.0);
        }
    }
}

pub fn run_ui(program: &Program, ui: &UiDecl) -> Result<(), eframe::Error> {
    let app = AecApp::new(program.clone(), ui.clone()).map_err(|e| {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(e)))
    })?;

    let title = ui.screen.title.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 800.0])
            .with_title(&title),
        ..Default::default()
    };

    eframe::run_native(&title, options, Box::new(|_cc| Ok(Box::new(app))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_state_is_synced_when_branch_appears_and_survives_rebuild() {
        let program = aec_parser::parse(
            "agent Test\nfn reveal() {\n    visible = true\n}\nfn update() {\n    detail = \"updated\"\n}\nui Main = Screen \"Test\" {\n    @visible: bool = false\n    if visible {\n        @detail: string = \"initial\"\n        Text detail\n    }\n}\n",
        ).unwrap();
        let ui = program.items.iter().find_map(|item| match item {
            TopLevelItem::Ui(ui) => Some(ui.clone()),
            _ => None,
        }).unwrap();
        let mut app = AecApp::new(program, ui).unwrap();
        assert!(app.state_var_names.contains("detail"));
        assert!(app.state.get_value("detail").is_none());

        app.execute_event("reveal");
        assert!(matches!(app.state.get_value("visible"), Some(UiValue::Bool(true))));
        assert!(matches!(app.state.get_value("detail"), Some(UiValue::String(text)) if text == "initial"));

        app.execute_event("update");
        assert!(matches!(app.state.get_value("detail"), Some(UiValue::String(text)) if text == "updated"));
    }

    #[test]
    fn qualified_public_component_is_rendered() {
        let library = aec_parser::parse(
            "agent Library\npub component Panel {\n    render {\n        Text \"from module\"\n    }\n}\n",
        )
        .unwrap();
        let mut program = aec_parser::parse(
            "agent Main\nui Main = Screen \"Main\" {\n    lib.Panel\n}\n",
        )
        .unwrap();
        let start = program.items.len();
        let module_count = library.items.len();
        program.items.extend(library.items);
        program.item_modules.resize(start, None);
        program
            .item_modules
            .extend((0..module_count).map(|_| Some("lib".to_string())));
        let ui = program
            .items
            .iter()
            .find_map(|item| match item {
                TopLevelItem::Ui(ui) => Some(ui.clone()),
                _ => None,
            })
            .unwrap();
        let app = AecApp::new(program, ui).unwrap();
        assert!(matches!(
            &app.widgets[0],
            Widget::Container(children)
                if matches!(&children[0], Widget::Text(text, _) if text == "from module")
        ));
    }

    #[test]
    fn parse_color_rejects_non_ascii_without_panicking() {
        assert!(parse_color("€abc").is_none());
        assert!(parse_color("12xz56").is_none());
    }

    #[test]
    fn chatbot_event_adds_messages_and_clears_draft() {
        let program = aec_parser::parse(include_str!("../../../examples/chatbot.aec")).unwrap();
        let ui = program.items.iter().find_map(|item| match item {
            TopLevelItem::Ui(ui) => Some(ui.clone()),
            _ => None,
        }).unwrap();
        let mut app = AecApp::new(program, ui).unwrap();
        app.state.set_string("draft", "Hello".to_string());

        app.execute_event("send_message");

        assert_eq!(app.state.get_string("draft"), "");
        let messages = app.state.get_value("messages").unwrap().as_array();
        assert_eq!(messages.len(), 2);
        assert!(matches!(&messages[0], UiValue::Object(message) if matches!(message.get("content"), Some(UiValue::String(text)) if text == "Hello")));
        assert!(matches!(&messages[1], UiValue::Object(message) if matches!(message.get("content"), Some(UiValue::String(text)) if text == "Echo: Hello")));
    }
}
