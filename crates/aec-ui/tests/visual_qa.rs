//! Visual QA that runs on every platform in CI.
//!
//! These tests assert on real rasterized pixels, not on a widget tree. A
//! program can pass every widget-level test and still be unusable because a
//! label is unreadable, text never reaches the screen, or a click does not
//! change what is on display. Each test below renders the same frame path the
//! native window uses and then looks at the result.

use std::collections::HashMap;

use aec_ast::{Program, TopLevelItem, UiDecl};
use aec_ui::egui;
use aec_ui::widgets::UiValue;
use aec_ui::{render_program, HarnessAction, RenderOptions, RenderedFrame};

/// Parses a program and returns it with its first root `ui` declaration.
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

fn render(source: &str, options: &RenderOptions) -> RenderedFrame {
    let (program, ui) = load(source);
    render_program(&program, &ui, options).expect("render must succeed")
}

/// Workspace root, since integration tests run with the crate directory as
/// their working directory.
fn example(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|error| panic!("{} must be readable: {error}", full.display()))
}

fn render_example(path: &str, options: &RenderOptions) -> RenderedFrame {
    render(&example(path), options)
}

/// WCAG relative luminance of one pixel.
fn luminance(pixel: [u8; 4]) -> f32 {
    let channel = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(pixel[0]) + 0.7152 * channel(pixel[1]) + 0.0722 * channel(pixel[2])
}

fn contrast(left: [u8; 4], right: [u8; 4]) -> f32 {
    let (a, b) = (luminance(left), luminance(right));
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Every pixel inside a rectangle given in points, converted to pixel space.
fn pixels_in(frame: &RenderedFrame, rect: egui::Rect) -> Vec<[u8; 4]> {
    let ppp = frame.pixels_per_point;
    let mut pixels = Vec::new();
    let left = (rect.min.x * ppp).floor().max(0.0) as u32;
    let top = (rect.min.y * ppp).floor().max(0.0) as u32;
    let right = ((rect.max.x * ppp).ceil() as u32).min(frame.width);
    let bottom = ((rect.max.y * ppp).ceil() as u32).min(frame.height);
    for y in top..bottom {
        for x in left..right {
            pixels.push(frame.pixel(x, y));
        }
    }
    pixels
}

/// The most frequent color in a region, used to identify a solid fill.
fn dominant_color(pixels: &[[u8; 4]]) -> [u8; 4] {
    let mut counts: HashMap<[u8; 4], usize> = HashMap::new();
    for pixel in pixels {
        *counts.entry(*pixel).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(pixel, _)| pixel)
        .unwrap_or([0, 0, 0, 0])
}

#[test]
fn window_paints_widgets_onto_the_backdrop() {
    let frame = render_example(
        "examples/chatbot.aec",
        &RenderOptions::default(),
    );
    let painted = frame.distinct_pixels([24, 26, 33, 255], 8);
    assert!(
        painted > 5_000,
        "expected a populated window, only {painted} pixels differ from the backdrop\n{}",
        frame.ascii_preview(72)
    );
}

#[test]
fn heading_text_actually_reaches_the_screen() {
    // An empty-but-painted window would still pass the pixel count above, so
    // assert that the heading area holds the high-contrast text color on the
    // dark default background rather than a flat fill.
    let frame = render_example("examples/chatbot.aec", &RenderOptions::default());
    let heading = egui::Rect::from_min_size(
        egui::pos2(0.0, 50.0),
        egui::vec2(300.0, 28.0),
    );
    let pixels = pixels_in(&frame, heading);
    let fill = dominant_color(&pixels);
    let brightest = pixels
        .iter()
        .copied()
        .max_by(|a, b| luminance(*a).partial_cmp(&luminance(*b)).unwrap())
        .unwrap();
    assert!(
        contrast(fill, brightest) > 4.5,
        "the \"Chat\" heading is not readable: fill {fill:?} against text {brightest:?}"
    );
}

#[test]
fn themed_button_label_meets_contrast_requirements() {
    // Regression: `Button "بفرست" background: theme.color.primary` used to draw
    // the theme's light `color.text` token on the light `primary` fill, which
    // is unreadable. The label must now be legible in the real frame.
    let frame = render_example("examples/theme.aec", &RenderOptions::default());
    let button = *frame
        .interactables
        .get("button:بفرست")
        .expect("the send button must be interactive");
    let pixels = pixels_in(&frame, button);
    let fill = dominant_color(&pixels);
    let text = pixels
        .iter()
        .copied()
        .min_by(|a, b| luminance(*a).partial_cmp(&luminance(*b)).unwrap())
        .unwrap();
    assert!(
        contrast(fill, text) >= 4.5,
        "button label is unreadable: fill {fill:?} against label {text:?}, ratio {:.2}\n{}",
        contrast(fill, text),
        frame.ascii_preview(72)
    );
}

#[test]
fn theme_background_token_reaches_the_pixels() {
    // `theme.color.background` is #1e1e2e. If the theme were ignored, the
    // window would keep the harness backdrop and this count would collapse.
    let frame = render_example("examples/theme.aec", &RenderOptions::default());
    let themed = frame.pixels_near([0x1e, 0x1e, 0x2e, 255], 2);
    assert!(
        themed > 50_000,
        "theme background did not reach the screen, only {themed} matching pixels"
    );
}

#[test]
fn persian_text_renders_glyphs_in_a_right_to_left_window() {
    let frame = render_example("examples/theme.aec", &RenderOptions::default());
    // Persian copy sits in the subtitle row; it must produce real ink and it
    // must hug the right edge, which is what RTL layout means.
    let region = egui::Rect::from_min_size(egui::pos2(0.0, 80.0), egui::vec2(640.0, 30.0));
    let pixels = pixels_in(&frame, region);
    let fill = dominant_color(&pixels);
    let is_ink = |pixel: &&[u8; 4]| contrast(fill, **pixel) > 3.0;

    let ink = pixels.iter().filter(is_ink).count();
    assert!(
        ink > 200,
        "expected Persian glyphs in the subtitle row, found {ink} contrasting pixels"
    );

    let midpoint = pixels.len() / 2;
    let left_ink = pixels[..midpoint].iter().filter(is_ink).count();
    let right_ink = pixels[midpoint..].iter().filter(is_ink).count();
    assert!(
        right_ink > left_ink * 4,
        "subtitle ink sits on the left ({left_ink}) more than the right ({right_ink}), so RTL layout is not applied"
    );
}

#[test]
fn typing_and_clicking_repaint_the_window_and_update_state() {
    let empty = render_example("examples/chatbot.aec", &RenderOptions::default());
    assert!(
        empty.state.get_value("messages").unwrap().as_array().is_empty(),
        "the demo starts with no messages"
    );

    let used = render_example(
        "examples/chatbot.aec",
        &RenderOptions {
            actions: vec![
                HarnessAction::Type {
                    target: "draft".to_string(),
                    text: "سلام".to_string(),
                },
                HarnessAction::Click {
                    label: "Send".to_string(),
                },
            ],
            ..Default::default()
        },
    );

    let messages = used.state.get_value("messages").unwrap().as_array();
    assert_eq!(messages.len(), 2, "sending must append user and assistant messages");
    assert!(matches!(
        &messages[0],
        UiValue::Object(message)
            if matches!(message.get("content"), Some(UiValue::String(text)) if text == "سلام")
    ));
    assert!(
        used.state.get_string("draft").is_empty(),
        "the draft must be cleared after sending"
    );
    assert_ne!(
        empty.fingerprint(),
        used.fingerprint(),
        "the conversation must be visible on screen, not only in state"
    );
}

#[test]
fn an_unknown_widget_name_fails_with_a_useful_message() {
    let source = "agent Main\nui Main = Screen \"S\" {\n    Button \"Only\"\n}\n";
    let (program, ui) = load(source);
    let error = render_program(
        &program,
        &ui,
        &RenderOptions {
            actions: vec![HarnessAction::Click {
                label: "Missing".to_string(),
            }],
            ..Default::default()
        },
    )
    .expect_err("clicking a missing button must fail");
    assert!(
        error.contains("button:Missing") && error.contains("button:Only"),
        "the error should name the wanted and the available widgets, got: {error}"
    );
}

#[test]
fn a_runaway_event_loop_is_stopped_instead_of_hanging_ci() {
    // `max_frames` is the guard that keeps a rebuild loop from wedging a build.
    let (program, ui) = load("agent Main\nui Main = Screen \"S\" {\n    Text \"hi\"\n}\n");
    let error = render_program(
        &program,
        &ui,
        &RenderOptions {
            actions: vec![HarnessAction::Wait { frames: 64 }],
            max_frames: 8,
            ..Default::default()
        },
    )
    .expect_err("exceeding the frame budget must fail");
    assert!(error.contains("exceeded"), "got: {error}");
}

#[test]
fn snapshots_are_written_as_decodable_png_files() {
    let frame = render_example("examples/chatbot.aec", &RenderOptions::default());
    let path = std::env::temp_dir().join("aec-visual-qa-snapshot.png");
    frame.write_png(&path).expect("writing a png must succeed");

    let bytes = std::fs::read(&path).expect("the snapshot must exist");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "missing png signature");
    assert!(
        bytes.len() > 1_000,
        "a {} byte png is suspiciously small",
        bytes.len()
    );
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// Platform-tolerance notes for maintainers
// ---------------------------------------------------------------------------
//
// The assertions above are calibrated on the Linux CI runner. Font rasterizers
// differ per platform (FreeType vs CoreText vs DirectWrite), so anti-aliasing
// coverage — and therefore exact pixel counts and contrast ratios — can shift
// slightly between hosts. The thresholds deliberately leave headroom (they
// assert *readable text*, not identical pixels); if a threshold ever trips on
// exactly one platform, compare the printed ascii_preview first and treat a
// systematic drop across all text tests as a font-loading regression, not a
// test bug.
