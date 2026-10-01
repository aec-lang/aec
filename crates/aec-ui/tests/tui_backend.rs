//! Terminal backend tests.
//!
//! [`render_grid`] needs no terminal, so these assert on the exact characters
//! and colors a user would see. That is the same promise the GUI harness makes
//! with pixels, and it is what keeps the two backends honest about each other.

use aec_ast::{Program, TopLevelItem, UiDecl};
use aec_ui::egui;
use aec_ui::tui::{self, Key};
use aec_ui::AecApp;

fn load(source: &str) -> (Program, UiDecl) {
    let program = aec_parser::parse(source).expect("example must parse");
    let ui = program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Ui(ui) => Some(ui.clone()),
            _ => None,
        })
        .expect("example must declare a ui");
    (program, ui)
}

fn example(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read_to_string(&full)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", full.display()))
}

/// The frame as lines, with trailing blanks removed.
fn lines(grid: &tui::Grid) -> Vec<String> {
    (0..grid.height)
        .map(|row| grid.row_text(row))
        .filter(|row| !row.is_empty())
        .collect()
}

fn grid_of(source: &str) -> tui::Grid {
    let (program, ui) = load(source);
    let mut app = AecApp::new(program, ui).expect("app must build");
    tui::render_grid(&mut app, 80, 24)
}

fn app_of(source: &str) -> AecApp {
    let (program, ui) = load(source);
    AecApp::new(program, ui).expect("app must build")
}

#[test]
fn a_window_shows_its_title_and_body() {
    let grid = grid_of("agent Main\nui Main = Screen \"Terminal\" {\n    Text \"hello\"\n}\n");
    let rows = lines(&grid);
    assert_eq!(rows.first().map(String::as_str), Some("Terminal"));
    assert!(
        rows.iter().any(|row| row == "hello"),
        "body text missing, got {rows:?}"
    );
}

#[test]
fn text_wraps_into_the_available_width() {
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Text \"one two three four five six seven eight nine ten\"\n}\n",
    );
    for row in 0..grid.height {
        let text = grid.row_text(row);
        assert!(
            text.chars().count() <= 80,
            "row {row} overflowed the terminal width: {text:?}"
        );
    }
}

#[test]
fn a_button_is_legible_on_its_own_fill() {
    // Same rule as the GUI backend: a themed fill must not swallow its label.
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Button \"Send\" background: \"#89b4fa\"\n}\n",
    );
    let row = (0..grid.height)
        .map(|row| grid.row_text(row))
        .find(|row| row.contains("Send"))
        .expect("the button label must be painted");
    let column = row.find("Send").unwrap();
    let cell = grid
        .cell(column, lines_to_row(&grid, &row))
        .expect("label cell exists");
    assert_eq!(cell.ch, 'S');
    assert!(cell.bg.is_some(), "the button must paint a background");
    let ratio = tui_contrast(cell.fg, cell.bg);
    assert!(
        ratio >= 4.5,
        "button label contrast is {ratio:.2} on {:?}, which is unreadable",
        cell.bg
    );
}

/// Maps a row's text back to its index, so a found row can be sampled.
fn lines_to_row(grid: &tui::Grid, text: &str) -> usize {
    (0..grid.height)
        .find(|row| grid.row_text(*row) == *text)
        .expect("row must exist")
}

fn tui_contrast(fg: Option<egui::Color32>, bg: Option<egui::Color32>) -> f32 {
    let channel = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = |color: egui::Color32| {
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    };
    let (fg, bg) = (fg.expect("label needs a foreground"), bg.expect("needs a background"));
    let (a, b) = (luminance(fg), luminance(bg));
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn a_card_is_drawn_as_a_closed_box() {
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Card {\n        Text \"inside\"\n    }\n}\n",
    );
    let top = (0..grid.height)
        .map(|row| grid.row_text(row))
        .find(|row| row.starts_with('┌'))
        .expect("a card must open with a corner");
    let bottom = (0..grid.height)
        .map(|row| grid.row_text(row))
        .find(|row| row.starts_with('└'))
        .expect("a card must close with a corner");
    assert!(top.starts_with('┌') && top.contains('┐'), "top edge: {top:?}");
    assert!(
        bottom.starts_with('└') && bottom.contains('┘'),
        "bottom edge: {bottom:?}"
    );
    assert!(
        lines(&grid).iter().any(|row| row.contains("inside")),
        "the card body must be painted"
    );
}

#[test]
fn persian_content_hugs_the_right_edge() {
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Text \"سلام دنیا\"\n}\n",
    );
    let row = (0..grid.height)
        .find(|row| grid.row_text(*row).contains("سلام"))
        .expect("Persian text must be painted");
    let text = grid.row_text(row);
    let start = text.find("سلام").unwrap();
    let used = tui::text_width("سلام دنیا");
    assert_eq!(
        start + used,
        80,
        "RTL text should end at the right edge, it started at column {start} in {text:?}"
    );
}

#[test]
fn a_ltr_row_keeps_its_children_left_to_right() {
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Row {\n        Button \"One\"\n        Button \"Two\"\n    }\n}\n",
    );
    let row = (0..grid.height)
        .map(|row| grid.row_text(row))
        .find(|row| row.contains("One") && row.contains("Two"))
        .expect("both buttons share a row");
    let one = row.find("One").unwrap();
    let two = row.find("Two").unwrap();
    assert!(one < two, "LTR row must place One before Two, got {row:?}");
}

#[test]
fn a_for_loop_repeats_its_body_once_per_item() {
    // A UI state list can only be filled at runtime, so the loop is driven the
    // way a real program drives it: the state is populated, then the tree is
    // rebuilt for rendering.
    let mut app = app_of(
        "agent Main\nui Main = Screen \"S\" {\n    @items = []\n    for item in items {\n        Text item\n    }\n}\n",
    );
    let empty = tui::render_grid(&mut app, 40, 10);
    assert!(
        lines(&empty).len() <= 2,
        "an empty list renders only the title, got {:?}",
        lines(&empty)
    );

    app.state.values.insert(
        "items".to_string(),
        aec_ui::UiValue::Array(vec![
            aec_ui::UiValue::String("a".to_string()),
            aec_ui::UiValue::String("b".to_string()),
            aec_ui::UiValue::String("c".to_string()),
        ]),
    );
    let filled = tui::render_grid(&mut app, 40, 10);
    let painted = lines(&filled);
    for expected in ["a", "b", "c"] {
        assert!(
            painted.iter().any(|row| row == expected),
            "loop body did not render {expected:?}, got {painted:?}"
        );
    }
}

#[test]
fn typing_appends_to_the_bound_input_and_the_grid_shows_it() {
    let mut app = app_of(
        "agent Main\nui Main = Screen \"S\" {\n    @draft: string = \"\"\n    Input placeholder: \"type\" bind value to draft\n}\n",
    );
    tui::focus_input("draft");
    for ch in "hi".chars() {
        assert!(tui::handle_key(&mut app, Key::Char(ch)));
    }
    assert_eq!(app.state.get_string("draft"), "hi");
    let grid = tui::render_grid(&mut app, 40, 6);
    assert!(
        lines(&grid).iter().any(|row| row.contains("hi")),
        "typed text must reach the screen, got {:?}",
        lines(&grid)
    );
}

#[test]
fn backspace_edits_the_focused_input() {
    let mut app = app_of(
        "agent Main\nui Main = Screen \"S\" {\n    @draft: string = \"abc\"\n    Input placeholder: \"p\" bind value to draft\n}\n",
    );
    tui::focus_input("draft");
    assert!(tui::handle_key(&mut app, Key::Backspace));
    assert_eq!(app.state.get_string("draft"), "ab");
    tui::blur_input();
}

#[test]
fn tab_cycles_focus_between_inputs() {
    let mut app = app_of(
        "agent Main\nui Main = Screen \"S\" {\n    @a: string = \"\"\n    @b: string = \"\"\n    Input placeholder: \"p\" bind value to a\n    Input placeholder: \"p\" bind value to b\n}\n",
    );
    tui::blur_input();
    assert!(tui::handle_key(&mut app, Key::Tab));
    assert_eq!(tui::focused_input().as_deref(), Some("a"));
    assert!(tui::handle_key(&mut app, Key::Tab));
    assert_eq!(tui::focused_input().as_deref(), Some("b"));
    assert!(tui::handle_key(&mut app, Key::Tab));
    assert_eq!(
        tui::focused_input().as_deref(),
        Some("a"),
        "focus must wrap around"
    );
    tui::blur_input();
}

#[test]
fn enter_runs_the_program_handler() {
    // The terminal backend must run the same event the GUI runs on click.
    let mut app = app_of(&example("examples/chatbot.aec"));
    tui::focus_input("draft");
    tui::handle_key(&mut app, Key::Char('h'));
    tui::handle_key(&mut app, Key::Char('i'));
    assert!(tui::handle_key(&mut app, Key::Enter));
    let messages = app.state.get_value("messages").unwrap().as_array();
    assert_eq!(messages.len(), 2, "enter must append the exchange");
    tui::blur_input();
}

#[test]
fn a_persian_keystroke_arrives_intact() {
    let mut app = app_of(
        "agent Main\nui Main = Screen \"S\" {\n    @draft: string = \"\"\n    Input placeholder: \"type\" bind value to draft\n}\n",
    );
    tui::focus_input("draft");
    assert!(tui::handle_key(&mut app, Key::Char('س')));
    assert_eq!(app.state.get_string("draft"), "س");
    tui::blur_input();
}

#[test]
fn escape_blurs_the_input_instead_of_quitting() {
    let mut app = app_of(
        "agent Main\nui Main = Screen \"S\" {\n    @a: string = \"\"\n    Input placeholder: \"p\" bind value to a\n}\n",
    );
    tui::focus_input("a");
    assert!(tui::handle_key(&mut app, Key::Escape));
    assert_eq!(tui::focused_input(), None);
    assert!(!tui::handle_key(&mut app, Key::Escape), "escape does nothing when idle");
}

#[test]
fn key_bytes_decode_to_the_right_key() {
    assert_eq!(tui_driver_decode(b"\r"), (Key::Enter, 1));
    assert_eq!(tui_driver_decode(b"\t"), (Key::Tab, 1), "Tab must not decode as a character");
    assert_eq!(tui_driver_decode(b"\x7f"), (Key::Backspace, 1));
    assert_eq!(tui_driver_decode(b"\x1b[3~"), (Key::Delete, 3));
    assert_eq!(tui_driver_decode(b"a"), (Key::Char('a'), 1));
    // Two bytes is one Persian letter, not two characters.
    assert_eq!(tui_driver_decode("س".as_bytes()), (Key::Char('س'), 2));
    assert_eq!(tui_driver_decode("سلام".as_bytes()), (Key::Char('س'), 2));
}

fn tui_driver_decode(bytes: &[u8]) -> (Key, usize) {
    aec_ui::tui_driver::decode_key(bytes).expect("bytes must decode")
}

#[test]
fn the_ansi_output_is_colored_and_resets_at_the_end() {
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Button \"Send\" background: \"#89b4fa\"\n}\n",
    );
    let ansi = tui::grid_to_ansi(&grid);
    assert!(ansi.starts_with("\x1b[H\x1b[2J"), "must home and clear");
    assert!(
        ansi.contains("48;2;137;180;250"),
        "the fill must be emitted as truecolor, got {ansi:?}"
    );
    assert!(
        ansi.contains("38;2;17;17;20"),
        "the contrasting label must be emitted as truecolor, got {ansi:?}"
    );
    assert!(ansi.contains("Send"), "the label must be emitted");
    assert!(ansi.ends_with("\x1b[0m"), "must reset the style afterwards");
}

#[test]
fn a_theme_background_reaches_the_terminal_cells() {
    let grid = grid_of(&example("examples/theme.aec"));
    let themed = grid
        .colored_cells()
        .into_iter()
        .filter(|(_, _, color)| {
            (color.r() as i32 - 0x1e).abs() < 3
                && (color.g() as i32 - 0x1e).abs() < 3
                && (color.b() as i32 - 0x2e).abs() < 3
        })
        .count();
    assert!(
        themed > 20,
        "the theme background should be painted, found {themed} cells"
    );
}

#[test]
fn the_terminal_renders_the_chatbot_example() {
    let grid = grid_of(&example("examples/chatbot.aec"));
    let painted = lines(&grid);
    assert!(
        painted.iter().any(|row| row.contains("Chat")),
        "chat heading missing, got {painted:?}"
    );
    assert!(
        painted.iter().any(|row| row.contains("Send")),
        "the send button is missing, got {painted:?}"
    );
}

#[test]
fn grid_measurement_reports_used_rows_and_colors() {
    let grid = grid_of(
        "agent Main\nui Main = Screen \"S\" {\n    Button \"Send\" background: \"#89b4fa\"\n}\n",
    );
    let stats = tui::grid_stats(&grid);
    assert!(stats.rows_used >= 2, "title plus button: {stats:?}");
    assert!(stats.colored > 0, "the button paints a fill: {stats:?}");
}

#[test]
fn narrow_terminals_do_not_panic_or_overflow() {
    // A 1-column terminal is the worst case for any layout code.
    let source = example("examples/theme.aec");
    for width in 1..=6 {
        for height in 1..=4 {
            let (program, ui) = load(&source);
            let mut app = AecApp::new(program, ui).unwrap();
            let narrow = tui::render_grid(&mut app, width, height);
            assert_eq!(
                narrow.len(),
                width * height,
                "grid must stay exactly {width}x{height}"
            );
        }
    }
}

#[test]
fn character_width_handles_wide_and_combining_marks() {
    assert_eq!(tui::char_width('a'), 1);
    assert_eq!(tui::char_width('س'), 1, "Persian letters are single width");
    assert_eq!(tui::char_width('日'), 2, "CJK is double width");
    assert_eq!(tui::char_width('\u{0301}'), 0, "combining marks take no cell");
    assert_eq!(tui::text_width("سلام"), 4);
    assert_eq!(tui::text_width("日本"), 4);
}
