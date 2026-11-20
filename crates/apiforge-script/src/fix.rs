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
    chain::{Braces, Unchained, braces, chain, chained, is_step},
    format::{Graph, OrderEdge},
    model::{Branch, NodeKind},
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
    fixed_with_edits(text, context).0
}

/// [`fix_source`], with the fixes it applied, as ranges of `text` in source
/// order: none when it changed nothing.
fn fixed_with_edits(text: &str, context: &LowerContext<'_>) -> (Fixed, Vec<Fix>) {
    let unchanged = |refused| {
        (
            Fixed {
                text: text.to_owned(),
                refused,
            },
            vec![],
        )
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
    for fix in &fixes {
        fixed.replace_range(fix.range.clone(), &fix.replacement);
    }
    fixes.reverse();
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
    (
        Fixed {
            text: fixed,
            refused: None,
        },
        fixes,
    )
}

/// What a source written for language 1 becomes in the written language
/// (LANG-20): judged always by its 0.1.0 reading, also without an `@after`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Judged {
    /// The text in the written language, runs as today: the same text when
    /// nothing changes. `edits` are the fixes applied, as ranges of the
    /// original text, in source order.
    Convertible { text: String, edits: Vec<Fix> },
    /// The statements that stand in the way, in source order; nothing is
    /// converted.
    Blocked(Vec<Blocker>),
    /// The text does not parse: its first syntax error.
    Unparsable(Diagnostic),
}

/// A statement that stands in the way of a conversion, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocker {
    /// The statement's range in the text.
    pub range: Range<usize>,
    /// The statement's handle; empty for an error of the check that names
    /// no statement.
    pub handle: String,
    pub reason: Blocked,
}

/// Why a statement stands in the way of a conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// A step that runs nowhere today: nothing orders it after `start`.
    /// `above` is the step written above it in its braces, whose `@after` the
    /// person may write on it instead of deleting it.
    RunsNowhere { above: Option<String> },
    /// A statement written elsewhere than where it runs today: `after` names
    /// the statements it runs after today, by handle.
    NotInPlace { after: Vec<String> },
    /// The converted text would not pass the check of the written language.
    Invalid(Diagnostic),
}

/// Judges `text`, a source of a project written in language 1, by its
/// 0.1.0 reading, whether or not it holds an `@after`: in its main flow, its
/// cleanup and a component's body, the steps it runs today, in their order,
/// must be the steps written, in the order written, and the converted text
/// must pass the check of the written language. [`fix_source`] makes the
/// comparison only for a file with an `@after` or without its `start`;
/// `apiforge fix --folder` makes it for every source with this function.
pub fn judge_language_1(text: &str, context: &LowerContext<'_>) -> Judged {
    let tree = parse(text);
    if let Some(error) = tree
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Judged::Unparsable(error.clone());
    }
    let written_context = context.clone().written();
    let lowered = lower(&tree, &written_context);
    let mut blockers = match &lowered.document {
        Some(written) => out_of_order(&tree, context, written),
        None => vec![],
    };
    // The errors of the written check that are not the fix's own: the text
    // would not pass it once converted. The fix's own are judged above, and
    // an error of the whole file (a cycle the written order makes) follows
    // from a statement out of its place, which is named instead.
    let ordered = blockers.is_empty();
    blockers.extend(
        lowered
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error && !fix_message(&d.message))
            .filter(|d| ordered || d.range != (0..text.len()))
            .map(invalid),
    );
    if blockers.is_empty() {
        let (fixed, edits) = fixed_with_edits(text, context);
        if let Some(refused) = fixed.refused {
            blockers.push(invalid(&refused));
        } else {
            let after = lower(&parse(&fixed.text), &written_context);
            blockers.extend(
                after
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .map(|d| Blocker {
                        // A range of the converted text says nothing of the
                        // original: the error is the file's.
                        range: 0..0,
                        handle: String::new(),
                        reason: Blocked::Invalid(d.clone()),
                    }),
            );
            if blockers.is_empty() {
                return Judged::Convertible {
                    text: fixed.text,
                    edits,
                };
            }
        }
    }
    blockers.sort_by_key(|blocker| (blocker.range.start, blocker.range.end));
    Judged::Blocked(blockers)
}

/// The messages of the written language that the conversion itself
/// resolves or judges by the 0.1.0 reading.
fn fix_message(message: &str) -> bool {
    matches!(
        message,
        AFTER_REMOVED | START_MISSING | START_NOT_FIRST | RUNS_NOWHERE | NOT_IN_PLACE
    )
}

fn invalid(diagnostic: &Diagnostic) -> Blocker {
    Blocker {
        range: diagnostic.range.clone(),
        handle: String::new(),
        reason: Blocked::Invalid(diagnostic.clone()),
    }
}

/// Every statement whose place in the written reading is not where the
/// 0.1.0 reading runs it, in each graph of the document. After each one
/// found, it is taken out of both readings, its neighbours joined, so that
/// the next is judged on its own.
fn out_of_order(tree: &SyntaxTree, context: &LowerContext<'_>, written: &Document) -> Vec<Blocker> {
    let Some(file) = tree.file.as_ref() else {
        return vec![];
    };
    let Some(stored) = lower(tree, &context.clone().stored()).document else {
        return vec![];
    };
    let mut ranges = HashMap::new();
    statement_ranges(&file.statements, &mut ranges);
    statement_ranges(file.cleanup.as_deref().unwrap_or_default(), &mut ranges);
    let mut attributes = HashMap::new();
    attribute_ends(&file.statements, &mut attributes);
    attribute_ends(file.cleanup.as_deref().unwrap_or_default(), &mut attributes);
    // A statement is named from its handle, after its attributes and the
    // comments written between them and it.
    for (handle, range) in &mut ranges {
        if let Some(end) = attributes.get(handle)
            && let Some(token) = tree.tokens.iter().find(|token| {
                token.range.start >= *end
                    && !matches!(
                        token.kind,
                        crate::lexer::TokenKind::Whitespace | crate::lexer::TokenKind::Comment
                    )
            })
            && token.range.start < range.end
        {
            range.start = token.range.start;
        }
    }
    let mut blockers = vec![];
    for (stored, written) in graphs(&stored).into_iter().zip(graphs(written)) {
        let (mut stored, mut written) = (stored.clone(), written.clone());
        // Each round takes one statement out, so this ends.
        for _ in 0..=written.nodes.len() + stored.nodes.len() {
            let Some((id, reason)) = first_out_of_order(&stored, &written) else {
                break;
            };
            if let Some(range) = ranges.get(&id) {
                blockers.push(Blocker {
                    range: range.clone(),
                    handle: id.clone(),
                    reason,
                });
            }
            if !take_out(&mut stored, &mut written, &id) {
                break;
            }
        }
    }
    blockers
}

/// The first statement of `written` out of the order `stored` runs today,
/// and why, or `None` when the two agree.
fn first_out_of_order(stored: &Graph, written: &Graph) -> Option<(String, Blocked)> {
    let handle = |graph: &Graph, id: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map_or_else(|| id.to_owned(), |node| node.handle.clone())
    };
    let sources = |graph: &Graph, id: &str| {
        let mut sources: Vec<String> = vec![];
        for edge in graph.edges.iter().filter(|edge| edge.target == id) {
            let source = handle(graph, &edge.source);
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        sources
    };
    let expected = match chained(stored) {
        Ok(expected) => expected,
        Err(Unchained::RunsNowhere(id)) => {
            let above = step_above(written, &id).map(|above| handle(written, &above));
            return Some((id, Blocked::RunsNowhere { above }));
        }
        Err(Unchained::NotAChain(id)) => {
            let after = sources(stored, &id);
            return Some((id, Blocked::NotInPlace { after }));
        }
    };
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
    let (wanted, actual) = (incoming(&expected), incoming(written));
    let empty = HashSet::new();
    let id = written
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .chain(expected.nodes.iter().map(|node| node.id.as_str()))
        .find(|id| wanted.get(*id).unwrap_or(&empty) != actual.get(*id).unwrap_or(&empty))?
        .to_owned();
    let after = sources(&expected, &id);
    Some((id, Blocked::NotInPlace { after }))
}

/// The step written above `id` in its braces, in the written reading.
fn step_above(written: &Graph, id: &str) -> Option<String> {
    let braces = braces(written);
    let own = braces.get(id).copied();
    let mut above = None;
    for node in &written.nodes {
        if node.id == id {
            return above;
        }
        if is_step(node.kind) && braces.get(node.id.as_str()).copied() == own {
            above = Some(node.id.clone());
        }
    }
    None
}

/// Takes the statement `id` out of both readings: the 0.1.0 reading joins
/// what ran before it to what ran after it; the written reading chains its
/// neighbours, as a deleted line would. An `if`, whose braces hold other
/// statements, is not taken out: `false`, and the judgement of this graph
/// stops at it.
fn take_out(stored: &mut Graph, written: &mut Graph, id: &str) -> bool {
    let is_if = |graph: &Graph| {
        graph
            .nodes
            .iter()
            .any(|node| node.id == id && node.kind == NodeKind::If)
    };
    if is_if(stored) || is_if(written) {
        return false;
    }
    let before: Vec<(String, Option<Branch>)> = stored
        .edges
        .iter()
        .filter(|edge| edge.target == id)
        .map(|edge| (edge.source.clone(), edge.branch))
        .collect();
    let after: Vec<String> = stored
        .edges
        .iter()
        .filter(|edge| edge.source == id)
        .map(|edge| edge.target.clone())
        .collect();
    stored.nodes.retain(|node| node.id != id);
    stored
        .edges
        .retain(|edge| edge.source != id && edge.target != id);
    for (source, branch) in &before {
        for target in &after {
            let joined = stored.edges.iter().any(|edge| {
                edge.source == *source && edge.target == *target && edge.branch == *branch
            });
            if !joined {
                stored.edges.push(OrderEdge {
                    id: format!("joined-{}", stored.edges.len()),
                    source: source.clone(),
                    target: target.clone(),
                    branch: *branch,
                });
            }
        }
    }
    let kept: HashMap<String, (String, Branch)> = braces(written)
        .into_iter()
        .filter(|(node, _)| *node != id)
        .map(|(node, (parent, branch))| (node.to_owned(), (parent.to_owned(), branch)))
        .collect();
    let mut rest = written.clone();
    rest.nodes.retain(|node| node.id != id);
    rest.edges
        .retain(|edge| edge.source != id && edge.target != id);
    let kept: Braces<'_> = kept
        .iter()
        .map(|(node, (parent, branch))| (node.as_str(), (parent.as_str(), *branch)))
        .collect();
    *written = chain(&rest, &kept);
    true
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

/// Where the last attribute of each statement that has one ends, by handle.
fn attribute_ends(statements: &[Statement], ends: &mut HashMap<String, usize>) {
    for statement in statements {
        if let Some(last) = statement.attributes.iter().map(|a| a.range.end).max() {
            ends.insert(statement.handle.clone(), last);
        }
        if let Form::If {
            then, otherwise, ..
        } = &statement.form
        {
            attribute_ends(then, ends);
            attribute_ends(otherwise.as_deref().unwrap_or_default(), ends);
        }
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
