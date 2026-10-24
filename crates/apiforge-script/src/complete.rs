//! What can be written at the cursor: the completion engine.
//!
//! It answers three kinds of editor: the code panel (`Script`), the object body
//! editor (`ObjectBody`), and the small fields and raw bodies where only the
//! inside of `{{ }}` is an expression (`Template`). The same index answers what a
//! name means ([`describe`]) and where it is defined ([`definition`]), so hover,
//! go-to-definition and the completion popup never disagree.
//!
//! The text is the truth while it is being typed, so handles come from the source
//! itself first and from the document only to fill in what the source cannot say.
//! Nothing here reads a secret: the host passes environment *names*, never values.
//!
//! The context is read from the token stream rather than from a parse tree,
//! because text under the cursor is usually incomplete, and a half-written
//! statement still has to answer.

use crate::lexer::{Token, TokenKind, lex};
use apiforge_core::{
    model::{HttpMethod, Predicate, ValueType},
    operator::Operator,
};
use std::ops::Range;

/// Completions one request may return. A popup shows a handful; the rest are
/// noise, and an unbounded list is a way to spend a frame badly.
pub const MAX_COMPLETIONS: usize = 60;

/// Which editor is asking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// A whole scenario or component file.
    #[default]
    Script,
    /// The contents of an object body: `key: expression` pairs.
    ObjectBody,
    /// Text with `{{ expression }}` holes: a URL, a header, a raw body.
    Template,
}

/// What a completion offers, which decides its icon and its rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Variable,
    Handle,
    Port,
    Path,
    Parameter,
    Input,
    Environment,
    Component,
    Operator,
    Predicate,
    Method,
    Keyword,
    Literal,
}

impl Kind {
    /// Where this kind sits in the list before prefix matching is considered.
    /// Variables first, as the plan asks, then what is in scope, then the
    /// vocabulary of the language.
    fn rank(self) -> i32 {
        match self {
            Self::Variable => 100,
            Self::Path | Self::Port => 95,
            Self::Handle => 90,
            Self::Parameter | Self::Input => 85,
            Self::Environment => 80,
            Self::Component => 75,
            Self::Operator | Self::Predicate => 60,
            Self::Method => 55,
            Self::Keyword => 50,
            Self::Literal => 40,
        }
    }
}

/// One offered completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub label: String,
    pub kind: Kind,
    /// A type, a signature or where the name comes from. Never a value.
    pub detail: Option<String>,
    /// The text that replaces [`Completion::replace`].
    pub insert: String,
    /// Where the caret belongs inside `insert`, for a call's parentheses.
    pub caret: usize,
    /// The range of the source the insertion replaces.
    pub replace: Range<usize>,
    pub score: i32,
}

impl Completion {
    fn new(kind: Kind, label: impl Into<String>, replace: Range<usize>) -> Self {
        let label = label.into();
        Self {
            caret: label.len(),
            insert: label.clone(),
            label,
            kind,
            detail: None,
            replace,
            score: 0,
        }
    }

    #[must_use]
    fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Inserts `insert` and puts the caret `caret` bytes into it.
    #[must_use]
    fn inserting(mut self, insert: impl Into<String>, caret: usize) -> Self {
        self.insert = insert.into();
        self.caret = caret.min(self.insert.len());
        self
    }
}

/// A port a block publishes, with the type its values have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub name: String,
    pub value_type: ValueType,
}

impl Port {
    pub fn new(name: impl Into<String>, value_type: ValueType) -> Self {
        Self {
            name: name.into(),
            value_type,
        }
    }
}

/// A block that can be referenced by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handle {
    pub name: String,
    /// A Value marked `var`, which the editor offers first.
    pub variable: bool,
    /// What the block is, for the detail line.
    pub kind: &'static str,
    pub ports: Vec<Port>,
    /// Where the block is written, when it comes from the source being edited.
    pub range: Option<Range<usize>>,
}

impl Handle {
    pub fn new(name: impl Into<String>, kind: &'static str) -> Self {
        Self {
            name: name.into(),
            variable: false,
            kind,
            ports: Vec::new(),
            range: None,
        }
    }

    #[must_use]
    pub fn with_ports(mut self, ports: Vec<Port>) -> Self {
        self.ports = ports;
        self
    }

    /// The port a path continues from when the block has exactly one, which is
    /// what lets `first.price` read a Value without naming a port.
    fn single_port(&self) -> Option<&Port> {
        match self.ports.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }
}

/// A component that can be called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentOffer {
    pub handle: String,
    pub name: String,
    pub inputs: Vec<String>,
}

/// A path a response or a contract is known to have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapePath {
    /// One segment, as it is written: `items`, `[0]`, `["odd key"]`.
    pub segment: String,
    pub value_type: ValueType,
    /// Where the shape is known from, e.g. `contract` or `last run`.
    pub source: Option<String>,
}

/// Where the paths under a block's port come from.
///
/// The desktop answers from the contract, the sample response and the last run's
/// output, in that order. A host that knows nothing returns nothing, and the
/// engine then offers only what the text itself says.
pub trait ShapeProvider {
    /// The segments that may follow `prefix` under `handle`'s `port`.
    ///
    /// `prefix` is the path typed so far, as written in the source and without a
    /// trailing dot, for example `body.items[0]`.
    fn segments(&self, handle: &str, port: &str, prefix: &str) -> Vec<ShapePath>;
}

/// A provider for a host that has no shapes to offer.
pub struct NoShapes;

impl ShapeProvider for NoShapes {
    fn segments(&self, _handle: &str, _port: &str, _prefix: &str) -> Vec<ShapePath> {
        Vec::new()
    }
}

/// What exists around the text being edited.
pub struct Scope<'a> {
    /// Blocks of the document. Blocks the source itself declares are added to
    /// these and win where the names collide.
    pub handles: &'a [Handle],
    /// Environment variable names. Never their values, secret or not.
    pub environment: &'a [String],
    pub parameters: &'a [String],
    /// Inputs of the component being edited, if one is.
    pub inputs: &'a [String],
    pub components: &'a [ComponentOffer],
    pub shapes: &'a dyn ShapeProvider,
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self {
            handles: &[],
            environment: &[],
            parameters: &[],
            inputs: &[],
            components: &[],
            shapes: &NoShapes,
        }
    }
}

/// What to complete.
pub struct Request<'a> {
    pub source: &'a str,
    /// The caret, as a UTF-8 byte offset into `source`.
    pub cursor: usize,
    pub mode: Mode,
    pub scope: Scope<'a>,
}

/// Everything that can be written at the cursor, best first.
pub fn complete(request: &Request<'_>) -> Vec<Completion> {
    let source = request.source;
    let cursor = request.cursor.min(source.len());
    if !source.is_char_boundary(cursor) {
        return Vec::new();
    }
    let (tokens, _) = lex(source);
    let word = word_at(&tokens, cursor);
    let prefix = &source[word.clone()];
    let handles = merged_handles(request);
    let mut found = match context(source, &tokens, request.mode, word.start) {
        Context::Text => Vec::new(),
        Context::Member { base } => members(request, &handles, &base, word.clone()),
        Context::Statement => {
            // What the cursor is asking for comes first: at the start of a
            // statement that is the statement vocabulary, not every name in scope.
            let mut found = preferred(statement_keywords(word.clone()));
            found.extend(expressions(request, &handles, word.clone()));
            found
        }
        Context::RequestItem => preferred(request_items(word.clone())),
        Context::Check => {
            let mut found = preferred(predicates(word.clone()));
            found.extend(expressions(request, &handles, word.clone()));
            found
        }
        Context::Expression => expressions(request, &handles, word.clone()),
    };
    rank(&mut found, prefix);
    found
}

/// Marks the candidates the context itself asks for, which lead the list.
fn preferred(mut found: Vec<Completion>) -> Vec<Completion> {
    for completion in &mut found {
        completion.score += CONTEXT_BIAS;
    }
    found
}

/// How far the words a context expects outrank the names merely in scope.
const CONTEXT_BIAS: i32 = 60;

/// Handles the document knows, overridden by the ones the source declares.
fn merged_handles(request: &Request<'_>) -> Vec<Handle> {
    let mut handles: Vec<Handle> = request.scope.handles.to_vec();
    if request.mode == Mode::Script {
        for declared in declarations(request.source) {
            match handles
                .iter_mut()
                .find(|handle| handle.name == declared.name)
            {
                // Keep the document's ports, which the text cannot state, but take
                // the source's position so that go-to-definition lands in the text.
                Some(known) => {
                    known.range = declared.range.clone();
                    known.variable = declared.variable;
                }
                None => handles.push(declared),
            }
        }
    }
    handles
}

/// The blocks a source declares, taken from its statements.
///
/// This reads the token stream rather than the parse tree: a file being typed is
/// usually not yet valid, and a statement whose right-hand side is half written
/// still declares its name.
pub fn declarations(source: &str) -> Vec<Handle> {
    let (tokens, _) = lex(source);
    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))
        .collect();
    let mut found: Vec<Handle> = Vec::new();
    for (index, token) in significant.iter().enumerate() {
        if token.kind != TokenKind::Identifier {
            continue;
        }
        if significant
            .get(index + 1)
            .is_none_or(|next| &source[next.range.clone()] != "=")
        {
            continue;
        }
        let name = &source[token.range.clone()];
        if RESERVED.contains(&name) {
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|before| significant.get(before))
            .map(|before| &source[before.range.clone()]);
        // `param x = …` and `input x: …` declare something else.
        if previous == Some("param") {
            continue;
        }
        let variable = previous == Some("var");
        let kind = significant
            .get(index + 2)
            .map(|form| &source[form.range.clone()])
            .map_or("value", form_kind);
        let mut handle = Handle::new(name, kind);
        handle.variable = variable;
        handle.ports = default_ports(kind);
        handle.range = Some(token.range.clone());
        if let Some(existing) = found.iter_mut().find(|found| found.name == name) {
            *existing = handle;
        } else {
            found.push(handle);
        }
    }
    found
}

/// What a statement's first word says the block is.
fn form_kind(first: &str) -> &'static str {
    match first {
        "start" => "start",
        "assert" => "assert",
        "condition" => "condition",
        "if" => "if",
        word if METHODS.contains(&word) => "request",
        _ => "value",
    }
}

/// The ports a kind publishes, for a block known only from the text.
fn default_ports(kind: &str) -> Vec<Port> {
    match kind {
        "request" => vec![
            Port::new("status", ValueType::Number),
            Port::new("body", ValueType::Any),
            Port::new("headers", ValueType::Object),
            Port::new("duration", ValueType::Number),
        ],
        "assert" | "if" => vec![Port::new("passed", ValueType::Boolean)],
        "condition" => vec![
            Port::new("passed", ValueType::Boolean),
            Port::new("value", ValueType::Any),
        ],
        "start" => Vec::new(),
        _ => vec![Port::new("value", ValueType::Any)],
    }
}

/// What the cursor sits in.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Context {
    /// Inside a string, outside any `{{ }}`.
    Text,
    /// After `base.`, where `base` is the chain of names already written.
    Member { base: Vec<String> },
    /// Where a statement may begin.
    Statement,
    /// Inside a request block, where its items may begin.
    RequestItem,
    /// Where a predicate may follow, inside `assert`, `if` or `condition`.
    Check,
    /// Anywhere else an expression may be written.
    Expression,
}

/// What kind of `{` a block was opened by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    /// The file, a `cleanup` block or an `if` branch: statements.
    Statements,
    /// A request block: `headers`, `query`, `body`, `timeout`, …
    Request,
    /// `condition all { … }`: checks.
    Checks,
    /// An object literal, including `headers { … }`.
    Object,
}

fn context(source: &str, tokens: &[Token], mode: Mode, at: usize) -> Context {
    if mode == Mode::Template {
        // The whole text is a template: only the inside of `{{ }}` is code.
        return interpolation(source, 0..source.len(), at).unwrap_or(Context::Text);
    }
    if let Some(inside) = template_context(source, tokens, at) {
        return inside;
    }
    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|token| {
            token.range.start < at
                && !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment)
        })
        .collect();
    if let Some(base) = member_base(source, &significant) {
        return Context::Member { base };
    }
    if mode != Mode::Script {
        // A body or a field is one expression or a list of them; the statement
        // vocabulary has no meaning there.
        return Context::Expression;
    }
    let mut blocks: Vec<Block> = Vec::new();
    // What the current statement looks like so far, to tell `handle = …` from the
    // beginning of a new statement.
    let mut assigned = false;
    let mut check_word = false;
    for (index, token) in significant.iter().enumerate() {
        let text = &source[token.range.clone()];
        match text {
            "{" => {
                blocks.push(open_block(source, &significant[..index]));
                assigned = false;
                check_word = false;
            }
            "}" => {
                blocks.pop();
                assigned = false;
                check_word = false;
            }
            "=" => assigned = true,
            "assert" | "if" => check_word = true,
            _ => {
                if starts_statement(source, &significant, index) {
                    assigned = false;
                    check_word = false;
                }
            }
        }
    }
    match blocks.last() {
        Some(Block::Request) if !assigned => Context::RequestItem,
        Some(Block::Checks) => Context::Check,
        _ if check_word => Context::Check,
        Some(Block::Object) => Context::Expression,
        Some(Block::Statements) | None if !assigned => Context::Statement,
        _ => Context::Expression,
    }
}

/// Whether the token at `index` begins a new statement, which resets what the
/// current one has seen. A statement begins at a handle followed by `=`, or at a
/// word that only ever starts one.
fn starts_statement(source: &str, significant: &[&Token], index: usize) -> bool {
    let text = &source[significant[index].range.clone()];
    if matches!(
        text,
        "var" | "export" | "param" | "description" | "on_error"
    ) {
        return true;
    }
    significant
        .get(index + 1)
        .is_some_and(|next| &source[next.range.clone()] == "=")
        && significant[index].kind == TokenKind::Identifier
}

/// What a `{` opened, from the words before it.
fn open_block(source: &str, before: &[&Token]) -> Block {
    let word = |back: usize| {
        before
            .len()
            .checked_sub(back)
            .and_then(|index| before.get(index))
            .map(|token| &source[token.range.clone()])
    };
    match word(1) {
        Some("headers" | "query" | "body") => Block::Object,
        Some("all" | "any") if word(2) == Some("condition") => Block::Checks,
        Some("cleanup") => Block::Statements,
        Some(")" | "}") | None => Block::Statements,
        _ => {
            // A request block follows the URL of a method statement; a brace after
            // a check or a scenario name opens statements; anything else is an
            // object literal.
            if before.iter().rev().take(6).any(|token| {
                let text = &source[token.range.clone()];
                METHODS.contains(&text)
            }) {
                Block::Request
            } else if before.iter().rev().take(4).any(|token| {
                let text = &source[token.range.clone()];
                matches!(text, "scenario" | "component" | "if" | "else" | "cleanup")
            }) {
                Block::Statements
            } else {
                Block::Object
            }
        }
    }
}

/// `Some` when the cursor is inside a string: an expression inside `{{ }}`, or
/// text where nothing can be completed.
fn template_context(source: &str, tokens: &[Token], at: usize) -> Option<Context> {
    let token = tokens
        .iter()
        .find(|token| token.kind == TokenKind::String && token.range.contains(&at))?;
    Some(interpolation(source, token.range.clone(), at).unwrap_or(Context::Text))
}

/// The context inside an open `{{ … }}` within `region`, or `None` when the
/// cursor is in the text around it.
fn interpolation(source: &str, region: Range<usize>, at: usize) -> Option<Context> {
    let before = source.get(region.start..at)?;
    let open = before.rfind("{{")?;
    if before.rfind("}}").is_some_and(|close| close > open) {
        return None;
    }
    let start = region.start + open + 2;
    let (tokens, _) = lex(source.get(start..at)?);
    let significant: Vec<Token> = tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))
        .map(|mut token| {
            token.range.start += start;
            token.range.end += start;
            token
        })
        .collect();
    let borrowed: Vec<&Token> = significant.iter().collect();
    Some(match member_base(source, &borrowed) {
        Some(base) => Context::Member { base },
        None => Context::Expression,
    })
}

/// The names before a trailing `.`, oldest first, or `None` when the cursor is
/// not completing a member.
fn member_base(source: &str, significant: &[&Token]) -> Option<Vec<String>> {
    let mut index = significant.len();
    if significant
        .get(index.checked_sub(1)?)
        .map(|token| &source[token.range.clone()])
        != Some(".")
    {
        return None;
    }
    index -= 1;
    let mut base = Vec::new();
    while let Some(token) = index.checked_sub(1).and_then(|at| significant.get(at)) {
        let text = &source[token.range.clone()];
        if token.kind == TokenKind::Identifier {
            base.push(text.to_owned());
            index -= 1;
            // A dot before it continues the chain.
            match index
                .checked_sub(1)
                .and_then(|at| significant.get(at))
                .map(|token| &source[token.range.clone()])
            {
                Some(".") => index -= 1,
                _ => break,
            }
        } else if text == "]" {
            // Skip an index segment; its key is part of the path, not a name.
            let mut depth = 0;
            while let Some(token) = index.checked_sub(1).and_then(|at| significant.get(at)) {
                index -= 1;
                match &source[token.range.clone()] {
                    "]" => depth += 1,
                    "[" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            base.push("[]".to_owned());
        } else {
            break;
        }
    }
    base.reverse();
    (!base.is_empty()).then_some(base)
}

/// The word being typed at the cursor, as a range to replace.
fn word_at(tokens: &[Token], cursor: usize) -> Range<usize> {
    tokens
        .iter()
        .find(|token| {
            token.kind == TokenKind::Identifier
                && token.range.start < cursor
                && token.range.end >= cursor
        })
        .map(|token| token.range.start..cursor)
        .unwrap_or(cursor..cursor)
}

const METHODS: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];

const RESERVED: [&str; 24] = [
    "scenario",
    "component",
    "cleanup",
    "param",
    "input",
    "var",
    "export",
    "start",
    "assert",
    "condition",
    "if",
    "else",
    "all",
    "any",
    "exists",
    "nonempty",
    "contains",
    "is",
    "within",
    "with",
    "true",
    "false",
    "null",
    "env",
];

fn statement_keywords(replace: Range<usize>) -> Vec<Completion> {
    let mut found = vec![
        Completion::new(Kind::Keyword, "var", replace.clone())
            .inserting("var ", 4)
            .detail("a value other blocks can use by name"),
        Completion::new(Kind::Keyword, "export", replace.clone())
            .inserting("export ", 7)
            .detail("publish a value as a component output"),
        Completion::new(Kind::Keyword, "param", replace.clone())
            .inserting("param ", 6)
            .detail("a scenario parameter"),
        Completion::new(Kind::Keyword, "cleanup", replace.clone())
            .inserting("cleanup {\n  \n}", 12)
            .detail("steps that run after the scenario, whatever happened"),
        Completion::new(Kind::Keyword, "description", replace.clone())
            .inserting("description \"\"", 13)
            .detail("what this scenario is for"),
        Completion::new(Kind::Keyword, "on_error", replace.clone())
            .inserting("on_error stop", 13)
            .detail("stop or continue after a failed step"),
        Completion::new(Kind::Keyword, "@after", replace.clone())
            .inserting("@after()", 7)
            .detail("run this step after another"),
        Completion::new(Kind::Keyword, "@label", replace.clone())
            .inserting("@label(\"\")", 8)
            .detail("the name shown on the board"),
    ];
    found.extend(forms(replace));
    found
}

/// The forms a statement's right-hand side may take.
fn forms(replace: Range<usize>) -> Vec<Completion> {
    let mut found = vec![
        Completion::new(Kind::Keyword, "start", replace.clone())
            .detail("where the scenario begins"),
        Completion::new(Kind::Keyword, "assert", replace.clone())
            .inserting("assert ", 7)
            .detail("check a value and fail the run when it does not hold"),
        Completion::new(Kind::Keyword, "condition", replace.clone())
            .inserting("condition all {\n  \n}", 18)
            .detail("check several values without failing the run"),
        Completion::new(Kind::Keyword, "if", replace.clone())
            .inserting("if  {\n  \n} else {\n  \n}", 3)
            .detail("take one of two branches"),
    ];
    for method in METHODS {
        found.push(
            Completion::new(Kind::Method, method, replace.clone())
                .inserting(format!("{method} \"\""), method.len() + 2)
                .detail("an HTTP request"),
        );
    }
    found
}

fn request_items(replace: Range<usize>) -> Vec<Completion> {
    vec![
        Completion::new(Kind::Keyword, "headers", replace.clone())
            .inserting("headers { }", 10)
            .detail("request headers"),
        Completion::new(Kind::Keyword, "query", replace.clone())
            .inserting("query { }", 8)
            .detail("query parameters"),
        Completion::new(Kind::Keyword, "body", replace.clone())
            .inserting("body { }", 7)
            .detail("the request body"),
        Completion::new(Kind::Keyword, "body text", replace.clone())
            .inserting("body text \"\"", 11)
            .detail("a raw text body"),
        Completion::new(Kind::Keyword, "timeout", replace.clone())
            .inserting("timeout 5s", 10)
            .detail("how long to wait for a response"),
        Completion::new(Kind::Keyword, "sample", replace.clone())
            .inserting("sample 200 { }", 13)
            .detail("the response a simulation uses"),
        Completion::new(Kind::Keyword, "contract", replace.clone())
            .inserting("contract \"\"", 10)
            .detail("the data contract the response follows"),
        Completion::new(Kind::Keyword, "argument", replace.clone())
            .inserting("argument  = ", 9)
            .detail("a value this request computes for itself"),
    ]
}

fn predicates(replace: Range<usize>) -> Vec<Completion> {
    [
        ("exists", "the value is present", "exists "),
        ("nonempty", "the value is not empty", "nonempty "),
        ("contains", "the text or list contains it", "contains "),
        ("is", "the value has this type", "is "),
        ("within", "how far apart `~=` may be", "within "),
    ]
    .into_iter()
    .map(|(label, detail, insert)| {
        Completion::new(Kind::Predicate, label, replace.clone())
            .inserting(insert, insert.len())
            .detail(detail)
    })
    .collect()
}

/// Everything that may appear where a value is expected.
fn expressions(
    request: &Request<'_>,
    handles: &[Handle],
    replace: Range<usize>,
) -> Vec<Completion> {
    let scope = &request.scope;
    let mut found = Vec::new();
    for handle in handles {
        let kind = if handle.variable {
            Kind::Variable
        } else {
            Kind::Handle
        };
        let detail = match handle.single_port() {
            Some(port) => format!("{} · {}", handle.kind, port.value_type.as_str()),
            None => handle.kind.to_owned(),
        };
        found.push(Completion::new(kind, handle.name.clone(), replace.clone()).detail(detail));
    }
    for name in scope.parameters {
        found.push(
            Completion::new(Kind::Parameter, format!("param.{name}"), replace.clone())
                .detail("a scenario parameter"),
        );
    }
    for name in scope.inputs {
        found.push(
            Completion::new(Kind::Input, format!("input.{name}"), replace.clone())
                .detail("an input of this component"),
        );
    }
    for name in scope.environment {
        found.push(
            Completion::new(Kind::Environment, format!("env.{name}"), replace.clone())
                // The value may be a secret, so only the name is ever shown.
                .detail("an environment variable"),
        );
    }
    for component in scope.components {
        let arguments = component
            .inputs
            .iter()
            .map(|input| format!("{input}: "))
            .collect::<Vec<_>>()
            .join(", ");
        let insert = format!("{}({arguments})", component.handle);
        let caret = component.handle.len() + 1 + arguments.len();
        found.push(
            Completion::new(Kind::Component, component.handle.clone(), replace.clone())
                .inserting(insert, caret)
                .detail(format!("component · {}", component.name)),
        );
    }
    for operator in Operator::ALL {
        let Some(name) = operator_name(operator) else {
            continue;
        };
        let spec = operator.spec();
        let signature = spec
            .operands
            .iter()
            .map(|operand| operand.name)
            .collect::<Vec<_>>()
            .join(", ");
        found.push(
            Completion::new(Kind::Operator, name.clone(), replace.clone())
                .inserting(format!("{name}()"), name.len() + 1)
                .detail(format!(
                    "{}({signature}) → {}",
                    operator.label(),
                    spec.output.as_str()
                )),
        );
    }
    for (label, detail) in [
        ("true", "a boolean"),
        ("false", "a boolean"),
        ("null", "no value"),
    ] {
        found.push(Completion::new(Kind::Literal, label, replace.clone()).detail(detail));
    }
    found
}

/// What follows a dot: ports and paths of a block, or the names under `env`,
/// `param` and `input`.
fn members(
    request: &Request<'_>,
    handles: &[Handle],
    base: &[String],
    replace: Range<usize>,
) -> Vec<Completion> {
    let scope = &request.scope;
    let (first, rest) = base
        .split_first()
        .map_or(("", &[][..]), |(first, rest)| (first.as_str(), rest));
    if rest.is_empty() {
        match first {
            "env" => {
                return scope
                    .environment
                    .iter()
                    .map(|name| {
                        Completion::new(Kind::Environment, name.clone(), replace.clone())
                            .detail("an environment variable")
                    })
                    .collect();
            }
            "param" => {
                return scope
                    .parameters
                    .iter()
                    .map(|name| {
                        Completion::new(Kind::Parameter, name.clone(), replace.clone())
                            .detail("a scenario parameter")
                    })
                    .collect();
            }
            "input" => {
                return scope
                    .inputs
                    .iter()
                    .map(|name| {
                        Completion::new(Kind::Input, name.clone(), replace.clone())
                            .detail("an input of this component")
                    })
                    .collect();
            }
            "op" => {
                return Operator::ALL
                    .into_iter()
                    .filter_map(|operator| {
                        let name = operator_name(operator)?;
                        Some(
                            Completion::new(Kind::Operator, name.clone(), replace.clone())
                                .inserting(format!("{name}()"), name.len() + 1)
                                .detail(operator.label()),
                        )
                    })
                    .collect();
            }
            _ => {}
        }
    }
    let Some(handle) = handles.iter().find(|handle| handle.name == first) else {
        return Vec::new();
    };
    // The first segment after a block with several ports names the port; a block
    // with one port is read straight through it.
    let (port, path) = match (handle.single_port(), rest.split_first()) {
        (Some(single), _) => (single.name.clone(), rest.to_vec()),
        (None, Some((named, after))) => (named.clone(), after.to_vec()),
        (None, None) => {
            return handle
                .ports
                .iter()
                .map(|port| {
                    Completion::new(Kind::Port, port.name.clone(), replace.clone())
                        .detail(format!("output · {}", port.value_type.as_str()))
                })
                .collect();
        }
    };
    let prefix = path.join(".");
    scope
        .shapes
        .segments(&handle.name, &port, &prefix)
        .into_iter()
        .map(|found| {
            let detail = match found.source {
                Some(source) => format!("{} · {source}", found.value_type.as_str()),
                None => found.value_type.as_str().to_owned(),
            };
            Completion::new(Kind::Path, found.segment, replace.clone()).detail(detail)
        })
        .collect()
}

/// The identifier an operator is written with, as the document names it.
fn operator_name(operator: Operator) -> Option<String> {
    match serde_json::to_value(operator) {
        Ok(serde_json::Value::String(name)) => Some(name),
        _ => None,
    }
}

/// Scores the candidates against what has been typed and drops what does not
/// match at all. Case-sensitive prefixes win over case-insensitive ones, which win
/// over a name that merely contains the text.
fn rank(found: &mut Vec<Completion>, prefix: &str) {
    if prefix.is_empty() {
        for completion in found.iter_mut() {
            completion.score += completion.kind.rank();
        }
    } else {
        let lower = prefix.to_ascii_lowercase();
        found.retain_mut(|completion| {
            let label = &completion.label;
            let bonus = if label.starts_with(prefix) {
                1000
            } else if label.to_ascii_lowercase().starts_with(&lower) {
                800
            } else if label.to_ascii_lowercase().contains(&lower) {
                400
            } else {
                return false;
            };
            // A short name that matches is usually the one meant.
            let length_penalty = i32::try_from(label.len()).unwrap_or(i32::MAX).min(200);
            completion.score += bonus + completion.kind.rank() - length_penalty;
            true
        });
    }
    found.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
    });
    found.dedup_by(|left, right| left.label == right.label && left.kind == right.kind);
    found.truncate(MAX_COMPLETIONS);
}

/// What a name at the cursor means: its kind and type, for a hover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    pub name: String,
    pub kind: Kind,
    pub detail: String,
    /// Where it is written in this source, when it is written here at all.
    pub definition: Option<Range<usize>>,
}

/// Describes the name under the cursor.
pub fn describe(request: &Request<'_>) -> Option<Description> {
    let source = request.source;
    let cursor = request.cursor.min(source.len());
    if !source.is_char_boundary(cursor) {
        return None;
    }
    let (tokens, _) = lex(source);
    let token = tokens.iter().find(|token| {
        token.kind == TokenKind::Identifier
            && token.range.start <= cursor
            && token.range.end >= cursor
    })?;
    let name = &source[token.range.clone()];
    let scope = &request.scope;
    if let Some(handle) = merged_handles(request)
        .into_iter()
        .find(|handle| handle.name == name)
    {
        let detail = match handle.single_port() {
            Some(port) => format!("{} · {}", handle.kind, port.value_type.as_str()),
            None => format!(
                "{} · {}",
                handle.kind,
                handle
                    .ports
                    .iter()
                    .map(|port| port.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        return Some(Description {
            kind: if handle.variable {
                Kind::Variable
            } else {
                Kind::Handle
            },
            detail,
            definition: handle.range.clone(),
            name: handle.name,
        });
    }
    if scope.parameters.iter().any(|known| known == name) {
        return Some(Description {
            name: name.to_owned(),
            kind: Kind::Parameter,
            detail: "a scenario parameter".to_owned(),
            definition: None,
        });
    }
    if scope.environment.iter().any(|known| known == name) {
        return Some(Description {
            name: name.to_owned(),
            kind: Kind::Environment,
            detail: "an environment variable".to_owned(),
            definition: None,
        });
    }
    if scope.inputs.iter().any(|known| known == name) {
        return Some(Description {
            name: name.to_owned(),
            kind: Kind::Input,
            detail: "an input of this component".to_owned(),
            definition: None,
        });
    }
    if let Some(component) = scope
        .components
        .iter()
        .find(|component| component.handle == name)
    {
        return Some(Description {
            name: name.to_owned(),
            kind: Kind::Component,
            detail: format!("component · {}", component.name),
            definition: None,
        });
    }
    let operator = Operator::ALL
        .into_iter()
        .find(|operator| operator_name(*operator).as_deref() == Some(name))?;
    Some(Description {
        name: name.to_owned(),
        kind: Kind::Operator,
        detail: format!("{} → {}", operator.label(), operator.spec().output.as_str()),
        definition: None,
    })
}

/// Where the name under the cursor is defined in this source.
pub fn definition(request: &Request<'_>) -> Option<Range<usize>> {
    describe(request)?.definition
}

/// Predicates as the language writes them, for the editor's own lists.
pub fn predicate_words() -> Vec<(Predicate, &'static str)> {
    vec![
        (Predicate::Equals, "=="),
        (Predicate::NotEquals, "!="),
        (Predicate::Greater, ">"),
        (Predicate::Less, "<"),
        (Predicate::Approximately, "~="),
        (Predicate::Contains, "contains"),
        (Predicate::Type, "is"),
        (Predicate::Exists, "exists"),
        (Predicate::NotEmpty, "nonempty"),
    ]
}

/// HTTP methods as the language writes them.
pub fn method_words() -> Vec<(HttpMethod, &'static str)> {
    vec![
        (HttpMethod::Get, "GET"),
        (HttpMethod::Head, "HEAD"),
        (HttpMethod::Post, "POST"),
        (HttpMethod::Put, "PUT"),
        (HttpMethod::Patch, "PATCH"),
        (HttpMethod::Delete, "DELETE"),
        (HttpMethod::Options, "OPTIONS"),
    ]
}

#[cfg(test)]
mod tests;

/// Where each statement of a source is written, as `(handle, range)`.
///
/// Statements inside an `if` block are included, so a cursor anywhere in the
/// text can be traced to the block it belongs to. The ranges come from the parse
/// tree and are in the order the text has them.
pub fn outline(source: &str) -> Vec<(String, Range<usize>)> {
    fn walk(statements: &[crate::ast::Statement], found: &mut Vec<(String, Range<usize>)>) {
        for statement in statements {
            found.push((statement.handle.clone(), statement.range.clone()));
            if let crate::ast::Form::If {
                then, otherwise, ..
            } = &statement.form
            {
                walk(then, found);
                if let Some(otherwise) = otherwise {
                    walk(otherwise, found);
                }
            }
        }
    }
    let tree = crate::parse(source);
    let mut found = Vec::new();
    if let Some(file) = &tree.file {
        walk(&file.statements, &mut found);
        if let Some(cleanup) = &file.cleanup {
            walk(cleanup, &mut found);
        }
    }
    found
}

/// The block the cursor is inside, by handle.
///
/// The innermost statement wins, so a cursor inside an `if` branch names the
/// block in the branch rather than the `if` around it.
pub fn statement_at(source: &str, cursor: usize) -> Option<String> {
    outline(source)
        .into_iter()
        .filter(|(_, range)| range.start <= cursor && cursor <= range.end)
        .min_by_key(|(_, range)| range.end - range.start)
        .map(|(handle, _)| handle)
}

/// Where a block is written, for revealing it when it is selected elsewhere.
pub fn statement_range(source: &str, handle: &str) -> Option<Range<usize>> {
    outline(source)
        .into_iter()
        .find(|(found, _)| found == handle)
        .map(|(_, range)| range)
}
