//! Headless visual QA harness.
//!
//! Renders the real egui output of an AEC program without a window server,
//! rasterizes it into RGBA pixels and can drive synthetic input (clicks and
//! typing). This turns "the window looked fine" into an assertion that runs in
//! CI on every platform.

use std::collections::HashMap;
use std::path::Path;

use aec_ast::{Program, UiDecl};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use egui::epaint::{self, ClippedPrimitive, ImageData, Primitive, TextureId};

use crate::renderer::AecApp;

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One synthetic input step for [`render_program`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessAction {
    /// Press and release the button whose visible label matches.
    Click { label: String },
    /// Focus the input bound to `target` and append `text` to it.
    Type { target: String, text: String },
    /// Render `frames` extra frames, for example to let a rebuild settle.
    Wait { frames: usize },
}

/// Rendering options for [`render_program`].
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Logical size of the virtual window, in points.
    pub size: Vec2,
    /// Device pixels per point; `2.0` renders a retina-like image.
    pub pixels_per_point: f32,
    /// Backdrop used before the UI paints anything.
    pub background: Color32,
    /// Synthetic input, applied in order after the first frames settle.
    pub actions: Vec<HarnessAction>,
    /// Upper bound on rendered frames; guards against accidental loops.
    pub max_frames: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            size: Vec2::new(640.0, 720.0),
            pixels_per_point: 1.0,
            background: Color32::from_rgb(24, 26, 33),
            actions: Vec::new(),
            max_frames: 240,
        }
    }
}

/// A rendered frame plus the widget rectangles that were painted into it.
#[derive(Debug, Clone)]
pub struct RenderedFrame {
    pub width: u32,
    pub height: u32,
    /// Non-premultiplied RGBA8 pixels, row by row, top to bottom.
    pub pixels: Vec<u8>,
    /// `button:<label>` / `input:<state-name>` to screen rectangle, in points.
    pub interactables: HashMap<String, Rect>,
    /// Final UI state, so tests can assert what the pixels were showing.
    pub state: crate::widgets::UiState,
    /// How many frames were rendered.
    pub frames: usize,
    /// Device pixels per point, so callers can map a point-space `Rect` back
    /// onto the pixel buffer.
    pub pixels_per_point: f32,
}

impl RenderedFrame {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let offset = ((y as usize * self.width as usize) + x as usize) * 4;
        [
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ]
    }

    /// Counts pixels that differ from the given backdrop by more than `tolerance`.
    pub fn distinct_pixels(&self, background: [u8; 4], tolerance: i32) -> usize {
        let mut count = 0;
        for chunk in self.rgba_chunks() {
            let delta = (0..4)
                .map(|channel| (chunk[channel] as i32 - background[channel] as i32).abs())
                .max()
                .unwrap_or(0);
            if delta > tolerance {
                count += 1;
            }
        }
        count
    }

    /// Counts pixels close to `target`, used to assert that a theme token or
    /// widget color really reached the screen.
    pub fn pixels_near(&self, target: [u8; 4], tolerance: i32) -> usize {
        let mut count = 0;
        for chunk in self.rgba_chunks() {
            let within = (0..4)
                .all(|channel| (chunk[channel] as i32 - target[channel] as i32).abs() <= tolerance);
            if within {
                count += 1;
            }
        }
        count
    }

    /// Total absolute channel difference, a cheap perceptual fingerprint used to
    /// prove that an interaction actually changed what is on screen.
    pub fn fingerprint(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in &self.pixels {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    /// Iterates the buffer four bytes at a time.
    ///
    /// Written by hand instead of with `as_chunks` because the workspace MSRV
    /// is 1.75 and `as_chunks` only stabilized in 1.88.
    fn rgba_chunks(&self) -> impl Iterator<Item = &[u8]> {
        self.pixels.chunks(4)
    }

    /// Coarse ASCII preview, attached to assertion failures.
    pub fn ascii_preview(&self, columns: usize) -> String {
        let rows = (columns * self.height as usize / self.width.max(1) as usize).max(1);
        let mut out = String::new();
        for row in 0..rows {
            for column in 0..columns {
                let x = (column * self.width as usize / columns).min(self.width as usize - 1);
                let y = (row * self.height as usize / rows).min(self.height as usize - 1);
                let [r, g, b, _] = self.pixel(x as u32, y as u32);
                let luminance = (r as u32 * 3 + g as u32 * 6 + b as u32) / 10;
                out.push(match luminance {
                    0..=24 => ' ',
                    25..=64 => '.',
                    65..=110 => '+',
                    111..=170 => '*',
                    _ => '#',
                });
            }
            out.push('\n');
        }
        out
    }

    /// Writes the frame as a PNG file.
    pub fn write_png(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
        let writer = std::io::BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        let mut png_writer = encoder.write_header().map_err(|error| error.to_string())?;
        png_writer
            .write_image_data(&self.pixels)
            .map_err(|error| error.to_string())?;
        png_writer.finish().map_err(|error| error.to_string())?;
        Ok(())
    }
}

/// Renders an AEC program offscreen and returns the final frame.
///
/// The whole `AecApp` frame path is used, so the pixels are the ones a native
/// window would show for the same state.
pub fn render_program(
    program: &Program,
    ui: &UiDecl,
    options: &RenderOptions,
) -> Result<RenderedFrame, String> {
    let mut app = AecApp::new(program.clone(), ui.clone())?;
    let context = egui::Context::default();
    let ppp = options.pixels_per_point;
    context.set_pixels_per_point(ppp);

    let screen = Rect::from_min_size(Pos2::ZERO, options.size);
    let mut input = egui::RawInput {
        screen_rect: Some(screen),
        ..Default::default()
    };
    let mut clock = 0.0f64;
    let mut atlas: Option<FontAtlas> = None;
    let mut frames = 0usize;
    let mut shapes: Vec<egui::epaint::ClippedShape> = Vec::new();

    let mut step = |app: &mut AecApp, input: &mut egui::RawInput| -> Result<(), String> {
        frames += 1;
        if frames > options.max_frames {
            return Err(format!(
                "visual harness exceeded {} frames; is an event looping?",
                options.max_frames
            ));
        }
        clock += 1.0 / 60.0;
        input.time = Some(clock);
        let output = context.run(input.clone(), |ctx| {
            RUNS.with(|runs| runs.set(runs.get() + 1));
            app.frame(ctx)
        });
        context.request_repaint();
        for (texture, delta) in output.textures_delta.set {
            if texture != TextureId::Managed(0) {
                continue;
            }
            if let ImageData::Font(font_image) = delta.image {
                let atlas = atlas.get_or_insert_with(|| FontAtlas::new(font_image.size));
                atlas.apply(&font_image, delta.pos);
            }
        }
        shapes = output.shapes;
        Ok(())
    };

    // Two warm-up frames so layout, fonts and the first rebuild have happened.
    step(&mut app, &mut input)?;
    step(&mut app, &mut input)?;

    for action in options.actions.clone() {
        let action = match action {
            HarnessAction::Wait { frames: extra } => {
                for _ in 0..extra.max(1) {
                    step(&mut app, &mut input)?;
                }
                continue;
            }
            other => other,
        };
        let (key, kind) = match &action {
            HarnessAction::Click { label } => (format!("button:{label}"), 'b'),
            HarnessAction::Type { target, .. } => (format!("input:{target}"), 'i'),
            HarnessAction::Wait { .. } => unreachable!(),
        };
        let rect = *app
            .interactables
            .get(&key)
            .ok_or_else(|| {
                let known: Vec<&String> = app.interactables.keys().collect();
                format!("no interactive widget named {key}; found {known:?}")
            })?;
        let center = rect.center();
        input.events.push(egui::Event::PointerMoved(center));
        input.events.push(egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        step(&mut app, &mut input)?;
        input.events.clear();
        input.events.push(egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        step(&mut app, &mut input)?;
        if let HarnessAction::Type { text, .. } = &action {
            let _ = kind;
            input.events.clear();
            input.events.push(egui::Event::Text(text.clone()));
            step(&mut app, &mut input)?;
        }
        input.events.clear();
        // Let the event handler and the reactive rebuild finish.
        step(&mut app, &mut input)?;
    }

    // Final settle frame with no pending input, then paint it.
    input.events.clear();
    let last = context.run(input.clone(), |ctx| app.frame(ctx));
    frames += 1;
    shapes = last.shapes;

    let primitives = context.tessellate(shapes, ppp);
    let (width, height) = (
        (options.size.x * ppp).round().max(1.0) as usize,
        (options.size.y * ppp).round().max(1.0) as usize,
    );
    let pixels = paint(&primitives, atlas.as_ref(), width, height, ppp, options.background);

    Ok(RenderedFrame {
        width: width as u32,
        height: height as u32,
        pixels,
        interactables: app.interactables.clone(),
        state: app.state.clone(),
        frames,
        pixels_per_point: ppp,
    })
}

struct FontAtlas {
    size: [usize; 2],
    coverage: Vec<f32>,
}

impl FontAtlas {
    fn new(size: [usize; 2]) -> Self {
        Self {
            size,
            coverage: vec![0.0; size[0] * size[1]],
        }
    }

    /// Applies a full or partial font atlas update, the way a GPU backend would.
    fn apply(&mut self, font_image: &epaint::FontImage, pos: Option<[usize; 2]>) {
        let (patch_w, patch_h) = (font_image.size[0], font_image.size[1]);
        match pos {
            None => {
                self.size = [patch_w, patch_h];
                self.coverage = font_image.pixels.clone();
            }
            Some(origin) => {
                if self.size[0] < origin[0] + patch_w || self.size[1] < origin[1] + patch_h {
                    let size = [
                        self.size[0].max(origin[0] + patch_w),
                        self.size[1].max(origin[1] + patch_h),
                    ];
                    let mut grown = vec![0.0; size[0] * size[1]];
                    for y in 0..self.size[1] {
                        for x in 0..self.size[0] {
                            grown[y * size[0] + x] = self.coverage[y * self.size[0] + x];
                        }
                    }
                    self.size = size;
                    self.coverage = grown;
                }
                for y in 0..patch_h {
                    for x in 0..patch_w {
                        let target = (origin[1] + y) * self.size[0] + (origin[0] + x);
                        self.coverage[target] = font_image.pixels[y * patch_w + x];
                    }
                }
            }
        }
    }

    fn sample(&self, uv: Pos2) -> f32 {
        if self.size[0] == 0 || self.size[1] == 0 {
            return 1.0;
        }
        let x = (uv.x as f64 * (self.size[0] - 1) as f64).clamp(0.0, self.size[0] as f64 - 1.0);
        let y = (uv.y as f64 * (self.size[1] - 1) as f64).clamp(0.0, self.size[1] as f64 - 1.0);
        let at = |ix: usize, iy: usize| self.coverage[iy * self.size[0] + ix];
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = (
            (x0 + 1).min(self.size[0] - 1),
            (y0 + 1).min(self.size[1] - 1),
        );
        let (tx, ty) = (x as f32 - x0 as f32, y as f32 - y0 as f32);
        let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
        let bottom = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

/// Rasterizes tessellated egui primitives into non-premultiplied RGBA8 pixels.
fn paint(
    primitives: &[ClippedPrimitive],
    atlas: Option<&FontAtlas>,
    width: usize,
    height: usize,
    ppp: f32,
    background: Color32,
) -> Vec<u8> {
    let bg = [
        background.r() as f32 / 255.0,
        background.g() as f32 / 255.0,
        background.b() as f32 / 255.0,
        background.a() as f32 / 255.0,
    ];
    // Premultiplied float accumulation buffer.
    let mut buffer = vec![0.0f32; width * height * 4];
    for index in (0..buffer.len()).step_by(4) {
        buffer[index..index + 4].copy_from_slice(&bg);
    }

    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        let is_font = matches!(mesh.texture_id, TextureId::Managed(0));
        let clip = primitive.clip_rect;
        let clip_min = (
            (clip.min.x * ppp).floor().max(0.0) as usize,
            (clip.min.y * ppp).floor().max(0.0) as usize,
        );
        let clip_max = (
            ((clip.max.x * ppp).ceil().max(0.0) as usize).min(width),
            ((clip.max.y * ppp).ceil().max(0.0) as usize).min(height),
        );
        if clip_min.0 >= clip_max.0 || clip_min.1 >= clip_max.1 {
            continue;
        }
        for triangle in mesh.indices.chunks(3) {
            let Some(vertices) = triangle
                .iter()
                .map(|index| mesh.vertices.get(*index as usize))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            rasterize_triangle(
                vertices[0],
                vertices[1],
                vertices[2],
                if is_font { atlas } else { None },
                &mut buffer,
                width,
                height,
                ppp,
                clip_min,
                clip_max,
            );
        }
    }

    let mut out = Vec::with_capacity(width * height * 4);
    for index in (0..buffer.len()).step_by(4) {
        let pixel = &buffer[index..index + 4];
        let alpha = pixel[3].clamp(0.0, 1.0);
        for &channel in &pixel[..3] {
            let value = if alpha > 0.0 {
                (channel / alpha).clamp(0.0, 1.0)
            } else {
                0.0
            };
            out.push((value * 255.0).round() as u8);
        }
        out.push((alpha * 255.0).round() as u8);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn rasterize_triangle(
    a: &epaint::Vertex,
    b: &epaint::Vertex,
    c: &epaint::Vertex,
    atlas: Option<&FontAtlas>,
    buffer: &mut [f32],
    width: usize,
    height: usize,
    ppp: f32,
    clip_min: (usize, usize),
    clip_max: (usize, usize),
) {
    let p0 = (a.pos.x as f64 * ppp as f64, a.pos.y as f64 * ppp as f64);
    let p1 = (b.pos.x as f64 * ppp as f64, b.pos.y as f64 * ppp as f64);
    let p2 = (c.pos.x as f64 * ppp as f64, c.pos.y as f64 * ppp as f64);
    let area = edge(p0, p1, p2);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    let min_x = (p0.0.min(p1.0).min(p2.0).floor().max(0.0) as usize)
        .max(clip_min.0)
        .min(width);
    let max_x = ((p0.0.max(p1.0).max(p2.0).ceil().max(0.0) as usize) + 1)
        .min(width)
        .min(clip_max.0);
    let min_y = (p0.1.min(p1.1).min(p2.1).floor().max(0.0) as usize)
        .max(clip_min.1)
        .min(height);
    let max_y = ((p0.1.max(p1.1).max(p2.1).ceil().max(0.0) as usize) + 1)
        .min(height)
        .min(clip_max.1);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let point = (x as f64 + 0.5, y as f64 + 0.5);
            let w0 = edge(p1, p2, point) / area;
            let w1 = edge(p2, p0, point) / area;
            let w2 = edge(p0, p1, point) / area;
            if !(w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) {
                continue;
            }
            let uv = Pos2::new(
                lerp(w0, a.uv.x, w1, b.uv.x, w2, c.uv.x),
                lerp(w0, a.uv.y, w1, b.uv.y, w2, c.uv.y),
            );
            let coverage = match atlas {
                Some(atlas) => atlas.sample(uv),
                None => 1.0,
            };
            let color = a.color.to_normalized_gamma_f32();
            let source = [
                color[0] * color[3] * coverage,
                color[1] * color[3] * coverage,
                color[2] * color[3] * coverage,
                color[3] * coverage,
            ];
            if source[3] <= 0.0 {
                continue;
            }
            let offset = (y * width + x) * 4;
            for channel in 0..4 {
                buffer[offset + channel] =
                    source[channel] + buffer[offset + channel] * (1.0 - source[3]);
            }
        }
    }
}

fn edge(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    (p.0 - a.0) * (b.1 - a.1) - (p.1 - a.1) * (b.0 - a.0)
}

fn lerp(w0: f64, v0: f32, w1: f64, v1: f32, w2: f64, v2: f32) -> f32 {
    (w0 * v0 as f64 + w1 * v1 as f64 + w2 * v2 as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use aec_ast::TopLevelItem;

    fn render(source: &str, options: &RenderOptions) -> RenderedFrame {
        let program = aec_parser::parse(source).expect("program parses");
        let ui = program
            .items
            .iter()
            .find_map(|item| match item {
                TopLevelItem::Ui(ui) => Some(ui.clone()),
                _ => None,
            })
            .expect("program has a ui block");
        render_program(&program, &ui, options).expect("renders")
    }

    #[test]
    fn paints_widgets_onto_the_backdrop() {
        let frame = render(
            "agent Qa\nui Main = Screen \"QA\" {\n    Heading \"Snapshot\"\n    Text \"hello\"\n}\n",
            &RenderOptions::default(),
        );
        let background = RenderOptions::default().background.to_array();
        assert!(
            frame.distinct_pixels(background, 8) > 500,
            "expected painted content, got {} distinct pixels\n{}",
            frame.distinct_pixels(background, 8),
            frame.ascii_preview(64)
        );
        assert!(frame.width > 0 && frame.height > 0);
    }

    #[test]
    fn clicking_a_button_changes_the_pixels() {
        let source = "agent Qa\nfn bump() {\n    count = count + 1\n}\nui Main = Screen \"QA\" {\n    @count: int = 0\n    Text \"count is {count}\"\n    Button \"bump\" on click -> bump()\n}\n";
        let before = render(source, &RenderOptions::default());
        assert!(before.interactables.contains_key("button:bump"));
        let after = render(
            source,
            &RenderOptions {
                actions: vec![HarnessAction::Click {
                    label: "bump".to_string(),
                }],
                ..Default::default()
            },
        );
        assert_ne!(
            before.fingerprint(),
            after.fingerprint(),
            "clicking the button must change what is on screen\n{}",
            after.ascii_preview(64)
        );
    }

    #[test]
    fn typing_into_a_bound_input_updates_state_and_pixels() {
        let source = "agent Qa\nfn send() {\n    sent = text\n}\nui Main = Screen \"QA\" {\n    @text: string = \"\"\n    @sent: string = \"\"\n    Input \"type\" bind text to text\n    Text \"sent: {sent}\"\n    Button \"send\" on click -> send()\n}\n";
        let options = RenderOptions {
            actions: vec![
                HarnessAction::Type {
                    target: "text".to_string(),
                    text: "سلام".to_string(),
                },
                HarnessAction::Click {
                    label: "send".to_string(),
                },
            ],
            ..Default::default()
        };
        let frame = render(source, &options);
        assert!(
            frame.interactables.contains_key("input:text"),
            "input target not found: {:?}",
            frame.interactables.keys().collect::<Vec<_>>()
        );
        let background = RenderOptions::default().background.to_array();
        assert!(
            frame.distinct_pixels(background, 8) > 500,
            "typed text should be visible\n{}",
            frame.ascii_preview(64)
        );
    }

    #[test]
    fn theme_background_reaches_the_framebuffer() {
        let frame = render(
            "agent Qa\ntheme Night default {\n    color { background: \"#101010\" primary: \"#ff0000\" }\n}\nui Main = Screen \"QA\" {\n    Heading \"Night\"\n}\n",
            &RenderOptions {
                background: Color32::from_rgb(255, 255, 255),
                ..Default::default()
            },
        );
        assert!(
            frame.pixels_near([16, 16, 16, 255], 2) > 1000,
            "theme background should dominate the frame\n{}",
            frame.ascii_preview(64)
        );
    }

    #[test]
    fn unknown_interactive_target_is_reported() {
        let program = aec_parser::parse("agent Qa\nui Main = Screen \"QA\" {\n    Text \"x\"\n}\n").unwrap();
        let ui = program
            .items
            .iter()
            .find_map(|item| match item {
                TopLevelItem::Ui(ui) => Some(ui.clone()),
                _ => None,
            })
            .unwrap();
        let error = render_program(
            &program,
            &ui,
            &RenderOptions {
                actions: vec![HarnessAction::Click {
                    label: "missing".to_string(),
                }],
                ..Default::default()
            },
        )
        .expect_err("missing button must fail");
        assert!(error.contains("button:missing"), "{error}");
    }
}

#[cfg(test)]
mod debug_state {
    use super::*;
    use aec_ast::TopLevelItem;

    #[test]
    fn print_chatbot_state() {
        let source = std::fs::read_to_string("../../examples/chatbot.aec").unwrap();
        let program = aec_parser::parse(&source).unwrap();
        let ui = program
            .items
            .iter()
            .find_map(|item| match item {
                TopLevelItem::Ui(ui) => Some(ui.clone()),
                _ => None,
            })
            .unwrap();
        let frame = render_program(
            &program,
            &ui,
            &RenderOptions {
                actions: vec![
                    HarnessAction::Type {
                        target: "draft".into(),
                        text: "سلام دنیا".into(),
                    },
                    HarnessAction::Click {
                        label: "Send".into(),
                    },
                ],
                ..Default::default()
            },
        )
        .unwrap();
        eprintln!("STATE: {:?}", frame.state.values);
        eprintln!(
            "FRAMES: harness={} closure_runs={}",
            frame.frames,
            RUNS.with(|runs| runs.get())
        );
    }
}
