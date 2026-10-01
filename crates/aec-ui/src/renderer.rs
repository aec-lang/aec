//! Native renderer — egui-based

use crate::widgets::{
    build_widgets_with_themes, InputEdit, ComponentRegistry, EventAction, ResolvedTheme, Themes, UiState,
    UiValue, Widget, WidgetStyle,
};
use aec_ast::{Program, TopLevelItem, UiDecl, UiStatement};
use aec_runtime::{Interpreter, Value};
use eframe::egui;
use std::collections::{HashMap, HashSet};
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
    /// Screen rectangles of interactive widgets from the last frame, keyed by
    /// `button:<label>` or `input:<state-name>`. Used by the visual QA harness.
    pub interactables: HashMap<String, egui::Rect>,
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
        let state_var_names = collect_state_names(&ui.screen.body, &components);

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
            interactables: HashMap::new(),
        })
    }

    pub fn load_fonts(&mut self, ctx: &egui::Context) {
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

    /// Runs an event handler, updates state and rebuilds the widget tree.
    ///
    /// Public so the terminal backend can trigger the same handlers the GUI
    /// does, instead of maintaining a second copy of the semantics.
    pub fn execute_action(&mut self, action: EventAction) {
        let (result, scoped_updates) = {
            let mut interp = self
                .interpreter
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            sync_state_to_interpreter(&self.state, &mut interp, &self.state_var_names);
            let scoped_values = action
                .scope
                .as_deref()
                .map(|scope| visible_scoped_values(&self.state, scope))
                .unwrap_or_default();
            let previous = {
                let mut environment = interp.global.borrow_mut();
                scoped_values
                    .iter()
                    .map(|(name, value)| {
                        let previous = environment.vars.get(name).cloned();
                        environment.set(name.clone(), ui_value_to_aec_value(value));
                        (name.clone(), previous)
                    })
                    .collect::<Vec<_>>()
            };
            let global = interp.global.clone();
            let result = interp.eval_expr(&action.handler, global);
            let updates = {
                let environment = interp.global.borrow();
                scoped_values
                    .iter()
                    .filter_map(|(name, _)| {
                        environment
                            .get(name)
                            .map(|value| (name.clone(), aec_value_to_ui_value(&value)))
                    })
                    .collect::<Vec<_>>()
            };
            let mut environment = interp.global.borrow_mut();
            for (name, value) in previous {
                match value {
                    Some(value) => environment.set(name, value),
                    None => {
                        environment.vars.remove(&name);
                    }
                }
            }
            (result, updates)
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
            for (name, value) in scoped_updates {
                self.state.values.insert(name, value);
            }
        }
        // The handler may have introduced state that only exists inside a
        // branch, so the tree is rebuilt before the new state is pushed back
        // into the interpreter.
        self.rebuild();
        let mut interp = self
            .interpreter
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sync_state_to_interpreter(&self.state, &mut interp, &self.state_var_names);
    }

    /// Applies a keystroke to the bound input state, as the terminal backend
    /// does. Returns true when the keystroke was consumed.
    pub fn type_into(&mut self, target: &str, text: &str) {
        let mut current = self.state.get_string(target);
        current.push_str(text);
        self.state.set_string(target, current);
    }

    /// Handles one editing key against a bound input.
    pub fn edit_input(&mut self, target: &str, edit: InputEdit) -> bool {
        let Some(current) = self.state.get_value(target).cloned() else {
            return false;
        };
        if !matches!(current, UiValue::String(_)) {
            return false;
        }
        let before = current.as_string();
        let after = match edit {
            InputEdit::Insert(text) => format!("{before}{text}"),
            InputEdit::Backspace => {
                let mut chars: Vec<char> = before.chars().collect();
                chars.pop();
                chars.into_iter().collect()
            }
            InputEdit::Delete => {
                let mut chars: Vec<char> = before.chars().collect();
                if !chars.is_empty() {
                    chars.remove(0);
                }
                chars.into_iter().collect()
            }
            InputEdit::Clear => String::new(),
        };
        if after == before {
            return false;
        }
        self.state.set_string(target, after);
        true
    }
}

fn collect_state_names(
    statements: &[UiStatement],
    components: &ComponentRegistry,
) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_state_names_in(statements, components, None, &mut names, &mut HashSet::new());
    names
}

fn collect_state_names_in(
    statements: &[UiStatement],
    components: &ComponentRegistry,
    module: Option<&str>,
    names: &mut HashSet<String>,
    active: &mut HashSet<String>,
) {
    for statement in statements {
        match statement {
            UiStatement::State(state) => {
                names.insert(state.name.name.clone());
            }
            UiStatement::Element(element) => {
                if let Some(children) = &element.children {
                    collect_state_names_in(children, components, module, names, active);
                }
            }
            UiStatement::If(branch) => {
                collect_state_names_in(&branch.then_body, components, module, names, active);
                if let Some(otherwise) = &branch.else_body {
                    collect_state_names_in(otherwise, components, module, names, active);
                }
            }
            UiStatement::For(iteration) => {
                collect_state_names_in(&iteration.body, components, module, names, active);
            }
            UiStatement::Component(component) => {
                let Some((identity, declaration, component_module)) =
                    resolve_component_for_collection(component, module, components)
                else {
                    continue;
                };
                if !active.insert(identity.clone()) {
                    continue;
                }
                if let Some(body) = &declaration.render {
                    collect_state_names_in(
                        body,
                        components,
                        component_module.as_deref(),
                        names,
                        active,
                    );
                }
                active.remove(&identity);
            }
        }
    }
}

fn resolve_component_for_collection<'a>(
    component: &aec_ast::ComponentUse,
    module: Option<&str>,
    components: &'a ComponentRegistry,
) -> Option<(String, &'a aec_ast::ComponentDecl, Option<String>)> {
    let requested = &component.name.name;
    let mut candidates = Vec::new();
    if requested.contains('.') {
        if let Some(module) = module {
            candidates.push(format!("{module}.{requested}"));
            let segments = requested.split('.').collect::<Vec<_>>();
            candidates.push(format!("{}::{}", module, segments.join("::")));
        }
        candidates.push(requested.clone());
        let mut segments = requested.split('.').collect::<Vec<_>>();
        if let Some(name) = segments.pop() {
            candidates.push(format!("{}::{}", segments.join("::"), name));
        }
    } else {
        if let Some(module) = module {
            candidates.push(format!("{module}::{requested}"));
        }
        candidates.push(requested.clone());
    }
    let key = candidates.into_iter().find(|key| components.contains_key(key))?;
    let declaration = components.get(&key)?;
    let component_module = key
        .rsplit_once("::")
        .map(|(module, _)| module.to_string())
        .or_else(|| key.rsplit_once('.').map(|(module, _)| module.to_string()))
        .or_else(|| module.map(str::to_string));
    let identity = component_module
        .as_deref()
        .map(|module| format!("{module}::{}", declaration.name.name))
        .unwrap_or_else(|| declaration.name.name.clone());
    Some((identity, declaration, component_module))
}

fn visible_scoped_values(state: &UiState, scope: &str) -> Vec<(String, UiValue)> {
    let mut values = Vec::new();
    let mut names = HashSet::new();
    let mut current = Some(scope);
    while let Some(scope) = current {
        let prefix = format!("{}::", scope);
        for (key, value) in &state.values {
            if let Some(name) = key.strip_prefix(&prefix) {
                if !name.contains("::") && names.insert(name.to_string()) {
                    values.push((name.to_string(), value.clone()));
                }
            }
        }
        current = scope.rsplit_once('/').map(|(parent, _)| parent);
    }
    values
}

fn sync_state_to_interpreter(
    state: &UiState,
    interp: &mut Interpreter,
    var_names: &HashSet<String>,
) {
    let mut env = interp.global.borrow_mut();
    for name in var_names {
        if let Some(value) = state.values.get(name) {
            env.set(name.clone(), ui_value_to_aec_value(value));
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
        if state.values.contains_key(name) {
            if let Some(value) = env.get(name) {
                state
                    .values
                    .insert(name.clone(), aec_value_to_ui_value(&value));
            }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
}

pub fn text_direction(text: &str) -> TextDirection {
    for character in text.chars() {
        if matches!(
            character,
            '\u{0600}'..='\u{06FF}'
                | '\u{0750}'..='\u{077F}'
                | '\u{08A0}'..='\u{08FF}'
                | '\u{FB50}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
        ) {
            return TextDirection::RightToLeft;
        }
        if character.is_alphabetic() {
            return TextDirection::LeftToRight;
        }
    }
    TextDirection::LeftToRight
}

pub fn is_rtl(text: &str) -> bool {
    text_direction(text) == TextDirection::RightToLeft
}

/// Reads an explicit `direction: "rtl" | "ltr"` style property.
pub fn style_direction(style: &WidgetStyle) -> Option<TextDirection> {
    match style.get_string("direction").as_deref() {
        Some("rtl" | "right-to-left" | "RTL") => Some(TextDirection::RightToLeft),
        Some("ltr" | "left-to-right" | "LTR") => Some(TextDirection::LeftToRight),
        _ => None,
    }
}

/// True when any visible text inside `widgets` is right-to-left.
pub fn widgets_are_rtl(widgets: &[Widget], state: &UiState) -> bool {
    widgets.iter().any(|widget| match widget {
        Widget::Text(text, _) | Widget::Heading(text, _) => is_rtl(text),
        Widget::Button { label, .. } => is_rtl(label),
        Widget::Display { var_name, .. } => is_rtl(&state.get_string(var_name)),
        Widget::Column(children, _)
        | Widget::Row(children, _)
        | Widget::Card(children, _) => widgets_are_rtl(children, state),
        Widget::Container(children) => widgets_are_rtl(children, state),
        Widget::If {
            then_branch,
            else_branch,
            ..
        } => {
            widgets_are_rtl(then_branch, state)
                || else_branch
                    .as_deref()
                    .map(|branch| widgets_are_rtl(branch, state))
                    .unwrap_or(false)
        }
        Widget::For { items, .. } => widgets_are_rtl(items, state),
        _ => false,
    })
}

/// Layout direction for a container: an explicit style wins, otherwise the
/// direction of the text it contains decides, so mixed-language UIs line up.
pub fn container_direction(
    children: &[Widget],
    state: &UiState,
    style: &WidgetStyle,
) -> TextDirection {
    style_direction(style).unwrap_or_else(|| {
        if widgets_are_rtl(children, state) {
            TextDirection::RightToLeft
        } else {
            TextDirection::LeftToRight
        }
    })
}

/// A vertical stack whose items hug the correct horizontal edge.
fn vertical_layout(direction: TextDirection) -> egui::Layout {
    match direction {
        TextDirection::RightToLeft => egui::Layout::top_down(egui::Align::Max),
        TextDirection::LeftToRight => egui::Layout::top_down(egui::Align::Min),
    }
}

/// A horizontal row, ordered right-to-left when the content is RTL.
fn horizontal_layout(direction: TextDirection) -> egui::Layout {
    match direction {
        TextDirection::RightToLeft => egui::Layout::right_to_left(egui::Align::Center),
        TextDirection::LeftToRight => egui::Layout::left_to_right(egui::Align::Center),
    }
}

pub fn style_value_color(value: &aec_ast::StyleValue) -> Option<egui::Color32> {
    match value {
        aec_ast::StyleValue::String(s) => parse_color(s),
        aec_ast::StyleValue::Ident(s) => parse_color(s),
        _ => None,
    }
}

fn make_text(text: &str, style: &WidgetStyle) -> egui::RichText {
    let mut rich_text = egui::RichText::new(text);
    if let Some(color) = style
        .get_string("color")
        .or_else(|| style.get_string("text_color"))
        .and_then(|color| parse_color(&color))
    {
        rich_text = rich_text.color(color);
    }
    if let Some(size) = style_number(style, &["size", "font_size"]) {
        rich_text = rich_text.size(size);
    }
    if let Some(weight) = style
        .get_string("weight")
        .or_else(|| style.get_string("font_weight"))
    {
        match weight.to_ascii_lowercase().as_str() {
            "bold" | "bolder" | "semibold" | "heavy" | "black" | "600" | "700" | "800"
            | "900" => rich_text = rich_text.strong(),
            "light" | "thin" | "100" | "200" | "300" => rich_text = rich_text.weak(),
            "italic" => rich_text = rich_text.italics(),
            _ => {}
        }
    }
    rich_text
}

pub fn style_number(style: &WidgetStyle, names: &[&str]) -> Option<f32> {
    names.iter().find_map(|name| {
        let value = style.properties.get(*name)?;
        let number = match value {
            aec_ast::StyleValue::Int(value) => *value as f32,
            aec_ast::StyleValue::Float(value) => *value as f32,
            aec_ast::StyleValue::String(value) | aec_ast::StyleValue::Ident(value) => {
                parse_dimension(value)?
            }
            aec_ast::StyleValue::Bool(_) => return None,
        };
        (number.is_finite() && number >= 0.0).then_some(number)
    })
}

pub fn parse_dimension(value: &str) -> Option<f32> {
    let value = value.trim();
    let value = value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("pt"))
        .unwrap_or(value)
        .trim();
    value.parse().ok()
}

pub fn style_spacing(style: &WidgetStyle) -> Option<f32> {
    style_number(style, &["spacing", "gap", "item_spacing"])
}

pub fn style_padding(style: &WidgetStyle) -> Option<f32> {
    style_number(style, &["padding", "spacing"])
}

pub fn style_min_width(style: &WidgetStyle) -> Option<f32> {
    style_number(style, &["min_width", "min_width_px", "min_size", "width"])
}

pub fn style_margin(style: &WidgetStyle) -> egui::Margin {
    let all = style_number(style, &["margin", "margin_all"]).unwrap_or(0.0);
    let horizontal = style_number(style, &["margin_horizontal", "margin_x"]).unwrap_or(all);
    let vertical = style_number(style, &["margin_vertical", "margin_y"]).unwrap_or(all);
    egui::Margin {
        left: horizontal,
        right: horizontal,
        top: vertical,
        bottom: vertical,
    }
}

pub fn with_style_margin<R>(
    ui: &mut egui::Ui,
    style: &WidgetStyle,
    render: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::none()
        .inner_margin(style_margin(style))
        .show(ui, render)
}

pub fn screen_style_from_theme(theme: Option<&ResolvedTheme>) -> WidgetStyle {
    let mut style = WidgetStyle::new();
    let Some(theme) = theme else {
        return style;
    };
    for (property, token) in [
        ("color", "color.text"),
        ("size", "text.size"),
        ("weight", "text.weight"),
    ] {
        if let Some(value) = theme.get(token) {
            style.properties.insert(property.to_string(), value.clone());
        }
    }
    for token in ["spacing.lg", "spacing.md", "spacing.default"] {
        if let Some(value) = theme.get(token) {
            style.properties.insert("spacing".to_string(), value.clone());
            break;
        }
    }
    style
}

/// WCAG relative luminance, used to reason about readable color pairs.
pub fn relative_luminance(color: egui::Color32) -> f32 {
    let channel = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// WCAG contrast ratio, from 1.0 (identical) to 21.0 (black on white).
pub fn contrast_ratio(left: egui::Color32, right: egui::Color32) -> f32 {
    let (a, b) = (
        relative_luminance(left),
        relative_luminance(right),
    );
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Keeps a widget's label readable on a fill the program chose itself.
///
/// `Button "…" background: theme.color.primary` paints a colored accent fill,
/// but the theme's `color.text` token is still injected as the label color, so
/// a light `primary` and a light `text` produced an unreadable label. The
/// author's choice is respected whenever it actually reads; when it falls below
/// the WCAG AA ratio, the more readable of the author's color and a plain
/// near-black/near-white wins.
pub fn readable_on(background: egui::Color32, preferred: Option<egui::Color32>) -> Option<egui::Color32> {
    let luminance = relative_luminance(background);
    let fallback = if luminance > 0.179 {
        egui::Color32::from_rgb(17, 17, 20)
    } else {
        egui::Color32::from_rgb(245, 246, 250)
    };
    let Some(preferred) = preferred else {
        return Some(fallback);
    };
    if contrast_ratio(preferred, background) >= MINIMUM_TEXT_CONTRAST {
        return Some(preferred);
    }
    let fallback_ratio = contrast_ratio(fallback, background);
    if fallback_ratio > contrast_ratio(preferred, background) {
        Some(fallback)
    } else {
        Some(preferred)
    }
}

/// WCAG AA for normal text.
const MINIMUM_TEXT_CONTRAST: f32 = 4.5;

pub fn parse_color(s: &str) -> Option<egui::Color32> {
    let value = s.trim().trim_start_matches('#');
    let expanded = match value.len() {
        3 | 4 if value.bytes().all(|byte| byte.is_ascii_hexdigit()) => value
            .bytes()
            .flat_map(|byte| [byte, byte])
            .map(char::from)
            .collect::<String>(),
        6 | 8 if value.bytes().all(|byte| byte.is_ascii_hexdigit()) => value.to_string(),
        _ => String::new(),
    };
    if expanded.len() == 6 || expanded.len() == 8 {
        let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
        let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
        let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
        if expanded.len() == 6 {
            return Some(egui::Color32::from_rgb(r, g, b));
        }
        let a = u8::from_str_radix(&expanded[6..8], 16).ok()?;
        return Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    match value.to_lowercase().as_str() {
        "red" => Some(egui::Color32::RED),
        "green" => Some(egui::Color32::GREEN),
        "blue" => Some(egui::Color32::BLUE),
        "white" => Some(egui::Color32::WHITE),
        "black" => Some(egui::Color32::BLACK),
        "gray" | "grey" => Some(egui::Color32::GRAY),
        "orange" => Some(egui::Color32::from_rgb(255, 159, 26)),
        "yellow" => Some(egui::Color32::from_rgb(255, 212, 59)),
        "purple" => Some(egui::Color32::from_rgb(184, 108, 255)),
        "magenta" | "pink" => Some(egui::Color32::from_rgb(255, 100, 180)),
        "cyan" => Some(egui::Color32::from_rgb(51, 204, 204)),
        "transparent" => Some(egui::Color32::TRANSPARENT),
        _ => None,
    }
}

impl eframe::App for AecApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

impl AecApp {
    /// Renders one frame and runs the AEC handlers triggered by it.
    ///
    /// This is the same code path the native window uses, so the headless
    /// visual QA harness sees exactly what a user would see.
    pub fn frame(&mut self, ctx: &egui::Context) {
        for event in self.draw_frame(ctx) {
            self.execute_action(event);
        }
    }

    /// Rebuilds the widget tree from the current state.
    ///
    /// Both backends call this: the egui renderer before painting, and the
    /// terminal renderer before measuring. Sharing it is what keeps a program
    /// behaving identically no matter which surface it is shown on.
    pub fn rebuild(&mut self) {
        let mut rebuild_state = self.state.clone();
        self.widgets = build_widgets_with_themes(
            &self.ui_decl.screen.body,
            &mut rebuild_state,
            &self.components,
            &self.themes,
        );
        self.state = rebuild_state;
    }

    /// Draws one frame into `ctx` and returns the events raised by widgets.
    pub fn draw_frame(&mut self, ctx: &egui::Context) -> Vec<EventAction> {
        self.load_fonts(ctx);
        self.rebuild();
        let mut pending: Vec<EventAction> = Vec::new();
        let mut targets: HashMap<String, egui::Rect> = HashMap::new();

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
                let screen_style = screen_style_from_theme(self.themes.active());
                let direction = container_direction(&self.widgets, &self.state, &screen_style);
                let widgets = self.widgets.clone();
                ui.with_layout(vertical_layout(direction), |ui| {
                    render_heading(ui, &self.ui_decl.screen.title, &screen_style);
                    ui.add_space(style_spacing(&screen_style).unwrap_or(16.0));
                    render_widgets(ui, &widgets, &mut self.state, &mut pending, &mut targets);
                });
            });
        });

        self.interactables = targets;
        pending
    }
}

fn render_widgets(
    ui: &mut egui::Ui,
    widgets: &[Widget],
    state: &mut UiState,
    pending: &mut Vec<EventAction>,
    targets: &mut HashMap<String, egui::Rect>,
) {
    for widget in widgets {
        render_widget(ui, widget, state, pending, targets);
    }
}

fn render_label(ui: &mut egui::Ui, text: &str, style: &WidgetStyle) {
    with_style_margin(ui, style, |ui| {
        if let Some(min_width) = style_min_width(style) {
            ui.set_min_width(min_width);
        }
        match text_direction(text) {
            TextDirection::RightToLeft => {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.label(make_text(text, style));
                });
            }
            TextDirection::LeftToRight => {
                ui.label(make_text(text, style));
            }
        }
    });
}

fn render_heading(ui: &mut egui::Ui, text: &str, style: &WidgetStyle) {
    with_style_margin(ui, style, |ui| {
        if let Some(min_width) = style_min_width(style) {
            ui.set_min_width(min_width);
        }
        match text_direction(text) {
            TextDirection::RightToLeft => {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.heading(make_text(text, style));
                });
            }
            TextDirection::LeftToRight => {
                ui.heading(make_text(text, style));
            }
        }
    });
}

fn render_widget(
    ui: &mut egui::Ui,
    widget: &Widget,
    state: &mut UiState,
    pending: &mut Vec<EventAction>,
    targets: &mut HashMap<String, egui::Rect>,
) {
    match widget {
        Widget::Column(children, style) => {
            with_style_margin(ui, style, |ui| {
                let mut frame = egui::Frame::default();
                if let Some(background) = style
                    .get_string("background")
                    .or_else(|| style.get_string("background_color"))
                    .and_then(|color| parse_color(&color))
                {
                    frame = frame.fill(background);
                }
                frame.show(ui, |ui| {
                    if let Some(min_width) = style_min_width(style) {
                        ui.set_min_width(min_width);
                    }
                    if let Some(spacing) = style_spacing(style) {
                        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                    }
                    let direction = container_direction(children, state, style);
                    ui.with_layout(vertical_layout(direction), |ui| {
                        render_widgets(ui, children, state, pending, targets);
                    });
                });
            });
        }

        Widget::Row(children, style) => {
            with_style_margin(ui, style, |ui| {
                let mut frame = egui::Frame::default();
                if let Some(background) = style
                    .get_string("background")
                    .or_else(|| style.get_string("background_color"))
                    .and_then(|color| parse_color(&color))
                {
                    frame = frame.fill(background);
                }
                frame.show(ui, |ui| {
                    if let Some(min_width) = style_min_width(style) {
                        ui.set_min_width(min_width);
                    }
                    if let Some(spacing) = style_spacing(style) {
                        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                    }
                    let direction = container_direction(children, state, style);
                    ui.with_layout(horizontal_layout(direction), |ui| {
                        render_widgets(ui, children, state, pending, targets);
                    });
                });
            });
        }

        Widget::Card(children, style) => {
            with_style_margin(ui, style, |ui| {
                let mut frame = egui::Frame::group(ui.style());
                if let Some(background) = style
                    .get_string("background")
                    .or_else(|| style.get_string("background_color"))
                    .and_then(|color| parse_color(&color))
                {
                    frame = frame.fill(background);
                }
                if let Some(radius) = style_number(style, &["radius", "border_radius"]) {
                    frame = frame.rounding(egui::Rounding::same(radius.max(0.0)));
                }
                if let Some(padding) = style_padding(style) {
                    frame = frame.inner_margin(egui::Margin::symmetric(padding, padding));
                }
                frame.show(ui, |ui| {
                    if let Some(min_width) = style_min_width(style) {
                        ui.set_min_width(min_width);
                    }
                    if let Some(spacing) = style_spacing(style) {
                        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                    }
                    render_widgets(ui, children, state, pending, targets);
                });
            });
        }

        Widget::Container(children) => {
            render_widgets(ui, children, state, pending, targets);
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
                let response = ui.add(text_edit);
                targets.insert(
                    format!("input:{}", target),
                    response.rect,
                );
                if response.changed() {
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
            if let Some(label_color) = bg.and_then(|fill| readable_on(fill, text_color)) {
                text = text.color(label_color);
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

            targets.insert(format!("button:{}", label), response.rect);

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
                render_widgets(ui, then_branch, state, pending, targets);
            } else if let Some(else_b) = else_branch {
                render_widgets(ui, else_b, state, pending, targets);
            }
        }

        Widget::For { variable: _, items } => {
            render_widgets(ui, items, state, pending, targets);
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
