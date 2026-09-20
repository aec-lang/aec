#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

impl Position {
    pub fn new(line: u32, column: u32, offset: u32) -> Self {
        Self { line, column, offset }
    }
    pub fn zero() -> Self {
        Self { line: 1, column: 1, offset: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
    pub fn dummy() -> Self {
        Span {
            start: Position::zero(),
            end: Position::zero(),
        }
    }
    pub fn len(&self) -> u32 {
        self.end.offset.saturating_sub(self.start.offset)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Spanned {
    fn span(&self) -> Span;
}
