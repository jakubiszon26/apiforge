use crate::lexer::Token;
use serde_json::Value;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub range: Range<usize>,
    pub replacement: String,
}

/// The warning a comment gets when it belongs to no statement: canonical
/// printing keeps notes attached to statements and drops the rest. Callers
/// recognise it by this constant rather than by its wording.
pub const UNATTACHED_COMMENT: &str =
    "This comment is not attached to a statement and will be dropped by formatting";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range<usize>,
    pub severity: Severity,
    /// Static descriptions only: never echo literal values or source fragments.
    pub message: String,
    pub fix: Option<Fix>,
}

impl Diagnostic {
    pub fn error(range: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            range,
            message: message.into(),
            severity: Severity::Error,
            fix: None,
        }
    }
}

/// The original source and every token (including trivia) remain available even
/// for invalid text. AST nodes and diagnostics use UTF-8 byte offsets into it.
#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxTree {
    pub source: String,
    pub tokens: Vec<Token>,
    pub file: Option<File>,
    pub diagnostics: Vec<Diagnostic>,
    /// Diagnostics found beyond `MAX_DIAGNOSTICS` and not kept.
    pub omitted_diagnostics: usize,
}

impl SyntaxTree {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileKind {
    Scenario { name: String },
    Component { handle: String, name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct File {
    pub kind: FileKind,
    pub description: Option<String>,
    pub on_error: Option<String>,
    pub parameters: Vec<(String, Expr)>,
    pub inputs: Vec<Input>,
    pub action: Option<String>,
    pub action_type: Option<String>,
    pub action_contract: Option<String>,
    pub statements: Vec<Statement>,
    pub cleanup: Option<Vec<Statement>>,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub value_type: String,
    pub default: Option<Expr>,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<Expr>,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub handle: String,
    pub variable: bool,
    pub attributes: Vec<Attribute>,
    pub note: Option<String>,
    pub form: Form,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Form {
    Start,
    Value(Expr),
    Request {
        method: String,
        url: Expr,
        items: Vec<RequestItem>,
    },
    Assert(Check),
    Condition {
        mode: String,
        checks: Vec<Check>,
        value: Option<Expr>,
    },
    If {
        check: Check,
        then: Vec<Statement>,
        otherwise: Option<Vec<Statement>>,
    },
    Export {
        value_type: String,
        value: Expr,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum RequestItem {
    Argument { name: String, value: Expr },
    Headers(Expr),
    Query(Expr),
    Body { text: bool, value: Expr },
    Timeout { milliseconds: f64 },
    Contract(String),
    Sample { status: u16, value: Expr },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub actual: Expr,
    pub predicate: String,
    pub expected: Option<Expr>,
    pub tolerance: Option<Expr>,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Literal(Value),
    Name(String),
    Object(Vec<(String, Expr)>),
    Array(Vec<Expr>),
    Template(Vec<Part>),
    Field {
        base: Box<Expr>,
        key: String,
        index: bool,
    },
    Call {
        name: String,
        revision: Option<u32>,
        arguments: Vec<Argument>,
    },
    Binary {
        operator: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Not(Box<Expr>),
    Group(Box<Expr>),
    /// An unconnected editor slot; lowering omits it rather than inventing null.
    Missing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Text(String),
    Value(Expr),
}
