use crate::{
    MAX_DEPTH, MAX_DIAGNOSTICS, MAX_SOURCE_BYTES,
    ast::*,
    lexer::{self, Token, TokenKind},
};
use serde_json::Value;
use std::{collections::HashSet, ops::Range};

type Result<T> = std::result::Result<T, ()>;

pub fn parse(source: &str) -> SyntaxTree {
    let (tokens, mut diagnostics, mut omitted) = lexer::lex_counted(source);
    let file = if source.len() > MAX_SOURCE_BYTES {
        None
    } else {
        let mut parser = Parser::new(source, &tokens, 0, 0);
        let mut file = parser.file().ok();
        if parser.at < parser.significant.len() {
            parser.error("Unexpected text after the file");
        }
        diagnostics.extend(parser.diagnostics);
        omitted += parser.omitted;
        omitted += diagnostics.len().saturating_sub(MAX_DIAGNOSTICS);
        diagnostics.truncate(MAX_DIAGNOSTICS);
        if let Some(file) = &mut file {
            omitted += attach_comments(source, &tokens, file, &mut diagnostics);
        }
        file
    };
    SyntaxTree {
        source: source.into(),
        tokens,
        file,
        diagnostics,
        omitted_diagnostics: omitted,
    }
}

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    significant: Vec<usize>,
    at: usize,
    last_end: usize,
    base: usize,
    depth: usize,
    diagnostics: Vec<Diagnostic>,
    /// Diagnostics dropped beyond `MAX_DIAGNOSTICS`.
    omitted: usize,
    parameter_names: HashSet<String>,
    input_names: HashSet<String>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, tokens: &'a [Token], base: usize, depth: usize) -> Self {
        Self {
            source,
            tokens,
            significant: tokens
                .iter()
                .enumerate()
                .filter(|(_, t)| !matches!(t.kind, TokenKind::Whitespace | TokenKind::Comment))
                .map(|(i, _)| i)
                .collect(),
            at: 0,
            last_end: base,
            base,
            depth,
            diagnostics: vec![],
            omitted: 0,
            parameter_names: HashSet::new(),
            input_names: HashSet::new(),
        }
    }

    fn token(&self, ahead: usize) -> Option<&Token> {
        self.significant
            .get(self.at + ahead)
            .map(|i| &self.tokens[*i])
    }
    fn peek(&self) -> &'a str {
        self.token(0).map_or("", |t| &self.source[t.range.clone()])
    }
    fn next_is(&self, value: &str) -> bool {
        self.token(1)
            .is_some_and(|t| &self.source[t.range.clone()] == value)
    }
    fn range(&self) -> Range<usize> {
        self.token(0).map_or(
            self.base + self.source.len()..self.base + self.source.len(),
            |t| self.base + t.range.start..self.base + t.range.end,
        )
    }
    fn bump(&mut self) -> &'a str {
        let text = self.peek();
        self.last_end = self.range().end;
        if self.at < self.significant.len() {
            self.at += 1;
        }
        text
    }
    fn eat(&mut self, text: &str) -> bool {
        if self.peek() == text {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, text: &'static str) -> Result<()> {
        if self.eat(text) {
            Ok(())
        } else {
            let range = self.range();
            let count = self.diagnostics.len();
            self.error(format!("Expected `{text}`"));
            if matches!(text, "}" | "]" | ")" | ":" | "=")
                && self.diagnostics.len() > count
                && let Some(d) = self.diagnostics.last_mut()
            {
                d.fix = Some(Fix {
                    range: range.start..range.start,
                    replacement: text.into(),
                });
            }
            Err(())
        }
    }
    fn error(&mut self, message: impl Into<String>) {
        self.error_at(self.range(), message);
    }

    fn error_at(&mut self, range: Range<usize>, message: impl Into<String>) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(Diagnostic::error(range, message));
        } else {
            self.omitted += 1;
        }
    }
    fn append_diagnostics(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        let remaining = MAX_DIAGNOSTICS.saturating_sub(self.diagnostics.len());
        let mut diagnostics = diagnostics.into_iter();
        self.diagnostics
            .extend(diagnostics.by_ref().take(remaining));
        self.omitted += diagnostics.count();
    }
    fn identifier(&mut self) -> Result<String> {
        if self
            .token(0)
            .is_some_and(|t| t.kind == TokenKind::Identifier)
        {
            Ok(self.bump().into())
        } else {
            self.error("Expected an identifier");
            Err(())
        }
    }
    fn name(&mut self) -> Result<String> {
        if self.token(0).is_some_and(|t| t.kind == TokenKind::String) {
            self.plain_string()
        } else {
            self.identifier()
        }
    }
    fn plain_string(&mut self) -> Result<String> {
        let expr = self.string()?;
        if let ExprKind::Literal(Value::String(text)) = expr.kind {
            Ok(text)
        } else {
            self.error_at(expr.range, "Interpolation is not allowed here");
            Err(())
        }
    }
    fn file(&mut self) -> Result<File> {
        let start = self.range().start;
        let kind = if self.eat("scenario") {
            FileKind::Scenario {
                name: self.plain_string()?,
            }
        } else if self.eat("component") {
            let handle_range = self.range();
            let handle = self.identifier()?;
            if !apiforge_core::handle::is_valid_handle(&handle) {
                self.error_at(handle_range, "Invalid or reserved component handle");
            }
            FileKind::Component {
                handle,
                name: self.plain_string()?,
            }
        } else {
            self.error("Expected a scenario or component declaration");
            return Err(());
        };
        self.expect("{")?;
        let mut file = File {
            kind,
            description: None,
            on_error: None,
            parameters: vec![],
            inputs: vec![],
            action: None,
            action_type: None,
            action_contract: None,
            statements: vec![],
            cleanup: None,
            range: start..start,
        };
        while !matches!(self.peek(), "}" | "") {
            let before = self.at;
            if self.file_item(&mut file).is_err() {
                self.recover(before);
            }
            if self.at == before {
                self.bump();
            }
        }
        let _ = self.expect("}");
        file.range.end = self.last_end;
        Ok(file)
    }
    fn file_item(&mut self, file: &mut File) -> Result<()> {
        let scenario = matches!(file.kind, FileKind::Scenario { .. });
        match self.peek() {
            "description" if !self.next_is("=") => {
                self.bump();
                let value = self.plain_string()?;
                if file.description.replace(value).is_some() {
                    self.error("Duplicate description");
                }
            }
            "on_error" if scenario && !self.next_is("=") => {
                self.bump();
                if !matches!(self.peek(), "stop" | "continue") {
                    self.error("Expected stop or continue");
                    return Err(());
                }
                if file.on_error.replace(self.bump().into()).is_some() {
                    self.error("Duplicate error policy");
                }
            }
            "param" if scenario => {
                self.bump();
                let name = self.name()?;
                self.expect("=")?;
                let value = self.literal()?;
                if !self.parameter_names.insert(name.clone()) {
                    self.error("Duplicate parameter");
                }
                file.parameters.push((name, value));
            }
            "cleanup" if scenario => {
                self.bump();
                let statements = self.block()?;
                if file.cleanup.replace(statements).is_some() {
                    self.error("Duplicate cleanup block");
                }
            }
            "action" if !scenario && !self.next_is("=") => {
                self.bump();
                let name = self.name()?;
                if self.eat(":") {
                    file.action_type = Some(self.value_type()?);
                }
                if self.eat("contract") {
                    file.action_contract = Some(self.plain_string()?);
                }
                if file.action.replace(name).is_some() {
                    self.error("Duplicate action argument");
                }
            }
            _ => {
                let start = self.range().start;
                let attributes = self.attributes()?;
                if !scenario && self.eat("input") {
                    let name = self.name()?;
                    self.expect(":")?;
                    let value_type = self.value_type()?;
                    let default = if self.eat("=") {
                        Some(self.literal()?)
                    } else {
                        None
                    };
                    if !self.input_names.insert(name.clone()) {
                        self.error("Duplicate component input");
                    }
                    file.inputs.push(Input {
                        name,
                        value_type,
                        default,
                        attributes,
                        range: start..self.last_end,
                    });
                } else {
                    file.statements
                        .push(self.statement_with_attributes(start, attributes)?);
                }
            }
        }
        Ok(())
    }
    fn value_type(&mut self) -> Result<String> {
        if matches!(
            self.peek(),
            "any" | "string" | "number" | "boolean" | "object" | "array"
        ) {
            Ok(self.bump().into())
        } else {
            self.error("Expected a value type");
            Err(())
        }
    }
    fn attributes(&mut self) -> Result<Vec<Attribute>> {
        let mut attributes = vec![];
        while self.peek() == "@" {
            let start = self.range().start;
            self.bump();
            let name = self.identifier()?;
            let mut arguments = vec![];
            if self.eat("(") {
                while !matches!(self.peek(), ")" | "") {
                    arguments.push(self.expr(0)?);
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(")")?;
            }
            attributes.push(Attribute {
                name,
                arguments,
                range: start..self.last_end,
            });
        }
        Ok(attributes)
    }
    fn block(&mut self) -> Result<Vec<Statement>> {
        self.expect("{")?;
        if self.depth >= MAX_DEPTH {
            self.error("Nesting exceeds the syntax limit");
            return Err(());
        }
        self.depth += 1;
        let mut statements = vec![];
        while !matches!(self.peek(), "}" | "") {
            let before = self.at;
            let start = self.range().start;
            let statement = self
                .attributes()
                .and_then(|attributes| self.statement_with_attributes(start, attributes));
            match statement {
                Ok(statement) => statements.push(statement),
                Err(()) => self.recover(before),
            }
            if self.at == before {
                self.bump();
            }
        }
        self.depth -= 1;
        self.expect("}")?;
        Ok(statements)
    }
    fn recover(&mut self, before: usize) {
        if self.at == before && self.peek() != "" {
            self.bump();
        }
        let mut nested = 0usize;
        while self.peek() != "" {
            let text = self.peek();
            if nested == 0
                && (text == "}"
                    || text == "@"
                    || text == "var"
                    || text == "export"
                    || (self
                        .token(0)
                        .is_some_and(|t| t.kind == TokenKind::Identifier)
                        && self.next_is("=")))
            {
                break;
            }
            if matches!(text, "{" | "[" | "(") {
                nested += 1;
            }
            if matches!(text, "}" | "]" | ")") {
                nested = nested.saturating_sub(1);
            }
            self.bump();
        }
    }
    fn statement_with_attributes(
        &mut self,
        start: usize,
        attributes: Vec<Attribute>,
    ) -> Result<Statement> {
        let variable = self.eat("var");
        let exported = self.eat("export");
        let handle_range = self.range();
        let handle = self.identifier()?;
        if !apiforge_core::handle::is_valid_handle(&handle) {
            self.error_at(handle_range, "Invalid or reserved block handle");
        }
        let value_type = if exported {
            self.expect(":")?;
            Some(self.value_type()?)
        } else {
            None
        };
        self.expect("=")?;
        let form = if let Some(value_type) = value_type {
            Form::Export {
                value_type,
                value: self.expr(0)?,
            }
        } else {
            self.form()?
        };
        if variable && !matches!(form, Form::Value(_)) {
            self.error("Only Value blocks may be marked var");
        }
        Ok(Statement {
            handle,
            variable,
            attributes,
            note: None,
            form,
            range: start..self.last_end,
        })
    }
    fn form(&mut self) -> Result<Form> {
        match self.peek() {
            "start" => {
                self.bump();
                Ok(Form::Start)
            }
            "assert" => {
                self.bump();
                Ok(Form::Assert(self.check()?))
            }
            "if" => {
                self.bump();
                let check = self.check()?;
                let then = self.block()?;
                let otherwise = if self.eat("else") {
                    Some(self.block()?)
                } else {
                    None
                };
                Ok(Form::If {
                    check,
                    then,
                    otherwise,
                })
            }
            "condition" => {
                self.bump();
                if !matches!(self.peek(), "all" | "any") {
                    self.error("Expected all or any");
                    return Err(());
                }
                let mode = self.bump().into();
                self.expect("{")?;
                let mut checks = vec![];
                while !matches!(self.peek(), "}" | "") {
                    checks.push(self.check()?);
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect("}")?;
                if checks.is_empty() {
                    self.error("A condition needs at least one check");
                }
                let value = if self.eat("with") {
                    Some(self.expr(0)?)
                } else {
                    None
                };
                Ok(Form::Condition {
                    mode,
                    checks,
                    value,
                })
            }
            "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS" => {
                let method = self.bump().into();
                let url = self.expr(0)?;
                let mut items = vec![];
                let mut seen = HashSet::new();
                if self.eat("{") {
                    while !matches!(self.peek(), "}" | "") {
                        let item = self.peek();
                        if item != "argument" && !seen.insert(item.to_owned()) {
                            self.error("Duplicate request field");
                        }
                        let value = match self.bump() {
                            "argument" => {
                                let name = self.name()?;
                                if !seen.insert(format!("argument:{name}")) {
                                    self.error("Duplicate request argument");
                                }
                                self.expect("=")?;
                                RequestItem::Argument {
                                    name,
                                    value: self.expr(0)?,
                                }
                            }
                            "headers" => RequestItem::Headers(self.object()?),
                            "query" => RequestItem::Query(self.object()?),
                            "body" => {
                                let text = self.eat("text");
                                RequestItem::Body {
                                    text,
                                    value: self.expr(0)?,
                                }
                            }
                            "timeout" => {
                                let number_end = self.range().end;
                                let value = self.number(false)?;
                                if self.range().start != number_end {
                                    self.error(
                                        "A duration unit must immediately follow its number",
                                    );
                                    return Err(());
                                }
                                let factor = match self.bump() {
                                    "ms" => 1.0,
                                    "s" => 1000.0,
                                    _ => {
                                        self.error("Expected ms or s duration unit");
                                        return Err(());
                                    }
                                };
                                let milliseconds = value.as_f64().unwrap_or(0.0) * factor;
                                if !milliseconds.is_finite()
                                    || milliseconds <= 0.0
                                    || milliseconds > 60_000.0
                                {
                                    self.error("Duration must be in (0, 60000] milliseconds");
                                    return Err(());
                                }
                                RequestItem::Timeout { milliseconds }
                            }
                            "contract" => RequestItem::Contract(self.plain_string()?),
                            "sample" => {
                                let status = self
                                    .number(false)?
                                    .as_u64()
                                    .and_then(|v| u16::try_from(v).ok());
                                let Some(status) = status.filter(|v| (100..=599).contains(v))
                                else {
                                    self.error("Expected an HTTP status from 100 to 599");
                                    return Err(());
                                };
                                RequestItem::Sample {
                                    status,
                                    value: self.literal()?,
                                }
                            }
                            _ => {
                                self.error("Unknown request field");
                                return Err(());
                            }
                        };
                        items.push(value);
                    }
                    self.expect("}")?;
                }
                Ok(Form::Request { method, url, items })
            }
            _ => Ok(Form::Value(self.expr(0)?)),
        }
    }
    fn check(&mut self) -> Result<Check> {
        let start = self.range().start;
        if matches!(self.peek(), "exists" | "nonempty") {
            let predicate = self.bump().into();
            let actual = self.expr(0)?;
            return Ok(Check {
                actual,
                predicate,
                expected: None,
                tolerance: None,
                range: start..self.last_end,
            });
        }
        let expr = self.expr(0)?;
        let explicit_predicate = matches!(&expr.kind, ExprKind::Binary { operator, .. } if matches!(operator.as_str(), "==" | "!=" | ">" | "<"));
        let (actual, predicate, expected) = match expr.kind {
            ExprKind::Binary {
                operator,
                left,
                right,
            } if matches!(operator.as_str(), "==" | "!=" | ">" | "<") => {
                (*left, operator, Some(*right))
            }
            kind => (
                Expr {
                    kind,
                    range: expr.range,
                },
                "==".into(),
                Some(Expr {
                    kind: ExprKind::Literal(Value::Bool(true)),
                    range: self.last_end..self.last_end,
                }),
            ),
        };
        let (predicate, expected, tolerance) = if matches!(self.peek(), "~=" | "contains" | "is") {
            if explicit_predicate {
                self.error("Parenthesize a comparison used as a check operand");
                return Err(());
            }
            // This form's left operand was parsed without a predicate token.
            let predicate = self.bump().to_owned();
            let expected = self.expr(0)?;
            let tolerance = if predicate == "~=" {
                self.expect("within")?;
                Some(self.numeric_literal()?)
            } else {
                None
            };
            (predicate, Some(expected), tolerance)
        } else {
            (predicate, expected, None)
        };
        Ok(Check {
            actual,
            predicate,
            expected,
            tolerance,
            range: start..self.last_end,
        })
    }
    fn number(&mut self, negative: bool) -> Result<Value> {
        if !self.token(0).is_some_and(|t| t.kind == TokenKind::Number) {
            self.error("Expected a JSON number");
            return Err(());
        }
        let range = self.range();
        let text = self.bump();
        let text = if negative {
            format!("-{text}")
        } else {
            text.to_owned()
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(value) if value.is_number() => Ok(value),
            _ => {
                self.error_at(range, "Invalid or non-finite JSON number");
                Err(())
            }
        }
    }
    fn numeric_literal(&mut self) -> Result<Expr> {
        let start = self.range().start;
        let negative = self.eat("-");
        let value = self.number(negative)?;
        Ok(Expr {
            kind: ExprKind::Literal(value),
            range: start..self.last_end,
        })
    }
    fn literal(&mut self) -> Result<Expr> {
        let value = self.expr(0)?;
        if literal_value(&value).is_none() {
            self.error_at(
                value.range.clone(),
                "Expected a literal JSON value without interpolation",
            );
            return Err(());
        }
        Ok(value)
    }
    fn expr(&mut self, minimum: u8) -> Result<Expr> {
        if self.depth >= MAX_DEPTH {
            self.error("Nesting exceeds the syntax limit");
            return Err(());
        }
        self.depth += 1;
        let value = self.expr_inner(minimum);
        self.depth -= 1;
        value
    }
    fn expr_inner(&mut self, minimum: u8) -> Result<Expr> {
        let start = self.range().start;
        let mut left = match self.peek() {
            "!" => {
                self.bump();
                let expr = self.expr(70)?;
                Expr {
                    range: start..self.last_end,
                    kind: ExprKind::Not(Box::new(expr)),
                }
            }
            "-" => self.numeric_literal()?,
            "(" => {
                self.bump();
                let expr = self.expr(0)?;
                self.expect(")")?;
                Expr {
                    range: start..self.last_end,
                    kind: ExprKind::Group(Box::new(expr)),
                }
            }
            "{" => self.object()?,
            "[" => {
                self.bump();
                let mut values = vec![];
                while !matches!(self.peek(), "]" | "") {
                    values.push(self.expr(0)?);
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect("]")?;
                Expr {
                    kind: ExprKind::Array(values),
                    range: start..self.last_end,
                }
            }
            "true" | "false" | "null" => Expr {
                kind: ExprKind::Literal(serde_json::from_str(self.bump()).unwrap()),
                range: start..self.last_end,
            },
            "?" => {
                self.bump();
                Expr {
                    kind: ExprKind::Missing,
                    range: start..self.last_end,
                }
            }
            _ => match self.token(0).map(|t| t.kind) {
                Some(TokenKind::String) => self.string()?,
                Some(TokenKind::Number) => {
                    let value = self.number(false)?;
                    Expr {
                        kind: ExprKind::Literal(value),
                        range: start..self.last_end,
                    }
                }
                Some(TokenKind::Identifier) => Expr {
                    kind: ExprKind::Name(self.bump().into()),
                    range: start..self.last_end,
                },
                _ => {
                    self.error("Expected an expression");
                    return Err(());
                }
            },
        };
        loop {
            if self.eat(".") {
                let key = self.identifier()?;
                left = Expr {
                    kind: ExprKind::Field {
                        base: Box::new(left),
                        key,
                        index: false,
                    },
                    range: start..self.last_end,
                };
            } else if self.eat("[") {
                let (key, index) = if self.token(0).is_some_and(|t| t.kind == TokenKind::Number) {
                    let value = self.number(false)?;
                    let Some(index) = value.as_u64() else {
                        self.error("Expected a non-negative integer path index");
                        return Err(());
                    };
                    (index.to_string(), true)
                } else {
                    (self.plain_string()?, false)
                };
                self.expect("]")?;
                left = Expr {
                    kind: ExprKind::Field {
                        base: Box::new(left),
                        key,
                        index,
                    },
                    range: start..self.last_end,
                };
            } else if self.peek() == "("
                || (self.peek() == "@"
                    && self.token(1).is_some_and(|t| t.kind == TokenKind::Number))
            {
                let name = match &left.kind {
                    ExprKind::Name(name) => name.clone(),
                    ExprKind::Field { base, key, .. } if matches!(&base.kind, ExprKind::Name(name) if name == "op") =>
                    {
                        format!("op.{key}")
                    }
                    _ => {
                        self.error("Only components and named operators can be called");
                        return Err(());
                    }
                };
                let revision = if self.eat("@") {
                    let revision = self
                        .number(false)?
                        .as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .filter(|v| *v > 0);
                    if revision.is_none() {
                        self.error("Expected a positive component revision");
                        return Err(());
                    }
                    revision
                } else {
                    None
                };
                self.expect("(")?;
                let mut arguments = vec![];
                let mut names = HashSet::new();
                let mut named = None;
                while !matches!(self.peek(), ")" | "") {
                    let name = if self.next_is(":") {
                        let name = self.name()?;
                        self.expect(":")?;
                        Some(name)
                    } else {
                        None
                    };
                    if named.is_some_and(|old| old != name.is_some()) {
                        self.error("Do not mix positional and named arguments");
                        return Err(());
                    }
                    named = Some(name.is_some());
                    if let Some(name) = &name
                        && !names.insert(name.clone())
                    {
                        self.error("Duplicate argument");
                    }
                    arguments.push(Argument {
                        name,
                        value: self.expr(0)?,
                    });
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(")")?;
                left = Expr {
                    kind: ExprKind::Call {
                        name,
                        revision,
                        arguments,
                    },
                    range: start..self.last_end,
                };
            } else {
                let Some(power) = precedence(self.peek()).filter(|power| *power >= minimum) else {
                    break;
                };
                let operator = self.bump().to_owned();
                let right = self.expr(power + 1)?;
                left = Expr {
                    kind: ExprKind::Binary {
                        operator,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    range: start..self.last_end,
                };
            }
            if expression_depth(&left) >= MAX_DEPTH {
                self.error("Expression exceeds the syntax depth limit");
                return Err(());
            }
        }
        Ok(left)
    }
    fn object(&mut self) -> Result<Expr> {
        let start = self.range().start;
        self.expect("{")?;
        let mut fields = vec![];
        let mut keys = HashSet::new();
        while !matches!(self.peek(), "}" | "") {
            let key = self.name()?;
            self.expect(":")?;
            if !keys.insert(key.clone()) {
                self.error("Duplicate object key");
            }
            fields.push((key, self.expr(0)?));
            if !self.eat(",") {
                break;
            }
        }
        self.expect("}")?;
        Ok(Expr {
            kind: ExprKind::Object(fields),
            range: start..self.last_end,
        })
    }
    fn string(&mut self) -> Result<Expr> {
        if !self.token(0).is_some_and(|t| t.kind == TokenKind::String) {
            self.error("Expected a string");
            return Err(());
        }
        let range = self.range();
        let raw = self.bump();
        let triple = raw.starts_with("\"\"\"");
        let width = if triple { 3 } else { 1 };
        if raw.len() < width * 2 || !raw.ends_with(if triple { "\"\"\"" } else { "\"" }) {
            return Err(());
        }
        let inner = &raw[width..raw.len() - width];
        let mut at = 0;
        let mut text_start = 0;
        let mut parts = vec![];
        while at < inner.len() {
            if inner.as_bytes()[at] == b'\\' {
                at += 1;
                if at < inner.len() {
                    at += inner[at..].chars().next().unwrap().len_utf8();
                }
            } else if inner[at..].starts_with("{{") {
                let text = decode_fragment(&inner[text_start..at], triple)
                    .ok_or_else(|| self.error_at(range.clone(), "Invalid string escape"))?;
                if !text.is_empty() {
                    parts.push(Part::Text(text));
                }
                let (end, valid) = lexer::interpolation_end(inner, at + 2, self.depth);
                if !valid {
                    self.error("Unterminated interpolation");
                    return Err(());
                }
                let fragment = &inner[at + 2..end - 2];
                let (tokens, errors, omitted) = lexer::lex_counted(fragment);
                self.omitted += omitted;
                let base = range.start + width + at + 2;
                self.append_diagnostics(errors.into_iter().map(|mut d| {
                    d.range = d.range.start + base..d.range.end + base;
                    d
                }));
                let mut nested = Parser::new(fragment, &tokens, base, self.depth + 1);
                let expr = nested.expr(0);
                if nested.peek() != "" {
                    nested.error("Unexpected text in interpolation");
                }
                self.omitted += nested.omitted;
                self.append_diagnostics(nested.diagnostics);
                parts.push(Part::Value(expr?));
                at = end;
                text_start = at;
            } else {
                at += inner[at..].chars().next().unwrap().len_utf8();
            }
        }
        let text = decode_fragment(&inner[text_start..], triple)
            .ok_or_else(|| self.error_at(range.clone(), "Invalid string escape"))?;
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        if triple {
            dedent(&mut parts, inner);
        }
        let kind = if parts.iter().all(|p| matches!(p, Part::Text(_))) {
            ExprKind::Literal(Value::String(
                parts
                    .into_iter()
                    .map(|p| match p {
                        Part::Text(t) => t,
                        _ => unreachable!(),
                    })
                    .collect(),
            ))
        } else {
            ExprKind::Template(parts)
        };
        Ok(Expr { kind, range })
    }
}

pub(crate) fn precedence(operator: &str) -> Option<u8> {
    Some(match operator {
        "||" => 10,
        "&&" => 20,
        "==" | "!=" => 30,
        "<" | ">" | "<=" | ">=" => 40,
        "+" | "-" => 50,
        "*" | "/" | "%" => 60,
        _ => return None,
    })
}

fn expression_depth(expr: &Expr) -> usize {
    1 + match &expr.kind {
        ExprKind::Binary { left, right, .. } => expression_depth(left).max(expression_depth(right)),
        ExprKind::Field { base, .. } | ExprKind::Not(base) | ExprKind::Group(base) => {
            expression_depth(base)
        }
        _ => 0,
    }
}

pub(crate) fn literal_value(expr: &Expr) -> Option<Value> {
    Some(match &expr.kind {
        ExprKind::Literal(value) => value.clone(),
        ExprKind::Group(value) => return literal_value(value),
        ExprKind::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| Some((key.clone(), literal_value(value)?)))
                .collect::<Option<_>>()?,
        ),
        ExprKind::Array(items) => {
            Value::Array(items.iter().map(literal_value).collect::<Option<_>>()?)
        }
        _ => return None,
    })
}

fn decode_fragment(raw: &str, triple: bool) -> Option<String> {
    let mut json = String::from("\"");
    let mut characters = raw.chars().peekable();
    while let Some(c) = characters.next() {
        match c {
            '\\' => {
                let next = characters.next()?;
                if next == '{' && characters.peek() == Some(&'{') {
                    json.push('{');
                } else {
                    json.push('\\');
                    json.push(next);
                }
            }
            '"' if triple => json.push_str("\\\""),
            '\n' if triple => json.push_str("\\n"),
            '\r' if triple => json.push_str("\\r"),
            '\t' if triple => json.push_str("\\t"),
            other => json.push(other),
        }
    }
    json.push('"');
    serde_json::from_str(&json).ok()
}

fn dedent(parts: &mut Vec<Part>, raw: &str) {
    let indentation = raw
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.bytes()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count()
        })
        .min()
        .unwrap_or(0);
    let mut line_start = false;
    for part in parts.iter_mut() {
        if let Part::Text(text) = part {
            let mut output = String::new();
            let mut removed = 0;
            for c in text.chars() {
                if line_start && removed < indentation && matches!(c, ' ' | '\t') {
                    removed += 1;
                    continue;
                }
                output.push(c);
                line_start = c == '\n';
                if line_start {
                    removed = 0;
                }
            }
            *text = output;
        } else {
            line_start = false;
        }
    }
    if let Some(Part::Text(text)) = parts.first_mut() {
        if text.starts_with("\r\n") {
            text.drain(..2);
        } else if text.starts_with('\n') {
            text.remove(0);
        }
    }
    if let Some(Part::Text(text)) = parts.last_mut() {
        if text.ends_with("\r\n") {
            text.truncate(text.len() - 2);
        } else if text.ends_with('\n') {
            text.pop();
        }
    }
    parts.retain(|p| !matches!(p, Part::Text(text) if text.is_empty()));
}

fn visit_statements(file: &mut File, mut visit: impl FnMut(&mut Statement)) {
    fn walk(statements: &mut [Statement], visit: &mut impl FnMut(&mut Statement)) {
        for statement in statements {
            if let Form::If {
                then, otherwise, ..
            } = &mut statement.form
            {
                walk(then, visit);
                if let Some(otherwise) = otherwise {
                    walk(otherwise, visit);
                }
            }
            visit(statement);
        }
    }
    walk(&mut file.statements, &mut visit);
    if let Some(cleanup) = &mut file.cleanup {
        walk(cleanup, &mut visit);
    }
}

fn attach_comments(
    source: &str,
    tokens: &[Token],
    file: &mut File,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    let comments: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Comment)
        .collect();
    let mut used = HashSet::new();
    // Inner statements are visited first, so a trailing comment belongs to the
    // nearest statement rather than its enclosing IF.
    visit_statements(file, |statement| {
        let mut note = vec![];
        let mut end = statement.range.start;
        let leading = comments.partition_point(|comment| comment.range.end <= end);
        for (index, comment) in comments[..leading].iter().enumerate().rev() {
            if used.contains(&index) {
                break;
            }
            let gap = &source[comment.range.end..end];
            if !gap.trim().is_empty() || gap.matches('\n').count() > 1 {
                break;
            }
            note.push((index, comment));
            end = comment.range.start;
        }
        note.reverse();
        let trailing =
            comments.partition_point(|comment| comment.range.start < statement.range.end);
        if let Some(comment) = comments.get(trailing)
            && !used.contains(&trailing)
        {
            let gap = &source[statement.range.end..comment.range.start];
            if gap.trim().is_empty() && !gap.contains('\n') {
                note.push((trailing, comment));
            }
        }
        if !note.is_empty() {
            statement.note = Some(
                note.into_iter()
                    .map(|(index, token)| {
                        used.insert(index);
                        let text =
                            source[token.range.start + 2..token.range.end].trim_end_matches('\r');
                        text.strip_prefix(' ').unwrap_or(text).to_owned()
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    });
    let mut omitted = 0;
    for (index, token) in comments.iter().enumerate() {
        if used.contains(&index) {
            continue;
        }
        if diagnostics.len() < MAX_DIAGNOSTICS {
            diagnostics.push(Diagnostic {
                range: token.range.clone(),
                severity: Severity::Warning,
                message: crate::UNATTACHED_COMMENT.into(),
                fix: None,
            });
        } else {
            omitted += 1;
        }
    }
    omitted
}
