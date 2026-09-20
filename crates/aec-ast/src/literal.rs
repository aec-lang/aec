use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Interpolated(Vec<InterpPart>),
    RawString(String),
    Int(i64),
    Float(f64),
    Duration(Duration),
    ByteSize(u64),
    Bool(bool),
    Uuid(Uuid),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Duration {
    pub value: u64,
    pub unit: DurationUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl DurationUnit {
    pub fn to_ms(self) -> u64 {
        match self {
            DurationUnit::Milliseconds => 1,
            DurationUnit::Seconds => 1_000,
            DurationUnit::Minutes => 60_000,
            DurationUnit::Hours => 3_600_000,
            DurationUnit::Days => 86_400_000,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            DurationUnit::Milliseconds => "ms",
            DurationUnit::Seconds => "s",
            DurationUnit::Minutes => "m",
            DurationUnit::Hours => "h",
            DurationUnit::Days => "d",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpPart {
    Text(String),
    Expr(String),
}

impl Literal {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Literal::String(s) | Literal::RawString(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Literal::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Literal::Bool(b) => Some(*b),
            _ => None,
        }
    }
}
