use apiforge_script::{ast::*, format, parse};

fn normalize(mut expr: Expr) -> Expr {
    expr.range = 0..0;
    match &mut expr.kind {
        ExprKind::Group(inner) => return normalize((**inner).clone()),
        ExprKind::Object(fields) => {
            for (_, value) in fields {
                *value = normalize(value.clone());
            }
        }
        ExprKind::Array(items) => {
            for value in items {
                *value = normalize(value.clone());
            }
        }
        ExprKind::Template(parts) => {
            for part in parts {
                if let Part::Value(value) = part {
                    *value = normalize(value.clone());
                }
            }
        }
        ExprKind::Field { base, .. } | ExprKind::Not(base) => **base = normalize((**base).clone()),
        ExprKind::Call { arguments, .. } => {
            for argument in arguments {
                argument.value = normalize(argument.value.clone());
            }
        }
        ExprKind::Binary { left, right, .. } => {
            **left = normalize((**left).clone());
            **right = normalize((**right).clone());
        }
        ExprKind::Literal(_) | ExprKind::Name(_) | ExprKind::Missing => {}
    }
    expr
}

fn random(state: &mut u64) -> usize {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 32) as usize
}

fn generate(state: &mut u64, depth: usize) -> String {
    let leaves = [
        "null",
        "true",
        "false",
        "0",
        "-0.5e-2",
        "12345678901234567890",
        "\"żółć 🦀\"",
        "\"\\{{literal}}\"",
        "?",
        "env[\"API-TOKEN\"]",
        "param[\"a b\"].items[0]",
        "products.body[\"key.with.dots\"]",
    ];
    let choice = random(state);
    if depth == 0 {
        return leaves[choice % leaves.len()].into();
    }
    let left = generate(state, depth - 1);
    let right = generate(state, depth - 1);
    match choice % 10 {
        0..=2 => {
            let operators = [
                "+", "-", "*", "/", "%", "==", "!=", ">", "<", ">=", "<=", "&&", "||",
            ];
            format!(
                "({left}) {} ({right})",
                operators[random(state) % operators.len()]
            )
        }
        3 => format!("!({left})"),
        4 => format!("{{ first: {left}, \"a b\": {right} }}"),
        5 => format!("[{left}, {right},]"),
        6 => format!("op.add(left: {left}, right: {right})"),
        7 => format!("concat({left}, {right})"),
        8 => format!("({left})[\"a b\"][0].id"),
        _ => format!("\"prefix {{{{{left}}}}} / {{{{{right}}}}} suffix\""),
    }
}

fn value(source: &str) -> Expr {
    let tree = parse(source);
    assert!(!tree.has_errors(), "{source}\n{:?}", tree.diagnostics);
    let statement = tree.file.unwrap().statements.remove(0);
    let Form::Value(expr) = statement.form else {
        panic!()
    };
    normalize(expr)
}

#[test]
fn ten_thousand_generated_expressions_keep_their_structure_after_formatting() {
    let mut state = 0x5eed_cafe_u64;
    for _ in 0..10_000 {
        let expr = generate(&mut state, 3);
        let source = format!("scenario \"Generated\" {{ value = {expr} }}");
        let expected = value(&source);
        let formatted = format(&source).unwrap();
        assert_eq!(value(&formatted), expected, "{source}\n{formatted}");
        assert_eq!(format(&formatted).unwrap(), formatted);
    }
}

#[test]
fn checked_in_examples_are_already_formatted() {
    for source in [
        include_str!("../../../fixtures/scripts/checkout.afs"),
        include_str!("../../../fixtures/scripts/lookup.afs"),
    ] {
        assert_eq!(format(source).unwrap(), source);
    }
}

#[test]
fn many_comment_notes_keep_their_statement_association() {
    let mut source = String::from("scenario \"Many\" {\n");
    for index in 0..10_000 {
        source.push_str(&format!("// Note {index}\nn{index} = {index}\n"));
    }
    source.push_str("}\n");
    let tree = parse(&source);
    assert!(!tree.has_errors());
    for (index, statement) in tree.file.unwrap().statements.iter().enumerate() {
        assert_eq!(
            statement.note.as_deref(),
            Some(format!("Note {index}").as_str())
        );
    }
    assert_eq!(
        format(&format(&source).unwrap()).unwrap(),
        format(&source).unwrap()
    );
}

use apiforge_script::complete::{self, Mode, Request, Scope};
use apiforge_script::{LowerContext, lexer::lex, lower};

const PIECES: [&str; 45] = [
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
    "GET",
    "POST",
    "{",
    "}",
    "[",
    "]",
    "(",
    ")",
    "\"",
    "\"a\"",
    "'",
    "=",
    "==",
    ".",
    ",",
    ":",
    "@",
    "//",
    "{{",
    "}}",
    "ż",
];

fn fragment(state: &mut u64) -> String {
    let count = random(state) % 24;
    let mut out = String::new();
    for _ in 0..count {
        out.push_str(PIECES[random(state) % PIECES.len()]);
        if random(state).is_multiple_of(3) {
            out.push(' ');
        }
        if random(state).is_multiple_of(7) {
            out.push('\n');
        }
    }
    out
}

/// The editor colours, parses, completes and describes whatever is in the
/// buffer, keystroke by keystroke, so none of it may panic on text that is not
/// yet a scenario. Every caret is tried, because a caret is where completion
/// and hover both start.
#[test]
fn half_typed_text_never_breaks_what_the_editor_calls_on_every_keystroke() {
    let project = apiforge_core::demos::empty_project();
    let mut state = 0x5eed_1234_u64;
    for _ in 0..1_000 {
        let source = fragment(&mut state);
        let _ = lex(&source);
        let tree = parse(&source);
        let _ = lower(&tree, &LowerContext::document(&project));
        // Every caret the editor can hand over is a character boundary.
        for (cursor, _) in source
            .char_indices()
            .chain(std::iter::once((source.len(), ' ')))
        {
            let request = Request {
                source: &source,
                cursor,
                mode: if cursor % 2 == 0 {
                    Mode::Script
                } else {
                    Mode::Template
                },
                scope: Scope::default(),
            };
            let _ = complete::complete(&request);
            let _ = complete::describe(&request);
            let _ = complete::statement_at(&source, cursor);
        }
        let _ = complete::statement_range(&source, "a");
    }
}
