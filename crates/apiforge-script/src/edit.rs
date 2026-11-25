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
//! values, when it was added from that step's *Next* (DIAG-07). A statement
//! whose place among the statements of its braces changed — a step moved on
//! the board and what went with it (DIAG-06a) — is cut and written again in
//! its new place as the author wrote it, line for line. The parts
//! of the file before its statements are edited one by one in the same way:
//! an `on_error` the author left out stays out until its value changes.
//!
//! Text this cannot place, or that would not read back as the edited
//! document, is printed whole, as before.
use crate::{
    ast::*,
    format::{attribute_rank, head, quote, statements},
    lexer::{Token, TokenKind},
    lower::Document,
    parse,
    print::{self, Printer, graphs, print_folder_document, printable, verify},
};
use apiforge_core::{
    branches::Branches,
    format::{Graph, Node, Project},
    model::Branch,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

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
    edited(project, &tree, previous, document, &HashSet::new())
        .filter(|text| verify(project, document, true, true, text).is_ok())
}

/// As [`edit_in_place`], for a move on the board (DIAG-06a): `moved` names
/// the blocks whose place in the document's order changed — a step put at
/// a place and the statements that went with it. Their lines are cut and
/// written in the new place as the author wrote them; every other statement
/// stays where it is written. `None` when the move cannot be written so: a
/// move is never printed whole.
pub fn edit_in_place_moving(
    project: &Project,
    source: &str,
    previous: &Document,
    document: &Document,
    moved: &[String],
) -> Option<String> {
    if !printable(project, document, true) {
        return None;
    }
    let tree = parse(source);
    let moved: HashSet<&str> = moved.iter().map(String::as_str).collect();
    edited(project, &tree, previous, document, &moved)
        .filter(|text| verify(project, document, true, true, text).is_ok())
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
            printer: Printer::new(project, document, true),
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
    /// statements of its own braces that the printer does not skip. A note
    /// that `@note` carries is written there only, not also as comments.
    fn written(&self, node: &Node, cleanup: bool, key: Key<'_>) -> Option<Statement> {
        let (graph, branches) = self.graph(cleanup)?;
        let mut statement = self.printer.statement(node, graph, branches, key.braces());
        if statement.attributes.iter().any(|a| a.name == "note") {
            statement.note = None;
        }
        Some(statement)
    }

    /// What `node` owns in `key`'s braces, spelled as `author` writes it:
    /// its text without its note and without the statements inside its own
    /// braces, an `if` up to its `{`.
    fn own(&self, node: &Node, cleanup: bool, key: Key<'_>, author: &Statement) -> Option<String> {
        let mut statement = self.written(node, cleanup, key)?;
        spelled(&mut statement, author);
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
    /// The text that goes when the edit leaves the braces empty: an `else`
    /// or a cleanup that held statements, as a block the edit opened does.
    close: Option<Range<usize>>,
    /// Whether the edit writes a new statement in the braces.
    filled: bool,
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
    /// Statements whose place changed, by identifier: among the statements
    /// of their braces, or into other braces (DIAG-06d).
    moved: HashMap<&'d str, Moved<'s, 'd>>,
    /// The blocks a move moved, by identifier.
    hint: HashSet<String>,
    blocks: HashMap<Key<'d>, Block>,
    missing: HashMap<Key<'d>, Missing>,
    /// Replacements, as `(range, order, text)`; an insertion has an empty range.
    edits: Vec<(Range<usize>, usize, String)>,
}

/// A statement whose place changed: its lines as the author wrote them,
/// written again in its new braces.
struct Moved<'s, 'd> {
    statement: &'s Statement,
    /// The braces it was written in and the braces it is written in now.
    from: Key<'d>,
    to: Key<'d>,
    lines: String,
    /// The indentation of its first line, which the braces it enters
    /// replace.
    indent: String,
}

/// `hint` names the blocks a move moved: where the order of the statements
/// of some braces changed, they are the ones whose lines move.
fn edited(
    project: &Project,
    tree: &SyntaxTree,
    previous: &Document,
    document: &Document,
    hint: &HashSet<&str>,
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
        moved: HashMap::new(),
        hint: hint.iter().map(|id| (*id).to_owned()).collect(),
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
    edit.reorder(hint)?;
    // New statements are printed with the new statements of their braces
    // only; the kept ones stay where they are written and the moved ones
    // are written as they were.
    edit.new.printer.skip = edit.kept.keys().chain(edit.moved.keys()).copied().collect();
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
        self.first_line(file, &was.kind, &is.kind)?;
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

    /// The words of the first line the edit changes, a component's name in
    /// code and the title; what stands between them and the brace is the
    /// author's (LANG-28).
    fn first_line(&mut self, file: &File, was: &FileKind, is: &FileKind) -> Option<()> {
        let words: Vec<Range<usize>> = self
            .after(file.range.start)
            .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment))
            .take(3)
            .map(|token| token.range.clone())
            .collect();
        let (was, is, title) = match (was, is) {
            (FileKind::Scenario { name: was }, FileKind::Scenario { name: is }) => (was, is, 1),
            (
                FileKind::Component {
                    handle: old,
                    name: was,
                },
                FileKind::Component { handle, name: is },
            ) => {
                if old != handle {
                    self.replace(words.get(1)?.clone(), handle.clone());
                }
                (was, is, 2)
            }
            _ => return None,
        };
        if was != is {
            self.replace(words.get(title)?.clone(), quote(is));
        }
        Some(())
    }

    /// Decides which statements stay where they are written, deletes the
    /// others and records the braces of the kept `if`s.
    fn classify(&mut self, statements: &'s [Statement], key: Key<'d>, cleanup: bool) -> Option<()> {
        for statement in statements {
            let id = self.ids.get(statement.handle.as_str()).copied();
            match id {
                Some(id) if self.stays(id, key, cleanup) => {
                    self.kept.insert(id, (statement, key));
                }
                // Moved into other braces (DIAG-06d): its lines, and those
                // of its own braces, are written there as they are.
                Some(id) if self.hint.contains(id) && self.moves_across(id, cleanup) => {
                    let to = self.new_key(id, cleanup)?;
                    self.cut(id, statement, key, to)?;
                }
                _ => {
                    // An `if` goes with the statements of its braces; those
                    // the document still has are written again as new ones.
                    let range = self.lines(statement.extent.clone());
                    self.replace(range, String::new());
                    continue;
                }
            }
            let id = id?;
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
                close: None,
                filled: false,
            };
            self.blocks
                .insert(Key::branch(id, Branch::True), then_block);
            match otherwise {
                Some(otherwise) => {
                    let from = then.last().map_or(brace.end, |s| s.extent.end);
                    let close = self.symbol(from, "}")?;
                    let keyword = self.symbol(close.end, "else")?;
                    let brace = self.symbol(keyword.end, "{")?;
                    // From the `}` of `then` to the `}` that ends the `if`.
                    let end = statement.range.end.checked_sub(1)?;
                    let else_block = Block {
                        open: self.line_after(brace.end),
                        indent: self.inner(otherwise, &indent),
                        close: (!otherwise.is_empty()).then_some(close.start..end),
                        filled: false,
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

    /// Whether the block `id` is moved into other braces of the same graph
    /// by the edit, as the same kind of block.
    fn moves_across(&self, id: &str, cleanup: bool) -> bool {
        let (Some((old, _)), Some((new, in_cleanup))) =
            (self.old.nodes.get(id), self.new.nodes.get(id))
        else {
            return false;
        };
        old.kind == new.kind && *in_cleanup == cleanup
    }

    /// The braces the edited document writes the block `id` in.
    fn new_key(&self, id: &str, cleanup: bool) -> Option<Key<'d>> {
        let (_, branches) = self.new.graph(cleanup)?;
        Some(match branches.parent(id) {
            None => Key::Top { cleanup },
            Some((x, branch)) => Key::branch(x, branch),
        })
    }

    /// Cuts the lines of a moved statement, which must stand on lines of its
    /// own, and records them to be written in `to`.
    fn cut(
        &mut self,
        id: &'d str,
        statement: &'s Statement,
        from: Key<'d>,
        to: Key<'d>,
    ) -> Option<()> {
        let range = self.lines(statement.extent.clone());
        let whole = range.start == self.line_start(statement.extent.start)
            && (range.end == self.text.len() || self.text[..range.end].ends_with('\n'));
        if !whole {
            return None;
        }
        self.moved.insert(
            id,
            Moved {
                statement,
                from,
                to,
                lines: self.text[range.clone()].to_owned(),
                indent: self.indent_at(statement.extent.start),
            },
        );
        self.replace(range, String::new());
        Some(())
    }

    /// Whether the block `id`, written in `key`'s braces, stays there: the
    /// edited document has it, of the same kind, in the same graph, and
    /// still in those braces, as [`Branches`] reads them: an edge from the
    /// empty branch of an `if` written before a step is not the edge of its
    /// braces (DIAG-06d).
    fn stays(&self, id: &str, key: Key<'_>, cleanup: bool) -> bool {
        let (Some((old, _)), Some((new, in_cleanup))) =
            (self.old.nodes.get(id), self.new.nodes.get(id))
        else {
            return false;
        };
        old.kind == new.kind
            && *in_cleanup == cleanup
            && self
                .new
                .graph(cleanup)
                .is_some_and(|(_, branches)| branches.parent(id) == key.braces())
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
                close: None,
                filled: false,
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
                        close: (!statements.is_empty()).then(|| self.lines(range.clone())),
                        filled: false,
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

    /// Finds the statements whose place among the kept statements of their
    /// braces changed and cuts their lines (DIAG-06a). Of the statements of
    /// some braces, the most that keep their order stay where they are
    /// written; the others move. The statements of `hint` move first, then
    /// the shortest. A statement that does not stand on lines of its own
    /// cannot move so, and the edit is not written.
    fn reorder(&mut self, hint: &HashSet<&str>) -> Option<()> {
        let mut rank: HashMap<&str, usize> = HashMap::new();
        let main = self.new.main.0;
        for (index, node) in main.nodes.iter().enumerate() {
            rank.insert(node.id.as_str(), index);
        }
        if let Some((cleanup, _)) = &self.new.cleanup {
            for (index, node) in cleanup.nodes.iter().enumerate() {
                rank.insert(node.id.as_str(), main.nodes.len() + index);
            }
        }
        let mut by_key: HashMap<Key<'d>, Vec<(&'d str, &'s Statement)>> = HashMap::new();
        for (&id, &(statement, key)) in &self.kept {
            by_key.entry(key).or_default().push((id, statement));
        }
        let mut keys: Vec<Key<'d>> = by_key.keys().copied().collect();
        keys.sort_by_key(|key| by_key[key].iter().map(|(_, s)| s.range.start).min());
        for key in keys {
            let mut old = by_key.remove(&key).unwrap_or_default();
            old.sort_by_key(|(_, statement)| statement.range.start);
            let mut new = old.clone();
            new.sort_by_key(|(id, _)| rank.get(id).copied().unwrap_or(usize::MAX));
            if old
                .iter()
                .map(|(id, _)| id)
                .eq(new.iter().map(|(id, _)| id))
            {
                continue;
            }
            let weight = |id: &str, statement: &Statement| -> u64 {
                if hint.contains(id) {
                    1
                } else {
                    (1 << 32) + (statement.extent.end - statement.extent.start) as u64
                }
            };
            let staying = heaviest_common(&old, &new, weight);
            for (id, statement) in old {
                if staying.contains(id) {
                    continue;
                }
                self.kept.remove(id);
                self.cut(id, statement, key, key)?;
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
            let was = self.old.own(old, cleanup, key, statement)?;
            let is = self.new.own(new, cleanup, key, statement)?;
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
            // A note its author wrote as `@note` is rewritten with the
            // statement, in that attribute.
            let attribute = statement.attributes.iter().any(|a| a.name == "note");
            if old.note != new.note && !attribute {
                let comments = self.new.written(new, cleanup, key)?.note;
                let note = comments.as_ref().map_or_else(String::new, |note| {
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
            /// A moved statement's lines, as the author wrote them, and the
            /// indentation of their first line.
            Moved(Key<'d>, String, String),
        }
        let mut steps = vec![];
        if let Some((graph, branches)) = self.new.graph(cleanup) {
            for node in &graph.nodes {
                if let Some(&(statement, key)) = self.kept.get(node.id.as_str()) {
                    steps.push(Step::Kept(key, self.line_after(statement.extent.end)));
                    continue;
                }
                if let Some(moved) = self.moved.get(node.id.as_str()) {
                    // Its lines move as they are only when the edit changes
                    // nothing else of it.
                    let (old, _) = self.old.nodes.get(node.id.as_str())?;
                    let unchanged = old.note == node.note
                        && self.old.own(old, cleanup, moved.from, moved.statement)?
                            == self.new.own(node, cleanup, moved.to, moved.statement)?;
                    if !unchanged {
                        return None;
                    }
                    steps.push(Step::Moved(
                        moved.to,
                        moved.lines.clone(),
                        moved.indent.clone(),
                    ));
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
                    let block = self.blocks.get_mut(&key)?;
                    block.filled = true;
                    let at = after.get(&key).copied().unwrap_or(block.open)?;
                    let text = indented(&text, &block.indent);
                    self.insert_at(at, text);
                    after.insert(key, Some(at));
                }
                Step::Moved(key, lines, from) => {
                    // Only the leading indentation changes, by the depth of
                    // the braces it enters (DIAG-06d).
                    if let Some(missing) = self.missing.get_mut(&key) {
                        let lines = reindented(&lines, &from, &missing.indent)?;
                        missing.statements.push_str(&lines);
                        continue;
                    }
                    let block = self.blocks.get_mut(&key)?;
                    block.filled = true;
                    let lines = reindented(&lines, &from, &block.indent)?;
                    let at = after.get(&key).copied().unwrap_or(block.open)?;
                    self.insert_at(at, lines);
                    after.insert(key, Some(at));
                }
            }
        }
        Some(())
    }

    fn finish(mut self) -> Option<String> {
        // Braces the edit empties go with their last statement, as an
        // `else` or a cleanup the edit opened would, so deleting a new step
        // gives the text back. What was written inside them goes too.
        let kept: Vec<Key<'d>> = self.kept.values().map(|&(_, key)| key).collect();
        let emptied: Vec<Range<usize>> = self
            .blocks
            .iter()
            .filter(|(key, block)| !block.filled && !kept.contains(key))
            .filter_map(|(_, block)| block.close.clone())
            .collect();
        for range in emptied {
            self.edits.retain(|(edit, _, _)| {
                edit.is_empty() || edit.start < range.start || edit.end > range.end
            });
            self.replace(range, String::new());
        }
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

/// `lines`, written with their first line indented by `from`, indented by
/// `to` instead: every line that is not empty gains or loses the one
/// difference; nothing else of any line changes. `None` when a line does not
/// start with the indentation it would lose, or the two differ otherwise.
fn reindented(lines: &str, from: &str, to: &str) -> Option<String> {
    if from == to {
        return Some(lines.to_owned());
    }
    let empty = |line: &str| line.trim_end_matches(['\n', '\r']).is_empty();
    if let Some(extra) = to.strip_prefix(from) {
        return Some(
            lines
                .split_inclusive('\n')
                .map(|line| {
                    if empty(line) {
                        line.to_owned()
                    } else {
                        format!("{extra}{line}")
                    }
                })
                .collect(),
        );
    }
    let less = from.strip_prefix(to)?;
    lines
        .split_inclusive('\n')
        .map(|line| {
            if empty(line) {
                Some(line.to_owned())
            } else if let Some(rest) = line.strip_prefix(less) {
                Some(rest.to_owned())
            } else if line.trim().is_empty() {
                Some(line.to_owned())
            } else {
                None
            }
        })
        .collect()
}

/// The statements of `old` that keep their order in `new`, the heaviest such
/// set by `weight`: a longest common subsequence of the two orders of the
/// same statements.
fn heaviest_common<'d>(
    old: &[(&'d str, &Statement)],
    new: &[(&'d str, &Statement)],
    weight: impl Fn(&str, &Statement) -> u64,
) -> HashSet<&'d str> {
    let (rows, columns) = (old.len(), new.len());
    let mut table = vec![vec![0u64; columns + 1]; rows + 1];
    for i in (0..rows).rev() {
        for j in (0..columns).rev() {
            table[i][j] = if old[i].0 == new[j].0 {
                weight(old[i].0, old[i].1) + table[i + 1][j + 1]
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut common = HashSet::new();
    while i < rows && j < columns {
        if old[i].0 == new[j].0 {
            common.insert(old[i].0);
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    common
}

/// `statement` as `author` spells it (KTLO-37): a note the author wrote as
/// `@note` stays that attribute, and the attributes keep the author's order;
/// one the author did not write goes before the first whose canonical place
/// is after its own.
fn spelled(statement: &mut Statement, author: &Statement) {
    let written: Vec<&str> = author.attributes.iter().map(|a| a.name.as_str()).collect();
    if written.contains(&"note")
        && !statement.attributes.iter().any(|a| a.name == "note")
        && let Some(note) = statement.note.take()
    {
        statement.attributes.push(Attribute {
            name: "note".into(),
            arguments: vec![Expr {
                kind: ExprKind::Literal(note.into()),
                range: 0..0,
            }],
            range: 0..0,
        });
    }
    let (mut ordered, rest): (Vec<Attribute>, Vec<Attribute>) =
        std::mem::take(&mut statement.attributes)
            .into_iter()
            .partition(|a| written.contains(&a.name.as_str()));
    ordered.sort_by_key(|a| written.iter().position(|name| *name == a.name));
    for attribute in rest {
        let rank = attribute_rank(&attribute.name, false);
        let at = ordered
            .iter()
            .position(|a| attribute_rank(&a.name, false) > rank)
            .unwrap_or(ordered.len());
        ordered.insert(at, attribute);
    }
    statement.attributes = ordered;
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

    /// Not in the window's form: a request block on one line and a check
    /// without spaces and with a comment after it.
    const SOURCE: &str = r#"scenario "Edit" {
  description "Edited on the board."
  param count = 1
  begin = start
  // Reads the list.
  list = GET "/items" { sample 200 { items: [] } }
  seen = assert list.status==200 // a quick check
  found = if list.status == 200 {
    item = GET "/items/1"
  }
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
        // As a board edit through the session: the chain is kept (LANG-13).
        let Document::Scenario(before) = &previous else {
            panic!("a scenario")
        };
        scenario.graph = apiforge_core::chain::edited(&before.graph, &scenario.graph);
        scenario.cleanup = apiforge_core::chain::edited(&before.cleanup, &scenario.cleanup);
        let text = edited(&project, &tree, &previous, &next, &HashSet::new())
            .expect("written into the text");
        verify(&project, &next, true, true, &text)
            .unwrap_or_else(|errors| panic!("{text}\n{errors:?}"));
        text
    }

    /// `SOURCE` after `change`, then that text edited back to the document
    /// `SOURCE` says, as a second board edit (or *Undo*) writes it.
    fn edit_and_back(change: impl FnOnce(&mut Scenario)) -> (String, String) {
        let project = apiforge_core::demos::shop_project();
        let original = lower(&parse(SOURCE), &LowerContext::folder(&project))
            .document
            .unwrap();
        let changed = edit(change);
        let tree = parse(&changed);
        let previous = lower(&tree, &LowerContext::folder(&project))
            .document
            .unwrap();
        let back = edited(&project, &tree, &previous, &original, &HashSet::new())
            .expect("written into the text");
        verify(&project, &original, true, true, &back)
            .unwrap_or_else(|errors| panic!("{back}\n{errors:?}"));
        (changed, back)
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
    fn a_changed_statement_after_an_if_stays_where_it_is_written() {
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
            SOURCE.replace(
                "  found = if list.status == 200 {\n    item = GET \"/items/1\"\n  }\n",
                "  item = GET \"/items/1\"\n"
            )
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
    /// A note that only `@note` can carry exactly is written once, in the
    /// attribute, not also as comments (review of #426).
    #[test]
    fn a_note_written_by_its_attribute_is_not_also_written_as_comments() {
        let text = edit(|scenario| node(scenario, "list").note = Some(" padded".into()));
        assert_eq!(text.matches("padded").count(), 1, "{text}");
        assert!(text.contains("@note(\" padded\")"), "{text}");
        assert!(!text.contains("Reads the list"), "{text}");
    }

    /// An `else` the edit opened is closed again when its last statement is
    /// deleted, so deleting the new step gives the text back (review of #426).
    #[test]
    fn an_else_opened_for_a_step_closes_when_the_step_is_deleted() {
        let (opened, back) = edit_and_back(|scenario| {
            let found = id(scenario, "found");
            scenario.graph = add_node(
                &scenario.graph,
                Block::Request,
                AT,
                None,
                Some(OutputRef {
                    node_id: &found,
                    port: FALSE_PORT,
                }),
            )
            .unwrap()
            .graph;
        });
        assert!(opened.contains("} else {"));
        assert_eq!(back, SOURCE);
    }

    /// The same for a cleanup the edit wrote.
    #[test]
    fn a_cleanup_written_for_a_step_goes_when_the_step_is_deleted() {
        let (opened, back) = edit_and_back(|scenario| {
            scenario.cleanup = add_node(&scenario.cleanup, Block::Request, AT, None, None)
                .unwrap()
                .graph;
        });
        assert!(opened.contains("cleanup {"));
        assert_eq!(back, SOURCE);
    }

    /// LANG-06, criterion 6 (KTLO-37): a board edit that rewrites a
    /// statement whose author wrote `@note` above `@label` keeps that order
    /// and that spelling on it, and changes no other line.
    #[test]
    fn ktlo37_an_edit_keeps_the_authors_order() {
        for note in ["Reads the list.", " padded"] {
            let source = format!(
                "scenario \"Edit\" {{\n  begin = start\n  @note({note:?})\n  @label(\"List\")\n  list = GET \"/items\"\n  done = POST \"/done\"\n}}\n"
            );
            let project = apiforge_core::demos::shop_project();
            let tree = parse(&source);
            let lowered = lower(&tree, &LowerContext::folder(&project));
            assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
            let previous = lowered.document.unwrap();
            let mut next = previous.clone();
            let Document::Scenario(scenario) = &mut next else {
                panic!("a scenario")
            };
            node(scenario, "list").label = Some("Items".into());
            let text = edited(&project, &tree, &previous, &next, &HashSet::new())
                .expect("written into the text");
            verify(&project, &next, true, true, &text)
                .unwrap_or_else(|errors| panic!("{text}\n{errors:?}"));
            assert_eq!(
                text,
                source.replace("@label(\"List\")", "@label(\"Items\")"),
                "{note:?}"
            );
        }
    }

    // LANG-28 ---------------------------------------------------------------

    /// `source` after `change`, as a model edit writes it, checked to read
    /// back as the changed document.
    fn changed(source: &str, change: impl FnOnce(&mut Document)) -> String {
        let project = apiforge_core::demos::shop_project();
        let lowered = lower(&parse(source), &LowerContext::folder(&project));
        assert!(!lowered.has_errors(), "{source}\n{:?}", lowered.diagnostics);
        let previous = lowered.document.unwrap();
        let mut next = previous.clone();
        change(&mut next);
        edit_in_place(&project, source, &previous, &next)
            .unwrap_or_else(|| panic!("not written into the text:\n{source}"))
    }

    fn renamed(document: &mut Document, title: &str) {
        match document {
            Document::Scenario(scenario) => scenario.name = title.into(),
            Document::Component(component) => component.name = title.into(),
        }
    }

    /// LANG-28, criterion 1: a changed title, or a component's changed name
    /// in code, replaces those words of the first line and no other byte.
    #[test]
    fn a_retitled_file_keeps_what_stands_before_its_brace() {
        const BODY: &str = " // open\n  begin = start\n  a = GET \"/a\" // kept\n}\n";
        const EXPORT: &str = " // open\n  export result: number = 1 // kept\n}\n";
        for between in [
            " // why\n",
            " // why { \"A\"\n  // and why\n\n",
            "  \t",
            "\r\n",
            "",
        ] {
            let scenario = format!("// top\nscenario \"A\"{between}{{{BODY}");
            assert_eq!(
                changed(&scenario, |document| renamed(document, "B { c")),
                scenario.replacen("scenario \"A\"", "scenario \"B { c\"", 1)
            );

            // The name in code says `A` too: each is found by its place.
            let component = format!("// top\ncomponent A \"A\"{between}{{{EXPORT}");
            let rehandled = |document: &mut Document| {
                let Document::Component(component) = document else {
                    panic!("a component")
                };
                component.handle = "B".into();
            };
            assert_eq!(
                changed(&component, |document| renamed(document, "C")),
                component.replacen("component A \"A\"", "component A \"C\"", 1)
            );
            assert_eq!(
                changed(&component, rehandled),
                component.replacen("component A \"A\"", "component B \"A\"", 1)
            );
            assert_eq!(
                changed(&component, |document| {
                    rehandled(document);
                    renamed(document, "C");
                }),
                component.replacen("component A \"A\"", "component B \"C\"", 1)
            );
        }

        // Comments between the words, and a title spelled another way.
        let source = format!("component // a \"x\"\n  A // b\n  \"\\u0041\" // c\n{{{EXPORT}");
        assert_eq!(
            changed(&source, |document| renamed(document, "C")),
            source.replacen("\"\\u0041\"", "\"C\"", 1)
        );

        // The language has no `/* */`: such a text has no model to edit.
        assert!(parse("scenario \"A\" /* why */ {\n  begin = start\n}\n").has_errors());
    }

    // DIAG-06a --------------------------------------------------------------

    /// Not in the window's form: notes as comments, attributes, a request on
    /// one line, a check without spaces with a comment after it, and a blank
    /// line that belongs to no statement.
    const MOVE: &str = r#"scenario "Move" {
  begin = start
  // Reads the list.
  @label("List")
  list = GET "/items" { sample 200 { items: [] } }
  seen = assert list.status==200 // a quick check

  other = GET "/other"
  // Posts the end.
  @label("Done")
  done = POST "/done" {
    body { n: 1 }
  }
  done_ok = assert done.status == 200 // the end
}
"#;

    /// `source` with the step `step` put after `anchor` as the board puts
    /// it, written into the text and checked to read back as the moved
    /// document.
    fn moved_text(source: &str, step: &str, anchor: &str) -> String {
        let project = apiforge_core::demos::shop_project();
        let tree = parse(source);
        let lowered = lower(&tree, &LowerContext::folder(&project));
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
        let previous = lowered.document.unwrap();
        let mut next = previous.clone();
        let Document::Scenario(scenario) = &mut next else {
            panic!("a scenario")
        };
        let (step, anchor) = (id(scenario, step), id(scenario, anchor));
        let (graph, moved) = apiforge_core::edit::place(
            &scenario.graph,
            &project,
            &step,
            apiforge_core::edit::Place::After(&anchor),
        )
        .expect("the step moves");
        scenario.graph = graph;
        let text = edit_in_place_moving(&project, source, &previous, &next, &moved)
            .expect("the move is written into the text");
        verify(&project, &next, true, true, &text)
            .unwrap_or_else(|errors| panic!("{text}\n{errors:?}"));
        text
    }

    /// DIAG-06a, criterion 2: the file after a move differs from the file
    /// before only by the moved statements' lines standing in the new place,
    /// with their notes, attributes and comments; nothing is reformatted.
    #[test]
    fn a_moved_statement_takes_only_its_own_lines() {
        let done = "  // Posts the end.\n  @label(\"Done\")\n  done = POST \"/done\" {\n    body { n: 1 }\n  }\n  done_ok = assert done.status == 200 // the end\n";
        let list = "  // Reads the list.\n  @label(\"List\")\n  list = GET \"/items\" { sample 200 { items: [] } }\n  seen = assert list.status==200 // a quick check\n";
        // Forwards: `done` and its check go up, after `list` and `seen`.
        assert_eq!(
            moved_text(MOVE, "done", "list"),
            MOVE.replace(done, "")
                .replace(list, &format!("{list}{done}"))
        );
        // Backwards: `list` and its check go down, after `other`.
        assert_eq!(
            moved_text(MOVE, "list", "other"),
            MOVE.replace(list, "").replace(
                "  other = GET \"/other\"\n",
                &format!("  other = GET \"/other\"\n{list}")
            )
        );
    }

    /// DIAG-06a, criterion 3: the checks and values written under a step
    /// that read it and no other step go with it, in their order; one that
    /// also reads another step stays where it is written.
    #[test]
    fn a_step_moves_with_its_checks() {
        const CHECKS: &str = r#"scenario "Checks" {
  begin = start
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  b = GET "/b" {
    sample 200 { n: 2 }
  }
  b_ok = assert b.status == 200
  b_both = assert b.body.n == a.body.n
  b_n = b.body.n
  c = GET "/c" {
    sample 200 {}
  }
}
"#;
        assert_eq!(
            moved_text(CHECKS, "b", "begin"),
            r#"scenario "Checks" {
  begin = start
  b = GET "/b" {
    sample 200 { n: 2 }
  }
  b_ok = assert b.status == 200
  b_n = b.body.n
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  b_both = assert b.body.n == a.body.n
  c = GET "/c" {
    sample 200 {}
  }
}
"#
        );
    }

    // DIAG-06d --------------------------------------------------------------

    /// `source` with `step` put at the place `at` names, as the board puts
    /// it, written into the text and checked to read back as the moved
    /// document.
    fn placed_text(
        source: &str,
        step: &str,
        at: impl FnOnce(&Scenario) -> apiforge_core::edit::Place<'static>,
    ) -> String {
        let project = apiforge_core::demos::shop_project();
        let tree = parse(source);
        let lowered = lower(&tree, &LowerContext::folder(&project));
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
        let previous = lowered.document.unwrap();
        let mut next = previous.clone();
        let Document::Scenario(scenario) = &mut next else {
            panic!("a scenario")
        };
        let place = at(scenario);
        let step = id(scenario, step);
        let (graph, moved) = apiforge_core::edit::place(&scenario.graph, &project, &step, place)
            .expect("the statement moves");
        scenario.graph = graph;
        let text = edit_in_place_moving(&project, source, &previous, &next, &moved)
            .expect("the move is written into the text");
        verify(&project, &next, true, true, &text)
            .unwrap_or_else(|errors| panic!("{text}\n{errors:?}"));
        text
    }

    fn leaked(id: String) -> &'static str {
        Box::leak(id.into_boxed_str())
    }

    fn after_handle(
        handle: &'static str,
    ) -> impl FnOnce(&Scenario) -> apiforge_core::edit::Place<'static> {
        move |scenario| apiforge_core::edit::Place::After(leaked(id(scenario, handle)))
    }

    fn first_in(
        handle: &'static str,
        branch: Branch,
    ) -> impl FnOnce(&Scenario) -> apiforge_core::edit::Place<'static> {
        move |scenario| apiforge_core::edit::Place::First(leaked(id(scenario, handle)), branch)
    }

    /// `lines` with `by` before every line that is not empty.
    fn deeper(lines: &str, by: &str) -> String {
        lines
            .split_inclusive('\n')
            .map(|line| {
                if line.trim_end_matches('\n').is_empty() {
                    line.to_owned()
                } else {
                    format!("{by}{line}")
                }
            })
            .collect()
    }

    /// `lines` with `by` taken from the start of every line that is not
    /// empty.
    fn shallower(lines: &str, by: &str) -> String {
        lines
            .split_inclusive('\n')
            .map(|line| line.strip_prefix(by).unwrap_or(line).to_owned())
            .collect()
    }

    const NESTED: &str = r#"scenario "Nested" {
  begin = start
  a = GET "/a"
  // Decides.
  x = if a.status == 200 {
    inner = GET "/inner"

    y = if inner.status == 200 {
      // The deepest.
      deep = GET "/deep"
    }
  }
  b = GET "/b"
}
"#;

    const Y: &str = "    y = if inner.status == 200 {\n      // The deepest.\n      deep = GET \"/deep\"\n    }\n";

    /// DIAG-06d, criterion 1: an `if` with a nested `if` moves as its lines
    /// are written; out of its braces, its lines are one level shallower.
    #[test]
    fn diag06d_a_nested_if_moves_with_its_lines() {
        let x = "  // Decides.\n  x = if a.status == 200 {\n    inner = GET \"/inner\"\n\n    y = if inner.status == 200 {\n      // The deepest.\n      deep = GET \"/deep\"\n    }\n  }\n";
        assert_eq!(
            placed_text(NESTED, "x", after_handle("b")),
            NESTED
                .replace(x, "")
                .replace("  b = GET \"/b\"\n", &format!("  b = GET \"/b\"\n{x}"))
        );
        assert_eq!(
            placed_text(NESTED, "y", after_handle("b")),
            NESTED.replace(Y, "").replace(
                "  b = GET \"/b\"\n",
                &format!("  b = GET \"/b\"\n{}", shallower(Y, "  "))
            )
        );
    }

    /// DIAG-06d, criterion 1: a free comment and a blank line inside the
    /// braces of a moved `if` travel as they are, one level deeper.
    #[test]
    fn diag06d_free_comments_inside_moved_braces_travel() {
        let source = r#"scenario "Free" {
  begin = start
  a = GET "/a"
  x = if a.status == 200 {
    inner = GET "/inner"

    // Nothing else is checked here yet.
  }
  z = if 1 == 1 {
    last = GET "/last"
  }
}
"#;
        let x = "  x = if a.status == 200 {\n    inner = GET \"/inner\"\n\n    // Nothing else is checked here yet.\n  }\n";
        assert_eq!(
            placed_text(source, "x", after_handle("last")),
            source.replace(x, "").replace(
                "    last = GET \"/last\"\n",
                &format!("    last = GET \"/last\"\n{}", deeper(x, "  "))
            )
        );
    }

    /// DIAG-06d, criterion 1: the last statement moved out of an `else`;
    /// the `else` then holds none and is written as the writer writes an
    /// `if` without one.
    #[test]
    fn diag06d_the_last_statement_out_of_an_else() {
        let source = r#"scenario "Else" {
  begin = start
  a = GET "/a"
  x = if a.status == 200 {
    inner = GET "/inner"
  } else {
    // The other way.
    other = GET "/other"
  }
  b = GET "/b"
}
"#;
        assert_eq!(
            placed_text(source, "other", after_handle("b")),
            r#"scenario "Else" {
  begin = start
  a = GET "/a"
  x = if a.status == 200 {
    inner = GET "/inner"
  }
  b = GET "/b"
  // The other way.
  other = GET "/other"
}
"#
        );
    }

    /// DIAG-06d, criterion 1: a request with a body over several lines,
    /// moved into a branch: every line one level deeper, the body the same.
    #[test]
    fn diag06d_a_multi_line_body_moves_one_level_deeper() {
        let source = r#"scenario "Body" {
  begin = start
  a = GET "/a"
  x = if a.status == 200 {
    inner = GET "/inner"
  }
  b = POST "/b" {
    body {
      name: "x",
      tags: [
        "one",
        "two"
      ]
    }
  }
}
"#;
        let b = "  b = POST \"/b\" {\n    body {\n      name: \"x\",\n      tags: [\n        \"one\",\n        \"two\"\n      ]\n    }\n  }\n";
        assert_eq!(
            placed_text(source, "b", after_handle("inner")),
            source.replace(b, "").replace(
                "    inner = GET \"/inner\"\n",
                &format!("    inner = GET \"/inner\"\n{}", deeper(b, "  "))
            )
        );
    }

    /// DIAG-06d, criterion 1: a triple-quoted string moved two levels deeper
    /// keeps its value: it loses its common indentation.
    #[test]
    fn diag06d_a_triple_quoted_string_moves_two_levels_deeper() {
        let source = "scenario \"Text\" {\n  begin = start\n  a = GET \"/a\"\n  x = if a.status == 200 {\n    y = if 1 == 1 {\n      deep = GET \"/deep\"\n    }\n  }\n  note = POST \"/note\" {\n    body {\n      text: \"\"\"\n        First line\n          indented\n\n        after a blank line\n        \"\"\"\n    }\n  }\n}\n";
        let note = "  note = POST \"/note\" {\n    body {\n      text: \"\"\"\n        First line\n          indented\n\n        after a blank line\n        \"\"\"\n    }\n  }\n";
        assert_eq!(
            placed_text(source, "note", first_in("y", Branch::True)),
            source.replace(note, "").replace(
                "    y = if 1 == 1 {\n",
                &format!("    y = if 1 == 1 {{\n{}", deeper(note, "    "))
            )
        );
    }
}
