use aec_ast::{ParseError, ParseErrorKind, Position, Span};
use pest::error::{Error as PestError, ErrorVariant};
use pest::RuleType;

/// Turns a pest error into a single readable line (instead of its multi-line block).
fn pest_message<R: RuleType>(err: &PestError<R>) -> String {
    match &err.variant {
        ErrorVariant::ParsingError {
            positives,
            negatives,
        } => {
            let names = |rules: &[R]| -> Vec<String> {
                let mut v: Vec<String> = rules.iter().map(|r| format!("{:?}", r)).collect();
                v.sort();
                v.dedup();
                v
            };

            let expected = names(positives);
            let mut message = if expected.is_empty() {
                "invalid input".to_string()
            } else {
                let shown: Vec<String> = expected.iter().take(6).cloned().collect();
                let mut s = format!("expected one of: {}", shown.join(", "));
                if expected.len() > shown.len() {
                    s.push_str(" …");
                }
                s
            };

            let forbidden = names(negatives);
            if !forbidden.is_empty() {
                message.push_str(&format!(" (not allowed: {})", forbidden.join(", ")));
            }
            message
        }
        ErrorVariant::CustomError { message } => message.clone(),
    }
}

pub fn convert_pest_error<R: RuleType>(err: PestError<R>) -> ParseError {
    let (start_line, start_col) = match err.line_col {
        pest::error::LineColLocation::Pos((l, c)) => (l, c),
        pest::error::LineColLocation::Span((l1, c1), _) => (l1, c1),
    };

    let span = Span::new(
        Position::new(start_line as u32, start_col as u32, 0),
        Position::new(start_line as u32, start_col as u32, 0),
    );

    let message = pest_message(&err);

    ParseError::new(span, ParseErrorKind::BuildError { message })
}

pub fn build_error(span: Span, message: impl Into<String>) -> ParseError {
    ParseError::new(
        span,
        ParseErrorKind::BuildError {
            message: message.into(),
        },
    )
}
