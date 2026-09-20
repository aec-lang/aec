use aec_ast::{ParseError, ParseErrorKind, Position, Span};
use pest::error::Error as PestError;
use pest::RuleType;

pub fn convert_pest_error<R: RuleType>(err: PestError<R>) -> ParseError {
    let (start_line, start_col) = match err.line_col {
        pest::error::LineColLocation::Pos((l, c)) => (l, c),
        pest::error::LineColLocation::Span((l1, c1), _) => (l1, c1),
    };

    let span = Span::new(
        Position::new(start_line as u32, start_col as u32, 0),
        Position::new(start_line as u32, start_col as u32, 0),
    );

    ParseError::new(
        span,
        ParseErrorKind::BuildError {
            message: err.to_string(),
        },
    )
}

pub fn build_error(span: Span, message: impl Into<String>) -> ParseError {
    ParseError::new(
        span,
        ParseErrorKind::BuildError {
            message: message.into(),
        },
    )
}
