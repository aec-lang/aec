//! Renders errors readably: message + source snippet + caret.

use aec_ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
        }
    }
}

/// Builds a source snippet with a caret at the error location.
///
/// ```text
///   --> 4:13
///    |
///  4 |     render {}
///    |             ^
/// ```
pub fn snippet(source: &str, span: Span) -> String {
    let line_no = span.start.line as usize;
    if line_no == 0 {
        return String::new();
    }

    let lines: Vec<&str> = source.lines().collect();
    let line = match lines.get(line_no - 1) {
        Some(l) => *l,
        None => return String::new(),
    };

    let start_col = (span.start.column as usize).saturating_sub(1);
    let end_col = if span.end.line == span.start.line {
        (span.end.column as usize).saturating_sub(1)
    } else {
        line.chars().count()
    };

    let gutter = line_no.to_string();
    let pad = " ".repeat(gutter.len());

    let indent: String = line
        .chars()
        .take(start_col)
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let caret_len = end_col.saturating_sub(start_col).max(1);
    let carets = "^".repeat(caret_len);

    format!(
        "{pad}--> {line}:{col}\n{pad} |\n{gutter} | {text}\n{pad} | {indent}{carets}",
        pad = pad,
        line = line_no,
        col = span.start.column,
        gutter = gutter,
        text = line,
        indent = indent,
        carets = carets,
    )
}

/// The full block for one diagnostic: message line first, then the source snippet.
pub fn render(source: &str, level: Level, message: &str, span: Span) -> String {
    let mut out = format!("{}: {}", level.label(), message);
    let snip = snippet(source, span);
    if !snip.is_empty() {
        out.push('\n');
        out.push_str(&snip);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use aec_ast::{Position, Span};

    fn span(line: u32, col: u32, end_line: u32, end_col: u32) -> Span {
        Span::new(
            Position::new(line, col, 0),
            Position::new(end_line, end_col, 0),
        )
    }

    #[test]
    fn snippet_points_at_the_span() {
        let source = "agent Test\n\nfn f() -> int {\n    let x: int = \"hi\"\n}\n";
        // `let x: int = ...` starts at column 5
        let out = snippet(source, span(4, 5, 4, 20));
        assert!(out.contains("--> 4:5"), "got:\n{}", out);
        assert!(out.contains(r#"let x: int = "hi""#), "got:\n{}", out);
        // The caret starts at column 5 and spans 15 characters (5 through 20)
        let caret_line = out.lines().last().unwrap();
        let caret_idx = caret_line.find('^').expect("caret should exist");
        let carets = caret_line.matches('^').count();
        assert_eq!(carets, 15, "got:\n{}", out);
        // The " | " prefix plus 4 characters of indentation
        assert_eq!(caret_idx, 4 + 4, "got:\n{}", out);
    }

    #[test]
    fn snippet_handles_out_of_range_line() {
        let source = "agent Test\n";
        assert_eq!(snippet(source, span(99, 1, 99, 1)), "");
    }

    #[test]
    fn render_includes_message_and_snippet() {
        let source = "agent Test\n\nfn f() -> int {\n    return zzz\n}\n";
        let out = render(
            source,
            Level::Warning,
            "undeclared variable",
            span(4, 12, 4, 15),
        );
        assert!(out.starts_with("warning: undeclared variable"));
        assert!(out.contains("--> 4:12"));
    }
}
