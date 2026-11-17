//! A board edit written into the author's text (LAY-34). Edit, fix and
//! format are three operations (#411): an edit rewrites or inserts the text
//! of what it changes and leaves every other byte as the author wrote it.
//! Nothing is formatted as a side effect.
//!
//! What a statement owns is the text the window prints for it in the braces
//! it is written in, without the statements inside its own braces. A
//! statement whose own text the edit changes is rewritten where it stands.
//! A new statement, or one the edit takes out of its braces, is written in
//! the braces the window prints it in, directly after the statement before
//! it in the document's order: after the step it follows, its checks and
//! values, when it was added from that step's *Next* (DIAG-07). The parts
//! of the file before its statements are edited one by one in the same way:
//! an `on_error` the author left out stays out until its value changes.
//!
//! Text this cannot place, or that would not read back as the edited
//! document, is printed whole, as before.
use crate::{
    ast::*,
    branches::Branches,
    format::{head, opening, statements},
    lexer::{Token, TokenKind},
    lower::Document,
    parse,
    print::{self, Printer, graphs, print_folder_document, printable, verify},
};
use apiforge_core::{
    format::{Graph, Node, Project},
    model::Branch,
};
use std::{collections::HashMap, ops::Range};

/// `source` edited to say `document`, as a board edit writes it: `previous`
/// is the document `source` says, which names the blocks of its statements.
/// Falls back to [`print_folder_document`] when the edit cannot be written
/// into the text.
pub fn edit_folder_document(
    project: &Project,
    source: &str,
    previous: &Document,
    document: &Document,
) -> Result<String, Vec<Diagnostic>> {
    match edit_in_place(project, source, previous, document) {
        Some(text) => Ok(text),
        None => print_folder_document(project, document),
    }
}

/// As [`edit_folder_document`], without the fallback: the edited text when
/// the edit can be written into `source` and reads back as `document`. A
/// caller that must warn before the whole file is printed again (comments
/// that belong to no statement are dropped by it) asks only on `None`.
pub fn edit_in_place(
    project: &Project,
    source: &str,
    previous: &Document,
    document: &Document,
) -> Option<String> {
    if !printable(project, document, true) {
        return None;
    }
    let tree = parse(source);
    edited(project, &tree, previous, document)
        .filter(|text| verify(project, document, true, text).is_ok())
}

/// Where a statement is written: the main flow or the cleanup, or a branch
/// of an `if`, named by the block's identifier and whether it is `then`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key<'d> {
    Top { cleanup: bool },
    Branch(&'d str, bool),
}

impl<'d> Key<'d> {
    fn branch(x: &'d str, branch: Branch) -> Self {
        Key::Branch(x, branch == Branch::True)
    }

    fn braces(self) -> Option<(&'d str, Branch)> {
        match self {
            Key::Top { .. } => None,
            Key::Branch(x, then) => Some((x, if then { Branch::True } else { Branch::False })),
        }
    }
}

/// One document: its printer and its graphs, read the way the printer reads
/// them.
struct Side<'d> {
    printer: Printer<'d>,
    main: (&'d Graph, Branches<'d>),
    cleanup: Option<(&'d Graph, Branches<'d>)>,
    /// Every block by identifier, with whether it is in the cleanup.
    nodes: HashMap<&'d str, (&'d Node, bool)>,
}

impl<'d> Side<'d> {
    fn new(project: &'d Project, document: &'d Document) -> Self {
        let (main, cleanup) = graphs(document);
        let mut nodes = HashMap::new();
        for node in &main.nodes {
            nodes.insert(node.id.as_str(), (node, false));
        }
        for node in cleanup.into_iter().flat_map(|graph| &graph.nodes) {
            nodes.insert(node.id.as_str(), (node, true));
        }
        Self {
            printer: Printer::new(project, document),
            main: (main, Branches::new(main)),
            cleanup: cleanup.map(|graph| (graph, Branches::new(graph))),
            nodes,
        }
    }

    fn graph(&self, cleanup: bool) -> Option<&(&'d Graph, Branches<'d>)> {
        if cleanup {
            self.cleanup.as_ref()
        } else {
            Some(&self.main)
        }
    }

    /// `node` written in `key`'s braces at depth 0, with its note and the
    /// statements of its own braces that the printer does not skip.
    fn written(&self, node: &Node, cleanup: bool, key: Key<'_>) -> Option<Statement> {
        let (graph, branches) = self.graph(cleanup)?;
        Some(self.printer.statement(node, graph, branches, key.braces()))
    }

    /// What `node` owns in `key`'s braces: its text without its note and
    /// without the statements inside its own braces, an `if` up to its `{`.
    fn own(&self, node: &Node, cleanup: bool, key: Key<'_>) -> Option<String> {
        let mut statement = self.written(node, cleanup, key)?;
        statement.note = None;
        let branch = if let Form::If {
            then, otherwise, ..
        } = &mut statement.form
        {
            then.clear();
            *otherwise = None;
            true
        } else {
            false
        };
        let mut out = String::new();
        statements(&mut out, &[statement], 0);
        let mut lines: Vec<&str> = out.lines().collect();
        if branch {
            lines.pop();
        }
        Some(lines.join("\n"))
    }
}

/// The braces of the old text, where statements are inserted.
struct Block {
    /// Where a statement written first in the braces goes, at a line start.
    open: Option<usize>,
    indent: String,
}

/// Braces the old text does not have yet: the `else` of an `if` or the
/// cleanup. Created at `at` when a statement is written in them.
struct Missing {
    at: usize,
    before: String,
    after: String,
    indent: String,
    statements: String,
}

struct Edit<'s, 'd> {
    text: &'s str,
    tokens: &'s [Token],
    old: Side<'d>,
    new: Side<'d>,
    /// Identifiers of the previous document by handle.
    ids: HashMap<&'d str, &'d str>,
    /// Statements left where they are written, by identifier.
    kept: HashMap<&'d str, (&'s Statement, Key<'d>)>,
    blocks: HashMap<Key<'d>, Block>,
    missing: HashMap<Key<'d>, Missing>,
    /// Replacements, as `(range, order, text)`; an insertion has an empty range.
    edits: Vec<(Range<usize>, usize, String)>,
}

fn edited(
    project: &Project,
    tree: &SyntaxTree,
    previous: &Document,
    document: &Document,
) -> Option<String> {
    if tree.has_errors() || std::mem::discriminant(previous) != std::mem::discriminant(document) {
        return None;
    }
    let file = tree.file.as_ref()?;
    let old = Side::new(project, previous);
    let ids = old
        .nodes
        .values()
        .map(|(node, _)| (node.handle.as_str(), node.id.as_str()))
        .collect();
    let mut edit = Edit {
        text: &tree.source,
        tokens: &tree.tokens,
        old,
        new: Side::new(project, document),
        ids,
        kept: HashMap::new(),
        blocks: HashMap::new(),
        missing: HashMap::new(),
        edits: vec![],
    };
    edit.head(file, previous, document)?;
    edit.classify(&file.statements, Key::Top { cleanup: false }, false)?;
    if let Some(cleanup) = &file.cleanup {
        edit.classify(cleanup, Key::Top { cleanup: true }, true)?;
    }
    edit.top_blocks(file)?;
    // New statements are printed with the new statements of their braces
    // only; the kept ones stay where they are written.
    edit.new.printer.skip = edit.kept.keys().copied().collect();
    edit.rewrite()?;
    edit.insert(false)?;
    edit.insert(true)?;
    edit.finish()
}

impl<'s, 'd> Edit<'s, 'd> {
    /// The file's parts before its statements, one by one.
    fn head(&mut self, file: &'s File, previous: &Document, document: &Document) -> Option<()> {
        let (was, is) = (print::head(previous), print::head(document));
        let brace = self.first(file.range.start, "{")?;
        if opening(&was.kind) != opening(&is.kind) {
            self.replace(file.range.start..brace.end, opening(&is.kind));
        }
        let indent = file
            .items
            .iter()
            .map(|(_, range)| range.start)
            .chain(file.statements.first().map(|s| s.extent.start))
            .min()
            .map_or_else(|| "  ".to_owned(), |at| self.indent_at(at));
        let written: HashMap<&Item, &Range<usize>> = file
            .items
            .iter()
            .map(|(item, range)| (item, range))
            .collect();
        let (was, is) = (head(&was), head(&is));
        // Where a part the text lacks goes: after the part before it.
        let mut at = self.line_after(brace.end);
        for (item, text) in &is {
            let before = was.iter().find(|(old, _)| old == item).map(|(_, t)| t);
            match written.get(item) {
                Some(range) => {
                    if before != Some(text) {
                        let indent = self.indent_at(range.start);
                        self.replace((*range).clone(), text.replace('\n', &format!("\n{indent}")));
                    }
                    at = self.line_after(range.end);
                }
                None if before != Some(text) => self.insert_at(at?, indented(text, &indent)),
                None => {}
            }
        }
        for (item, _) in &was {
            if !is.iter().any(|(new, _)| new == item)
                && let Some(range) = written.get(item)
            {
                let range = self.lines((*range).clone());
                self.replace(range, String::new());
            }
        }
        Some(())
    }

    /// Decides which statements stay where they are written, deletes the
    /// others and records the braces of the kept `if`s.
    fn classify(&mut self, statements: &'s [Statement], key: Key<'d>, cleanup: bool) -> Option<()> {
        for statement in statements {
            let id = self.ids.get(statement.handle.as_str()).copied();
            let Some(id) = id.filter(|&id| self.stays(id, key, cleanup)) else {
                // An `if` goes with the statements of its braces; those the
                // document still has are written again as new ones.
                let range = self.lines(statement.extent.clone());
                self.replace(range, String::new());
                continue;
            };
            self.kept.insert(id, (statement, key));
            let Form::If {
                check,
                then,
                otherwise,
            } = &statement.form
            else {
                continue;
            };
            let brace = self.symbol(check.range.end, "{")?;
            let indent = self.indent_at(statement.range.start);
            let then_block = Block {
                open: self.line_after(brace.end),
                indent: self.inner(then, &indent),
            };
            self.blocks
                .insert(Key::branch(id, Branch::True), then_block);
            match otherwise {
                Some(otherwise) => {
                    let from = then.last().map_or(brace.end, |s| s.extent.end);
                    let close = self.symbol(from, "}")?;
                    let keyword = self.symbol(close.end, "else")?;
                    let brace = self.symbol(keyword.end, "{")?;
                    let else_block = Block {
                        open: self.line_after(brace.end),
                        indent: self.inner(otherwise, &indent),
                    };
                    self.blocks
                        .insert(Key::branch(id, Branch::False), else_block);
                }
                None => {
                    let at = statement.range.end.checked_sub(1)?;
                    if self.text.get(at..at + 1) != Some("}") {
                        return None;
                    }
                    self.missing.insert(
                        Key::branch(id, Branch::False),
                        Missing {
                            at,
                            before: "} else {\n".into(),
                            after: indent.clone(),
                            indent: format!("{indent}  "),
                            statements: String::new(),
                        },
                    );
                }
            }
            self.classify(then, Key::branch(id, Branch::True), cleanup)?;
            self.classify(
                otherwise.as_deref().unwrap_or_default(),
                Key::branch(id, Branch::False),
                cleanup,
            )?;
        }
        Some(())
    }

    /// Whether the block `id`, written in `key`'s braces, stays there: the
    /// edited document has it, of the same kind, in the same graph, and
    /// still in those braces.
    fn stays(&self, id: &str, key: Key<'_>, cleanup: bool) -> bool {
        let (Some((old, _)), Some((new, in_cleanup))) =
            (self.old.nodes.get(id), self.new.nodes.get(id))
        else {
            return false;
        };
        old.kind == new.kind
            && *in_cleanup == cleanup
            && key.braces().is_none_or(|(x, branch)| {
                self.new.graph(cleanup).is_some_and(|(graph, _)| {
                    graph.edges.iter().any(|edge| {
                        edge.source == x && edge.target == id && edge.branch == Some(branch)
                    })
                })
            })
    }

    /// The braces of the main flow and of the cleanup.
    fn top_blocks(&mut self, file: &'s File) -> Option<()> {
        let brace = self.first(file.range.start, "{")?;
        let first = file.statements.first().map(|s| s.extent.start);
        let head_end = file
            .items
            .iter()
            .filter(|(item, range)| {
                *item != Item::Cleanup && first.is_none_or(|first| range.end <= first)
            })
            .map(|(_, range)| range.end)
            .max();
        let indent = file
            .items
            .iter()
            .map(|(_, range)| range.start)
            .chain(first)
            .min()
            .map_or_else(|| "  ".to_owned(), |at| self.indent_at(at));
        self.blocks.insert(
            Key::Top { cleanup: false },
            Block {
                open: self.line_after(head_end.unwrap_or(brace.end)),
                indent: first.map_or_else(|| indent.clone(), |at| self.indent_at(at)),
            },
        );
        let cleanup = file
            .items
            .iter()
            .find(|(item, _)| *item == Item::Cleanup)
            .map(|(_, range)| range.clone());
        match (cleanup, &file.cleanup) {
            (Some(range), Some(statements)) => {
                let brace = self.first(range.start, "{")?;
                let outer = self.indent_at(range.start);
                self.blocks.insert(
                    Key::Top { cleanup: true },
                    Block {
                        open: self.line_after(brace.end),
                        indent: statements.first().map_or_else(
                            || format!("{outer}  "),
                            |s| self.indent_at(s.extent.start),
                        ),
                    },
                );
            }
            _ => {
                let close = file.range.end.checked_sub(1)?;
                if self.text.get(close..close + 1) != Some("}") {
                    return None;
                }
                let start = self.line_start(close);
                let at = if self.text[start..close].trim().is_empty() {
                    start
                } else {
                    close
                };
                self.missing.insert(
                    Key::Top { cleanup: true },
                    Missing {
                        at,
                        before: format!("{indent}cleanup {{\n"),
                        after: format!("{indent}}}\n"),
                        indent: format!("{indent}  "),
                        statements: String::new(),
                    },
                );
            }
        }
        Some(())
    }

    /// Rewrites what a kept statement owns when the edit changes it.
    fn rewrite(&mut self) -> Option<()> {
        let mut kept: Vec<_> = self
            .kept
            .iter()
            .map(|(&id, &(s, key))| (id, s, key))
            .collect();
        kept.sort_by_key(|(_, statement, _)| statement.range.start);
        for (id, statement, key) in kept {
            let (old, cleanup) = self.old.nodes[id];
            let (new, _) = self.new.nodes[id];
            let was = self.old.own(old, cleanup, key)?;
            let is = self.new.own(new, cleanup, key)?;
            let indent = self.indent_at(statement.range.start);
            if was != is {
                let end = match &statement.form {
                    Form::If { check, .. } => self.symbol(check.range.end, "{")?.end,
                    _ => statement.range.end,
                };
                self.replace(
                    statement.range.start..end,
                    is.replace('\n', &format!("\n{indent}")),
                );
            }
            if old.note != new.note {
                let note = new.note.as_ref().map_or_else(String::new, |note| {
                    note.split('\n')
                        .map(|line| format!("// {line}\n{indent}"))
                        .collect()
                });
                self.replace(statement.extent.start..statement.range.start, note);
                if statement.extent.end > statement.range.end {
                    self.replace(statement.range.end..statement.extent.end, String::new());
                }
            }
        }
        Some(())
    }

    /// Writes the new statements of one graph, each after the statement
    /// before it in the document's order that is written in the same braces.
    fn insert(&mut self, cleanup: bool) -> Option<()> {
        enum Step<'d> {
            Kept(Key<'d>, Option<usize>),
            New(Key<'d>, String),
        }
        let mut steps = vec![];
        if let Some((graph, branches)) = self.new.graph(cleanup) {
            for node in &graph.nodes {
                if let Some(&(statement, key)) = self.kept.get(node.id.as_str()) {
                    steps.push(Step::Kept(key, self.line_after(statement.extent.end)));
                    continue;
                }
                let key = match branches.parent(&node.id) {
                    None => Key::Top { cleanup },
                    Some((x, branch)) => match self.kept.get_key_value(x) {
                        Some((&x, _)) => Key::branch(x, branch),
                        // Written inside the braces of a new `if`.
                        None => continue,
                    },
                };
                let mut text = String::new();
                statements(&mut text, &[self.new.written(node, cleanup, key)?], 0);
                steps.push(Step::New(key, text));
            }
        }
        // Where the next new statement of the braces goes, when the text
        // has a line for it there.
        let mut after: HashMap<Key<'d>, Option<usize>> = HashMap::new();
        for step in steps {
            match step {
                Step::Kept(key, at) => {
                    after.insert(key, at);
                }
                Step::New(key, text) => {
                    if let Some(missing) = self.missing.get_mut(&key) {
                        missing
                            .statements
                            .push_str(&indented(&text, &missing.indent));
                        continue;
                    }
                    let block = self.blocks.get(&key)?;
                    let at = after.get(&key).copied().unwrap_or(block.open)?;
                    let text = indented(&text, &block.indent);
                    self.insert_at(at, text);
                    after.insert(key, Some(at));
                }
            }
        }
        Some(())
    }

    fn finish(mut self) -> Option<String> {
        for missing in std::mem::take(&mut self.missing).into_values() {
            if !missing.statements.is_empty() {
                self.insert_at(
                    missing.at,
                    missing.before + &missing.statements + &missing.after,
                );
            }
        }
        self.edits
            .sort_by_key(|(range, order, _)| (range.start, range.end, *order));
        let mut out = String::with_capacity(self.text.len());
        let mut at = 0;
        for (range, _, text) in &self.edits {
            if range.start < at {
                return None;
            }
            out.push_str(&self.text[at..range.start]);
            out.push_str(text);
            at = range.end;
        }
        out.push_str(&self.text[at..]);
        Some(out)
    }

    fn replace(&mut self, range: Range<usize>, text: String) {
        let order = self.edits.len();
        self.edits.push((range, order, text));
    }

    fn insert_at(&mut self, at: usize, text: String) {
        self.replace(at..at, text);
    }

    /// The first token after `from` that is not whitespace or a comment,
    /// when it is `symbol`.
    fn symbol(&self, from: usize, symbol: &str) -> Option<Range<usize>> {
        let token = self
            .after(from)
            .find(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))?;
        (self.text[token.range.clone()] == *symbol).then(|| token.range.clone())
    }

    /// The first `symbol` token after `from`.
    fn first(&self, from: usize, symbol: &str) -> Option<Range<usize>> {
        self.after(from)
            .find(|token| {
                token.kind == TokenKind::Symbol && self.text[token.range.clone()] == *symbol
            })
            .map(|token| token.range.clone())
    }

    /// The tokens that start at or after `from`.
    fn after(&self, from: usize) -> impl Iterator<Item = &'s Token> {
        let start = self
            .tokens
            .partition_point(|token| token.range.start < from);
        self.tokens[start..].iter()
    }

    fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |index| index + 1)
    }

    /// The start of the next line when nothing but blanks follows `at` on
    /// its line.
    fn line_after(&self, at: usize) -> Option<usize> {
        let rest = &self.text[at..];
        let end = rest
            .find('\n')
            .map_or(self.text.len(), |index| at + index + 1);
        self.text[at..end].trim().is_empty().then_some(end)
    }

    /// The indent of the statements of braces opened on a line indented by
    /// `outer`.
    fn inner(&self, statements: &[Statement], outer: &str) -> String {
        statements.first().map_or_else(
            || format!("{outer}  "),
            |statement| self.indent_at(statement.extent.start),
        )
    }

    /// The blanks a line starts with.
    fn indent_at(&self, at: usize) -> String {
        let start = self.line_start(at);
        self.text[start..]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    }

    /// `range` widened to whole lines when it is alone on them.
    fn lines(&self, range: Range<usize>) -> Range<usize> {
        let start = self.line_start(range.start);
        match self.line_after(range.end) {
            Some(end) if self.text[start..range.start].trim().is_empty() => start..end,
            _ => range,
        }
    }
}

/// `text`, lines printed at depth 0, indented by `indent`.
fn indented(text: &str, indent: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                "\n".to_owned()
            } else {
                format!("{indent}{line}\n")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LowerContext, lower};
    use apiforge_core::{
        block::Block,
        edit::{OutputRef, add_node},
        format::{Binding, Position, Scenario},
        graph::{FALSE_PORT, delete_nodes},
        model::ErrorPolicy,
    };
    use std::collections::HashSet;

    /// Not in the window's form: a request block on one line, a check
    /// without spaces and with a comment after it, and `done` written outside
    /// the braces whose branch alone orders it.
    const SOURCE: &str = r#"scenario "Edit" {
  description "Edited on the board."
  param count = 1
  begin = start
  // Reads the list.
  @after(begin)
  list = GET "/items" { sample 200 { items: [] } }
  seen = assert list.status==200 // a quick check
  found = if list.status == 200 {
    item = GET "/items/1"
  }
  @after(found.then)
  done = POST "/done"
}
"#;

    const AT: Position = Position { x: 0.0, y: 0.0 };

    /// `SOURCE` after `change`, as a board edit writes it, checked to read
    /// back as the changed document.
    fn edit(change: impl FnOnce(&mut Scenario)) -> String {
        let project = apiforge_core::demos::shop_project();
        let tree = parse(SOURCE);
        let lowered = lower(&tree, &LowerContext::folder(&project));
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
        let previous = lowered.document.unwrap();
        let mut next = previous.clone();
        let Document::Scenario(scenario) = &mut next else {
            panic!("a scenario")
        };
        change(scenario);
        let text = edited(&project, &tree, &previous, &next).expect("written into the text");
        verify(&project, &next, true, &text).unwrap_or_else(|errors| panic!("{text}\n{errors:?}"));
        text
    }

    fn id(scenario: &Scenario, handle: &str) -> String {
        scenario
            .graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap()
            .id
            .clone()
    }

    fn node<'a>(scenario: &'a mut Scenario, handle: &str) -> &'a mut Node {
        scenario
            .graph
            .nodes
            .iter_mut()
            .find(|node| node.handle == handle)
            .unwrap()
    }

    #[test]
    fn an_edit_without_a_change_writes_the_text_as_it_is() {
        assert_eq!(edit(|_| {}), SOURCE);
    }

    #[test]
    fn a_rename_rewrites_the_statements_that_name_the_block() {
        let text = edit(|scenario| node(scenario, "list").handle = "items".into());
        assert_eq!(
            text,
            SOURCE
                .replace(
                    "  list = GET \"/items\" { sample 200 { items: [] } }",
                    "  items = GET \"/items\" {\n    sample 200 { items: [] }\n  }"
                )
                .replace("list.status==200", "items.status == 200")
                .replace("if list.status", "if items.status")
        );
    }

    #[test]
    fn a_changed_statement_stays_outside_the_braces_it_was_written_outside() {
        let text = edit(|scenario| {
            node(scenario, "done").inputs.insert(
                "url".into(),
                Binding::Literal {
                    value: "/finished".into(),
                },
            );
        });
        assert_eq!(text, SOURCE.replace("\"/done\"", "\"/finished\""));
    }

    #[test]
    fn a_deleted_statement_takes_its_lines_and_nothing_else() {
        let text = edit(|scenario| {
            let item = id(scenario, "item");
            scenario.graph = delete_nodes(&scenario.graph, &HashSet::from([item.as_str()]));
        });
        assert_eq!(text, SOURCE.replace("    item = GET \"/items/1\"\n", ""));
    }

    #[test]
    fn a_deleted_if_writes_the_statements_of_its_braces_where_it_stood() {
        let text = edit(|scenario| {
            let found = id(scenario, "found");
            scenario.graph = delete_nodes(&scenario.graph, &HashSet::from([found.as_str()]));
        });
        assert_eq!(
            text,
            SOURCE
                .replace(
                    "  found = if list.status == 200 {\n    item = GET \"/items/1\"\n  }\n",
                    "  item = GET \"/items/1\"\n"
                )
                .replace("  @after(found.then)\n", "")
        );
    }

    #[test]
    fn a_step_added_on_a_branch_without_braces_opens_them() {
        let text = edit(|scenario| {
            let found = id(scenario, "found");
            let added = add_node(
                &scenario.graph,
                Block::Request,
                AT,
                None,
                Some(OutputRef {
                    node_id: &found,
                    port: FALSE_PORT,
                }),
            )
            .unwrap();
            scenario.graph = added.graph;
        });
        assert_eq!(
            text,
            SOURCE.replace(
                "    item = GET \"/items/1\"\n  }\n",
                "    item = GET \"/items/1\"\n  } else {\n    request = GET \"/endpoint\" {\n      timeout 10s\n    }\n  }\n"
            )
        );
    }

    #[test]
    fn a_step_added_to_a_cleanup_the_text_lacks_writes_the_cleanup() {
        let text = edit(|scenario| {
            scenario.cleanup = add_node(&scenario.cleanup, Block::Request, AT, None, None)
                .unwrap()
                .graph;
        });
        assert_eq!(
            text,
            SOURCE.replace(
                "  done = POST \"/done\"\n}\n",
                "  done = POST \"/done\"\n  cleanup {\n    request = GET \"/endpoint\" {\n      timeout 10s\n    }\n  }\n}\n"
            )
        );
    }

    #[test]
    fn a_changed_note_rewrites_its_comments_only() {
        let text = edit(|scenario| {
            node(scenario, "list").note = Some("Reads every item.".into());
            node(scenario, "seen").note = None;
        });
        assert_eq!(
            text,
            SOURCE
                .replace("// Reads the list.", "// Reads every item.")
                .replace(" // a quick check", "")
        );
    }

    #[test]
    fn the_parts_before_the_statements_are_edited_one_by_one() {
        let text = edit(|scenario| {
            scenario.description = "Changed.".into();
            scenario.error_policy = ErrorPolicy::Continue;
            scenario.parameters.insert("limit".into(), 5.into());
        });
        assert_eq!(
            text,
            SOURCE.replace(
                "  description \"Edited on the board.\"\n  param count = 1\n",
                "  description \"Changed.\"\n  on_error continue\n  param count = 1\n  param limit = 5\n"
            )
        );
    }
}
