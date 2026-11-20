//! The fix of a file written for language 1 (LANG-13). Edit, fix and format
//! are three operations (#411): the fix changes only what the written
//! language requires. It deletes the `@after` lines of a file that is written
//! in the order it runs today and writes `begin = start` where a flow with a
//! step has none; every other byte stays, comments included. A file it cannot
//! fix without changing what runs is left as it is, and the statement the
//! person places by hand is named.
use crate::{
    ast::*,
    lower::{Document, LowerContext, lower},
    parse,
};
use apiforge_core::{
    chain::{Unchained, chained},
    format::Graph,
    model::Branch,
};
use std::{collections::HashMap, collections::HashSet, ops::Range};

/// An `@after` in a source.
pub const AFTER_REMOVED: &str = "`@after` is not part of the language: the steps of a block run in the order they are written; delete this line";
/// A main flow with a step and no `start`.
pub const START_MISSING: &str = "A scenario with a step needs `start` as its first step; the fix writes it above the first statement";
/// A step written above `start`.
pub const START_NOT_FIRST: &str =
    "`start` must be the first step of the flow; move the steps written above it below it";
/// The fix is refused: a step nothing orders after `start` today.
pub const RUNS_NOWHERE: &str = "This step runs nowhere today: nothing orders it after `start`; write it where it should run or delete it, then delete the `@after` lines";
/// The fix is refused: a statement written elsewhere than where it runs.
pub const NOT_IN_PLACE: &str = "This statement is not written where it runs today; move it there, then delete the `@after` lines";

/// The text of a file after its fix, or as it was when the fix is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixed {
    pub text: String,
    /// Why the text was left as it is: the statement the person places by
    /// hand, or the first syntax error. `None` when the fix was written or
    /// there was nothing to fix.
    pub refused: Option<Diagnostic>,
}

impl Fixed {
    /// Whether the fix changed the text.
    pub fn changed(&self, text: &str) -> bool {
        self.text != text
    }
}

/// `text` with the fix of the written language applied: its `@after` lines
/// deleted and a missing `begin = start` written, when the file is written in
/// the order it runs today. Applied again it changes nothing. The window, the
/// command line and the MCP server call this one function.
pub fn fix_source(text: &str, context: &LowerContext<'_>) -> Fixed {
    let unchanged = |refused| Fixed {
        text: text.to_owned(),
        refused,
    };
    let tree = parse(text);
    if let Some(error) = tree
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return unchanged(Some(error.clone()));
    }
    let context = context.clone().written();
    let lowered = lower(&tree, &context);
    if let Some(refused) = lowered
        .diagnostics
        .iter()
        .find(|d| matches!(d.message.as_str(), RUNS_NOWHERE | NOT_IN_PLACE))
    {
        return unchanged(Some(refused.clone()));
    }
    // Lowering offers a fix only when the stored reading of the same text,
    // written as a chain, is the written reading (`run_order`). A text with
    // nothing to fix is returned as it is: whether it is a language-1 text
    // that would run otherwise (a request nothing ordered after `start`) is
    // known only from its project's manifest, which `apiforge fix --folder`
    // (LANG-20) reads.
    let mut fixes = vec![];
    for diagnostic in &lowered.diagnostics {
        if !matches!(diagnostic.message.as_str(), AFTER_REMOVED | START_MISSING) {
            continue;
        }
        match &diagnostic.fix {
            Some(fix) => fixes.push(fix.clone()),
            None => return unchanged(Some(diagnostic.clone())),
        }
    }
    // From the end, so earlier ranges stay valid; a deletion before an
    // insertion at the same place.
    fixes.sort_by_key(|fix| std::cmp::Reverse((fix.range.start, fix.range.end)));
    let applied = fixes.len();
    let mut fixed = text.to_owned();
    for fix in fixes {
        fixed.replace_range(fix.range, &fix.replacement);
    }
    // What the fix writes has no error the text did not have.
    let errors = |diagnostics: &[Diagnostic]| {
        diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    };
    let after = lower(&parse(&fixed), &context);
    if errors(&after.diagnostics) + applied > errors(&lowered.diagnostics) {
        let new = after
            .diagnostics
            .iter()
            .find(|d| d.severity == Severity::Error)
            .cloned();
        return unchanged(new);
    }
    Fixed {
        text: fixed,
        refused: None,
    }
}

/// The fix that removes the attribute at `range` and nothing else: the
/// comments written inside it stay, each on a line of its own where the
/// attribute stood (review of #488).
pub(crate) fn remove_attribute(tree: &SyntaxTree, range: Range<usize>) -> Fix {
    let source = &tree.source;
    let comments: Vec<&str> = tree
        .tokens
        .iter()
        .filter(|token| {
            token.kind == crate::lexer::TokenKind::Comment
                && range.start <= token.range.start
                && token.range.start < range.end
        })
        .map(|token| source[token.range.clone()].trim_end())
        .collect();
    let removed = line_of(source, range.clone());
    if comments.is_empty() {
        return Fix {
            range: removed,
            replacement: String::new(),
        };
    }
    let line = source[..range.start].rfind('\n').map_or(0, |at| at + 1);
    let indent: String = source[line..range.start]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let alone = removed.start == line;
    let replacement = if alone {
        comments
            .iter()
            .map(|comment| format!("{indent}{comment}{newline}"))
            .collect()
    } else {
        // The attribute shares its last line with its statement: the comments
        // go on their own lines before it.
        comments
            .iter()
            .map(|comment| format!("{comment}{newline}{indent}"))
            .collect()
    };
    Fix {
        range: removed,
        replacement,
    }
}

/// The line `range` stands on, with its line break, when nothing else is
/// written on it; otherwise `range` and the spaces after it.
pub(crate) fn line_of(source: &str, range: Range<usize>) -> Range<usize> {
    let start = source[..range.start].rfind('\n').map_or(0, |at| at + 1);
    let end = source[range.end..]
        .find('\n')
        .map_or(source.len(), |at| range.end + at + 1);
    if source[start..range.start].trim().is_empty() && source[range.end..end].trim().is_empty() {
        return start..end;
    }
    let spaces = source[range.end..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .count();
    range.start..range.end + spaces
}

/// A fix that writes `statement` on a line of its own above the line `at`
/// stands on, with that line's indentation and line break.
pub(crate) fn insert_before(source: &str, at: usize, statement: &str) -> Fix {
    let start = source[..at].rfind('\n').map_or(0, |at| at + 1);
    let indent = &source[start..at];
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if indent.trim().is_empty() {
        Fix {
            range: start..start,
            replacement: format!("{indent}{statement}{newline}"),
        }
    } else {
        Fix {
            range: at..at,
            replacement: format!("{statement}{newline}  "),
        }
    }
}

/// Whether the file, lowered in the written language as `written`, runs as
/// the stored spelling of the same text runs today: each graph of the stored
/// reading, written as a chain ([`chained`]), has exactly the ordering edges
/// of the written reading. Otherwise the first statement out of place, by its
/// range, and why.
pub(crate) fn run_order(
    tree: &SyntaxTree,
    context: &LowerContext<'_>,
    written: &Document,
) -> Result<(), Option<(Range<usize>, &'static str)>> {
    let file = tree.file.as_ref().ok_or(None)?;
    let stored = lower(tree, &context.clone().stored())
        .document
        .ok_or(None)?;
    let mut ranges = HashMap::new();
    statement_ranges(&file.statements, &mut ranges);
    statement_ranges(file.cleanup.as_deref().unwrap_or_default(), &mut ranges);
    let place = |id: &str, message| {
        ranges
            .get(id)
            .map(|range: &Range<usize>| (range.clone(), message))
    };
    for (stored, written) in graphs(&stored).into_iter().zip(graphs(written)) {
        let expected = chained(stored).map_err(|refused| match refused {
            Unchained::RunsNowhere(step) => place(&step, RUNS_NOWHERE),
            Unchained::NotAChain(step) => place(&step, NOT_IN_PLACE),
        })?;
        let incoming = |graph: &Graph| {
            let mut edges: HashMap<String, HashSet<(String, Option<Branch>)>> = HashMap::new();
            for edge in &graph.edges {
                edges
                    .entry(edge.target.clone())
                    .or_default()
                    .insert((edge.source.clone(), edge.branch));
            }
            edges
        };
        let (expected, actual) = (incoming(&expected), incoming(written));
        if expected == actual {
            continue;
        }
        let empty = HashSet::new();
        let out_of_place = written
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .chain(expected.keys().map(String::as_str))
            .find(|id| expected.get(*id).unwrap_or(&empty) != actual.get(*id).unwrap_or(&empty));
        return Err(out_of_place.and_then(|id| place(id, NOT_IN_PLACE)));
    }
    Ok(())
}

fn graphs(document: &Document) -> Vec<&Graph> {
    match document {
        Document::Scenario(scenario) => vec![&scenario.graph, &scenario.cleanup],
        Document::Component(component) => vec![&component.graph],
    }
}

fn statement_ranges(statements: &[Statement], ranges: &mut HashMap<String, Range<usize>>) {
    for statement in statements {
        ranges.insert(statement.handle.clone(), statement.range.clone());
        if let Form::If {
            then, otherwise, ..
        } = &statement.form
        {
            statement_ranges(then, ranges);
            statement_ranges(otherwise.as_deref().unwrap_or_default(), ranges);
        }
    }
}
