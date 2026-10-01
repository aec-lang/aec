//! Terminal backend for AEC.
//!
//! The same widget tree that the egui renderer paints is measured and painted
//! here, into a grid of character cells. The terminal cannot rasterize, so
//! every AEC style is mapped onto the palette the cell format already has:
//! truecolor foreground and background, bold, dim and reverse.
//!
//! Layout direction is honoured by this crate rather than the terminal. A
//! terminal is free to apply bidirectional reordering to whatever it is given,
//! and most do, so text is written in logical order and only the *positioning*
//! is decided here: an RTL container hugs the right edge and lays its row
//! children out from the right.
//!
//! [`render_grid`] is pure and takes no terminal, so tests assert on the exact
//! cells a user would see.

use crate::renderer::{
    container_direction, is_rtl, parse_color, readable_on, screen_style_from_theme, style_min_width,
    style_padding, style_spacing, AecApp, TextDirection,
};
use crate::widgets::{InputEdit, UiState, UiValue, Widget, WidgetStyle};

/// A color that may be left to the terminal.
pub type Paint = Option<egui::Color32>;

/// One character position on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Paint,
    pub bg: Paint,
    pub bold: bool,
    pub dim: bool,
    pub reverse: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            reverse: false,
        }
    }
}

/// A fixed-size character grid: the terminal equivalent of a frame.
#[derive(Debug, Clone)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
        }
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }

    pub fn cell(&self, x: usize, y: usize) -> Option<&Cell> {
        self.index(x, y).map(|index| &self.cells[index])
    }

    /// Number of cells, which is always `width * height`.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn set(&mut self, x: usize, y: usize, cell: Cell) {
        if let Some(index) = self.index(x, y) {
            self.cells[index] = cell;
        }
    }

    /// Paints a solid rectangle, used for cards, buttons and rules.
    pub fn fill(&mut self, x: usize, y: usize, width: usize, height: usize, bg: Paint) {
        for row in y..y + height {
            for column in x..x + width {
                if let Some(index) = self.index(column, row) {
                    let cell = &mut self.cells[index];
                    cell.bg = bg;
                    if cell.ch == ' ' {
                        cell.ch = ' ';
                    }
                }
            }
        }
    }

    /// Writes text left to right, stopping at the right edge.
    ///
    /// Returns how many columns were used, which is the width in cells and not
    /// the number of characters, because wide glyphs take two columns.
    pub fn write(&mut self, x: usize, y: usize, text: &str, style: &CellStyle) -> usize {
        let mut column = x;
        for ch in text.chars() {
            let span = char_width(ch);
            if column + span > self.width {
                break;
            }
            self.set(column, y, style.cell(ch));
            // A wide glyph owns a trailing blank so background fills through.
            for extra in 1..span {
                self.set(column + extra, y, style.cell(' '));
            }
            column += span;
        }
        column - x
    }

    /// Writes text so that it ends at `right`, which is RTL alignment.
    pub fn write_right(&mut self, right: usize, y: usize, text: &str, style: &CellStyle) -> usize {
        let used = text.chars().map(char_width).sum::<usize>();
        if used > right {
            return self.write(0, y, text, style);
        }
        self.write(right - used, y, text, style)
    }

    /// The text of a row, with trailing blanks removed.
    pub fn row_text(&self, y: usize) -> String {
        if y >= self.height {
            return String::new();
        }
        let line = (0..self.width)
            .map(|x| self.cells[y * self.width + x].ch)
            .collect::<String>();
        line.trim_end().to_string()
    }

    /// Row `y` with the first `count` characters removed, for assertions.
    pub fn row_text_from(&self, y: usize, x: usize) -> String {
        if y >= self.height {
            return String::new();
        }
        (x..self.width)
            .map(|column| self.cells[y * self.width + column].ch)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    /// Every cell whose background is not the terminal default.
    pub fn colored_cells(&self) -> Vec<(usize, usize, egui::Color32)> {
        (0..self.height)
            .flat_map(|y| (0..self.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| self.cell(x, y)?.bg.map(|bg| (x, y, bg)))
            .collect()
    }
}

/// Display width of one character in terminal cells.
///
/// Zero for combining marks, two for the East Asian wide ranges, one
/// otherwise. CJK is not the point of the language, but a wrong width would
/// shear the whole layout to its right.
pub fn char_width(ch: char) -> usize {
    let code = ch as u32;
    if code == 0 {
        return 0;
    }
    let combining = matches!(code,
        0x0300..=0x036F
        | 0x0483..=0x0489
        | 0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x0670
        | 0x06D6..=0x06DC
        | 0x200B..=0x200F
        | 0xFE00..=0xFE0F
        | 0xFE20..=0xFE2F
    );
    if combining {
        return 0;
    }
    let wide = matches!(code,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD
    );
    usize::from(wide) + 1
}

/// Width of a string in terminal cells.
pub fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// How a painted run of cells should look.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellStyle {
    pub fg: Paint,
    pub bg: Paint,
    pub bold: bool,
    pub dim: bool,
    pub reverse: bool,
}

impl CellStyle {
    fn cell(&self, ch: char) -> Cell {
        Cell {
            ch,
            fg: self.fg,
            bg: self.bg,
            bold: self.bold,
            dim: self.dim,
            reverse: self.reverse,
        }
    }
}

/// The size a widget asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Size {
    width: usize,
    height: usize,
}

/// Default horizontal padding inside a bordered box.
const BOX_PADDING: usize = 1;
/// Width a bare button reserves around its label.
const BUTTON_PADDING: usize = 2;
/// Width of the input field when the program does not size it.
const DEFAULT_INPUT_WIDTH: usize = 24;

/// Renders the app's current widget tree into a grid.
pub fn render_grid(app: &mut AecApp, width: usize, height: usize) -> Grid {
    app.rebuild();
    let mut grid = Grid::new(width, height);
    let screen = screen_style_from_theme(app.themes.active());
    let direction = container_direction(&app.widgets, &app.state, &screen);

    let title = app.ui_decl.screen.title.clone();
    let title_style = text_style(&screen, true);
    let mut y = 0;
    let title_height = if title.is_empty() {
        0
    } else {
        y = place_text(&mut grid, &title, 0, y, width, direction, &title_style);
        y += style_spacing(&screen).unwrap_or(1.0).ceil() as usize;
        1
    };
    let _ = title_height;

    let body = measure_block(&app.widgets, &app.state, width);
    paint_block(
        &mut grid,
        &app.widgets,
        &app.state,
        0,
        y,
        width,
        &screen,
        &body,
    );
    grid
}

/// Resolves the colors, weight and fill for a piece of text.
fn text_style(style: &WidgetStyle, heading: bool) -> CellStyle {
    let fg = style
        .get_string("color")
        .or_else(|| style.get_string("text_color"))
        .and_then(|value| parse_color(&value));
    let weight = style
        .get_string("weight")
        .or_else(|| style.get_string("font_weight"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    CellStyle {
        fg,
        bg: None,
        bold: heading
            || matches!(
                weight.as_str(),
                "bold" | "bolder" | "semibold" | "heavy" | "black" | "600" | "700" | "800"
                    | "900"
            ),
        dim: matches!(weight.as_str(), "light" | "thin" | "100" | "200" | "300"),
        reverse: false,
    }
}

/// Resolves a background token from a style.
fn background_of(style: &WidgetStyle) -> Paint {
    style
        .get_string("background")
        .or_else(|| style.get_string("background_color"))
        .and_then(|value| parse_color(&value))
}

/// Writes one line of text at the correct edge and returns the next free row.
fn place_text(
    grid: &mut Grid,
    text: &str,
    x: usize,
    y: usize,
    width: usize,
    direction: TextDirection,
    style: &CellStyle,
) -> usize {
    let right = (x + width).min(grid.width);
    match direction {
        TextDirection::RightToLeft => {
            grid.write_right(right, y, text, style);
        }
        TextDirection::LeftToRight => {
            grid.write(x, y, text, style);
        }
    }
    y + 1
}

/// Measures a list of widgets stacked in a column.
fn measure_block(widgets: &[Widget], state: &UiState, width: usize) -> Size {
    let mut total = Size::default();
    for (index, widget) in widgets.iter().enumerate() {
        let child = measure_widget(widget, state, width);
        total.width = total.width.max(child.width);
        total.height += child.height;
        if index + 1 < widgets.len() {
            total.height += 1;
        }
    }
    total
}

fn measure_widget(widget: &Widget, state: &UiState, width: usize) -> Size {
    match widget {
        Widget::Column(children, style) => {
            let inner = (width.saturating_sub(spacing_of(style) * 2)).max(1);
            let mut size = measure_block(children, state, inner);
            if size.height == 0 {
                size.height = 1;
            }
            size
        }
        Widget::Row(children, ..) => {
            let mut total = 0usize;
            let mut tallest = 0usize;
            for (index, child) in children.iter().enumerate() {
                let size = measure_widget(child, state, width.saturating_sub(total));
                total += size.width;
                tallest = tallest.max(size.height);
                if index + 1 < children.len() {
                    total += 1;
                }
            }
            Size {
                width: total.min(width.max(1)),
                height: tallest.max(1),
            }
        }
        Widget::Card(children, ..) => {
            let inner = (width.saturating_sub(2 + BOX_PADDING * 2)).max(1);
            let body = measure_block(children, state, inner);
            Size {
                width: (body.width + 2 + BOX_PADDING * 2).min(width.max(1)),
                height: body.height + 2,
            }
        }
        Widget::Container(children) => measure_block(children, state, width),
        Widget::Text(text, _) | Widget::Heading(text, _) => Size {
            width: text_width(text),
            height: 1,
        },
        Widget::Display { var_name, .. } => Size {
            width: text_width(&state.get_string(var_name)),
            height: 1,
        },
        Widget::Button { label, .. } => Size {
            width: (text_width(label) + BUTTON_PADDING).min(width.max(1)),
            height: 1,
        },
        Widget::Input {
            bind_target: _,
            value,
            placeholder,
            style,
        } => {
            let field = input_width(style, width);
            let content = text_width(value)
                .max(text_width(placeholder.as_deref().unwrap_or("")))
                .min(field.saturating_sub(1));
            Size {
                width: field,
                height: 1,
            }
            .widened(content)
        }
        Widget::MessagesList { source, .. } => {
            let items = state.get_value(source).map(UiValue::as_array).unwrap_or_default();
            Size {
                width,
                height: items.len().max(1),
            }
        }
        Widget::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let branch = if *condition {
                then_branch
            } else {
                else_branch.as_deref().unwrap_or(&[])
            };
            measure_block(branch, state, width)
        }
        Widget::For { items, .. } => measure_block(items, state, width),
        Widget::Divider => Size {
            width,
            height: 1,
        },
        Widget::Spacer => Size {
            width: 0,
            height: 1,
        },
    }
}

trait Sized {
    fn widened(self, width: usize) -> Self;
}

impl Sized for Size {
    fn widened(mut self, width: usize) -> Self {
        self.width = self.width.max(width);
        self
    }
}

fn spacing_of(style: &WidgetStyle) -> usize {
    style_padding(style).unwrap_or(0.0).round().max(0.0) as usize
}

fn input_width(style: &WidgetStyle, available: usize) -> usize {
    let declared = style
        .get_string("width")
        .and_then(|value| value.parse::<f32>().ok());
    let width = declared
        .or_else(|| style_min_width(style))
        .unwrap_or(DEFAULT_INPUT_WIDTH as f32);
    (width.round().max(4.0) as usize).min(available.max(4))
}

/// Paints a list of widgets stacked in a column.
///
/// The argument list is long because a column needs the widget list, the state
/// it reads, its origin, its width, the screen style and the measured size.
#[allow(clippy::too_many_arguments)]
fn paint_block(
    grid: &mut Grid,
    widgets: &[Widget],
    state: &UiState,
    x: usize,
    y: usize,
    width: usize,
    screen: &WidgetStyle,
    size: &Size,
) -> usize {
    let _ = size;
    let mut row = y;
    for (index, widget) in widgets.iter().enumerate() {
        if index > 0 {
            row += 1;
        }
        let child = measure_widget(widget, state, width);
        row = paint_widget(grid, widget, state, x, row, width, screen);
        let _ = child;
    }
    row
}

#[allow(clippy::too_many_arguments)]
fn paint_widget(
    grid: &mut Grid,
    widget: &Widget,
    state: &UiState,
    x: usize,
    y: usize,
    width: usize,
    screen: &WidgetStyle,
) -> usize {
    match widget {
        Widget::Column(children, style) => {
            let inset = spacing_of(style);
            let background = background_of(style);
            if let Some(fill) = background {
                let height = measure_block(children, state, width).height;
                grid.fill(x + inset, y, width.saturating_sub(inset * 2), height, Some(fill));
            }
            let block = measure_block(children, state, width);
            paint_block(
                grid,
                children,
                state,
                x + inset,
                y,
                width.saturating_sub(inset * 2),
                screen,
                &block,
            )
            .max(y + 1)
        }
        Widget::Row(children, style) => {
            let direction = container_direction(children, state, style);
            let background = background_of(style);
            let sizes = children
                .iter()
                .map(|child| measure_widget(child, state, width))
                .collect::<Vec<_>>();
            let tallest = sizes.iter().map(|size| size.height).max().unwrap_or(1).max(1);
            if let Some(fill) = background {
                let total = sizes.iter().map(|size| size.width).sum::<usize>()
                    + sizes.len().saturating_sub(1);
                grid.fill(x, y, total.min(width), tallest, Some(fill));
            }

            // An RTL row starts from the right edge and walks leftwards.
            let total = sizes.iter().map(|size| size.width).sum::<usize>()
                + sizes.len().saturating_sub(1);
            let right = (x + width).min(grid.width);
            let mut cursor = match direction {
                TextDirection::RightToLeft => right.saturating_sub(total),
                TextDirection::LeftToRight => x,
            };

            for (index, child) in children.iter().enumerate() {
                if index > 0 {
                    cursor += 1;
                }
                let size = sizes[index];
                match direction {
                    TextDirection::RightToLeft => {
                        paint_widget(grid, child, state, cursor, y, size.width, screen);
                        cursor += size.width;
                    }
                    TextDirection::LeftToRight => {
                        paint_widget(grid, child, state, cursor, y, size.width, screen);
                        cursor += size.width;
                    }
                }
            }
            (y + tallest).max(y + 1)
        }
        Widget::Card(children, style) => {
            let body = measure_block(children, state, width);
            let inner_width = (body.width + BOX_PADDING * 2).min(width.max(1));
            let border = style
                .get_string("border_color")
                .and_then(|value| parse_color(&value))
                .or_else(|| {
                    style
                        .get_string("color")
                        .and_then(|value| parse_color(&value))
                });
            let frame = CellStyle {
                fg: border,
                bg: background_of(style),
                ..Default::default()
            };
            let left = x;
            let top = y;
            let right = (left + inner_width).min(grid.width.saturating_sub(1));
            let bottom = top + body.height;

            grid.write(left, top, "┌", &frame);
            for column in left + 1..=right {
                grid.set(column, top, frame.cell('─'));
            }
            grid.set(right, top, frame.cell('┐'));
            for row in top + 1..=bottom {
                grid.set(left, row, frame.cell('│'));
                for column in left + 1..=right {
                    grid.set(column, row, frame.cell(' '));
                }
                grid.set(right, row, frame.cell('│'));
            }
            if bottom < grid.height {
                grid.write(left, bottom, "└", &frame);
                for column in left + 1..=right {
                    grid.set(column, bottom, frame.cell('─'));
                }
                grid.set(right, bottom, frame.cell('┘'));
            }

            let block = measure_block(children, state, width);
            paint_block(
                grid,
                children,
                state,
                left + 1 + BOX_PADDING,
                top + 1,
                width.saturating_sub(2 + BOX_PADDING * 2),
                screen,
                &block,
            );
            bottom + 1
        }
        Widget::Container(children) => {
            let block = measure_block(children, state, width);
            paint_block(grid, children, state, x, y, width, screen, &block).max(y)
        }
        Widget::Text(text, widget_style) => {
            let align = direction_for(text, widget_style);
            let style = text_style(widget_style, false);
            place_text(grid, text, x, y, width, align, &style)
        }
        Widget::Heading(text, widget_style) => {
            let align = direction_for(text, widget_style);
            let mut style = text_style(widget_style, true);
            style.fg = style
                .fg
                .or_else(|| screen.get_string("color").and_then(|value| parse_color(&value)));
            place_text(grid, text, x, y, width, align, &style)
        }
        Widget::Display { var_name, style } => {
            let value = state.get_string(var_name);
            let align = direction_for(&value, style);
            let style = text_style(style, false);
            place_text(grid, &value, x, y, width, align, &style)
        }
        Widget::Button { label, style, .. } => {
            let fill = background_of(style);
            let preferred = style
                .get_string("color")
                .or_else(|| style.get_string("text_color"))
                .and_then(|value| parse_color(&value));
            let foreground = fill.and_then(|background| readable_on(background, preferred));
            let mut cell_style = text_style(style, false);
            if let Some(foreground) = foreground {
                cell_style.fg = Some(foreground);
            }
            let pad = BUTTON_PADDING / 2;
            let used = text_width(label) + BUTTON_PADDING;
            let left = match direction_for(label, style) {
                TextDirection::RightToLeft => (x + width).saturating_sub(used),
                TextDirection::LeftToRight => x,
            };
            cell_style.bg = fill;
            grid.write(left, y, &" ".repeat(pad), &cell_style);
            grid.write(left + pad, y, label, &cell_style);
            grid.write(left + pad + text_width(label), y, &" ".repeat(pad), &cell_style);
            y + 1
        }
        Widget::Input {
            bind_target,
            placeholder,
            value,
            style,
        } => {
            let field_width = input_width(style, width);
            let fill = background_of(style);
            let preferred = style
                .get_string("color")
                .or_else(|| style.get_string("text_color"))
                .and_then(|value| parse_color(&value));
            let mut cell_style = text_style(style, false);
            if let Some(background) = fill {
                cell_style.bg = Some(background);
                if let Some(foreground) = readable_on(background, preferred) {
                    cell_style.fg = Some(foreground);
                }
            }
            let caret = bind_target.as_deref().is_some_and(is_focused);
            let shown = if value.is_empty() {
                placeholder.clone().unwrap_or_default()
            } else {
                value.clone()
            };
            let inner = field_width.saturating_sub(1).max(1);
            let clipped = truncate(&shown, inner);
            let empty = value.is_empty();
            let mut body = cell_style.clone();
            if empty {
                body.dim = true;
            }
            grid.write(x, y, &clipped, &body);
            let caret_cell = Cell {
                ch: if caret { '▏' } else { ' ' },
                fg: cell_style.fg,
                bg: cell_style.bg,
                bold: caret,
                dim: false,
                reverse: caret,
            };
            grid.set(x + text_width(&clipped), y, caret_cell);
            for column in (x + text_width(&clipped) + 1)..(x + field_width) {
                if column < grid.width {
                    grid.set(column, y, cell_style.cell(' '));
                }
            }
            y + 1
        }
        Widget::MessagesList { source, style } => {
            let items = state
                .get_value(source)
                .map(UiValue::as_array)
                .unwrap_or_default();
            let mut row = y;
            if items.is_empty() {
                let muted = CellStyle {
                    fg: screen.get_string("color").and_then(|v| parse_color(&v)),
                    dim: true,
                    ..Default::default()
                };
                row = place_text(grid, "no messages yet", x, row, width, TextDirection::LeftToRight, &muted);
                return row;
            }
            for item in items {
                let UiValue::Object(fields) = item else {
                    row += 1;
                    continue;
                };
                let role = fields
                    .get("role")
                    .map(UiValue::as_string)
                    .unwrap_or_default();
                let content = fields
                    .get("content")
                    .map(UiValue::as_string)
                    .unwrap_or_default();
                let label = format!("{role}: {content}");
                let truncated = truncate(&label, width);
                row = place_text(
                    grid,
                    &truncated,
                    x,
                    row,
                    width,
                    direction_for(&truncated, style),
                    &CellStyle::default(),
                );
            }
            row
        }
        Widget::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let branch = if *condition {
                then_branch
            } else {
                else_branch.as_deref().unwrap_or(&[])
            };
            let block = measure_block(branch, state, width);
            paint_block(grid, branch, state, x, y, width, screen, &block).max(y)
        }
        Widget::For { items, .. } => {
            let block = measure_block(items, state, width);
            paint_block(grid, items, state, x, y, width, screen, &block).max(y)
        }
        Widget::Divider => {
            let rule = CellStyle {
                fg: screen.get_string("color").and_then(|value| parse_color(&value)),
                dim: true,
                ..Default::default()
            };
            for column in x..(x + width).min(grid.width) {
                grid.set(column, y, rule.cell('─'));
            }
            y + 1
        }
        Widget::Spacer => y + 1,
    }
}

fn direction_for(text: &str, style: &WidgetStyle) -> TextDirection {
    match style.get_string("direction").map(|value| value.to_ascii_lowercase()) {
        Some(value) if value == "rtl" || value == "right-to-left" => TextDirection::RightToLeft,
        Some(value) if value == "ltr" || value == "left-to-right" => TextDirection::LeftToRight,
        _ => {
            if is_rtl(text) {
                TextDirection::RightToLeft
            } else {
                TextDirection::LeftToRight
            }
        }
    }
}

/// Shortens text to at most `width` cells.
pub fn truncate(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let span = char_width(ch);
        if used + span > width {
            break;
        }
        out.push(ch);
        used += span;
    }
    out
}

thread_local! {
    /// Which bound input the terminal user is typing into.
    static FOCUS: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn is_focused(target: &str) -> bool {
    FOCUS.with(|focus| focus.borrow().as_deref() == Some(target))
}

/// The bound input that keystrokes currently go to.
pub fn focused_input() -> Option<String> {
    FOCUS.with(|focus| focus.borrow().clone())
}

/// Moves keyboard focus, so Tab cycles between bound inputs.
pub fn focus_input(target: &str) {
    FOCUS.with(|focus| *focus.borrow_mut() = Some(target.to_string()));
}

pub fn blur_input() {
    FOCUS.with(|focus| *focus.borrow_mut() = None);
}

/// The bound inputs of the current tree, in order, for Tab cycling.
pub fn focusable_inputs(app: &AecApp) -> Vec<String> {
    let mut found = Vec::new();
    collect_inputs(&app.widgets, &mut found);
    found
}

fn collect_inputs(widgets: &[Widget], out: &mut Vec<String>) {
    for widget in widgets {
        match widget {
            Widget::Input {
                bind_target: Some(target),
                ..
            } => {
                if !out.contains(target) {
                    out.push(target.clone());
                }
            }
            Widget::Column(children, _)
            | Widget::Row(children, _)
            | Widget::Card(children, _)
            | Widget::Container(children) => collect_inputs(children, out),
            Widget::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_inputs(then_branch, out);
                if let Some(branch) = else_branch {
                    collect_inputs(branch, out);
                }
            }
            Widget::For { items, .. } => collect_inputs(items, out),
            _ => {}
        }
    }
}

/// Applies one key to the app, mirroring what a user would do.
///
/// Returns true when the key was handled, so the driver can decide whether the
/// frame needs repainting.
pub fn handle_key(app: &mut AecApp, key: Key) -> bool {
    match key {
        Key::Tab => {
            let targets = focusable_inputs(app);
            if targets.is_empty() {
                return false;
            }
            let next = match focused_input() {
                None => 0,
                Some(current) => match targets.iter().position(|target| *target == current) {
                    Some(index) => (index + 1) % targets.len(),
                    None => 0,
                },
            };
            focus_input(&targets[next]);
            true
        }
        Key::Escape => {
            if focused_input().is_some() {
                blur_input();
                true
            } else {
                false
            }
        }
        Key::Backspace => match focused_input() {
            Some(target) => app.edit_input(&target, InputEdit::Backspace),
            None => false,
        },
        Key::Delete => match focused_input() {
            Some(target) => app.edit_input(&target, InputEdit::Delete),
            None => false,
        },
        Key::Enter => submit_focused(app),
        Key::Char(ch) => match focused_input() {
            Some(target) => {
                app.type_into(&target, &ch.to_string());
                true
            }
            None => false,
        },
    }
}

/// Triggers the first button, which is what Enter does with nothing focused.
fn submit_focused(app: &mut AecApp) -> bool {
    let Some(action) = first_button_action(&app.widgets) else {
        return false;
    };
    app.execute_action(action.clone());
    true
}

fn first_button_action(widgets: &[Widget]) -> Option<crate::widgets::EventAction> {
    for widget in widgets {
        match widget {
            Widget::Button {
                on_click: Some(action),
                ..
            } => return Some(action.clone()),
            Widget::Column(children, _)
            | Widget::Row(children, _)
            | Widget::Card(children, _)
            | Widget::Container(children) => {
                if let Some(action) = first_button_action(children) {
                    return Some(action);
                }
            }
            Widget::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(action) = first_button_action(then_branch) {
                    return Some(action);
                }
                if let Some(action) = else_branch.as_deref().and_then(first_button_action) {
                    return Some(action);
                }
            }
            Widget::For { items, .. } => {
                if let Some(action) = first_button_action(items) {
                    return Some(action);
                }
            }
            _ => {}
        }
    }
    None
}

/// A key the terminal backend understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Tab,
    Escape,
}

/// Renders a grid as an ANSI escape sequence string.
///
/// Unchanged cells emit nothing, so a redraw only repaints what moved.
pub fn grid_to_ansi(grid: &Grid) -> String {
    let mut out = String::with_capacity(grid.width * grid.height);
    out.push_str("\x1b[H\x1b[2J");
    // Only the *style* is tracked between cells. Comparing whole cells would
    // re-emit an escape sequence for every character, since each one differs.
    let mut active: Option<CellStyle> = None;
    for y in 0..grid.height {
        for x in 0..grid.width {
            let cell = grid.cell(x, y).cloned().unwrap_or_default();
            let style = CellStyle {
                fg: cell.fg,
                bg: cell.bg,
                bold: cell.bold,
                dim: cell.dim,
                reverse: cell.reverse,
            };
            if active.as_ref() != Some(&style) {
                out.push_str(&style_sequence(&style));
                active = Some(style);
            }
            out.push(cell.ch);
        }
        if y + 1 < grid.height {
            out.push_str("\r\n");
        }
    }
    out.push_str("\x1b[0m");
    out
}

fn style_sequence(cell: &CellStyle) -> String {
    let mut sequence = String::from("\x1b[0");
    if cell.bold {
        sequence.push_str(";1");
    }
    if cell.dim {
        sequence.push_str(";2");
    }
    if cell.reverse {
        sequence.push_str(";7");
    }
    if let Some(fg) = cell.fg {
        sequence.push_str(&format!(
            ";38;2;{};{};{}",
            fg.r(),
            fg.g(),
            fg.b()
        ));
    }
    if let Some(bg) = cell.bg {
        sequence.push_str(&format!(
            ";48;2;{};{};{}",
            bg.r(),
            bg.g(),
            bg.b()
        ));
    }
    sequence.push('m');
    sequence
}

/// Diagnostic counters, handy in tests and in the driver's status line.
pub fn grid_stats(grid: &Grid) -> GridStats {
    let mut stats = GridStats {
        colored: grid.colored_cells().len(),
        rows_used: 0,
    };
    for y in 0..grid.height {
        let row = grid.row_text(y);
        if !row.trim().is_empty() {
            stats.rows_used += 1;
        }
    }
    stats
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridStats {
    pub colored: usize,
    pub rows_used: usize,
}
