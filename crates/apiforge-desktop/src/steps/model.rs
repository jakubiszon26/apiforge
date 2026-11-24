//! The rows of *Steps*: what the list shows of a scenario, read from its
//! graphs alone (REQ-01a). Built once per model, never per frame.

use super::sends::{self, Sends};
use crate::canvas::check_text;
use apiforge_core::{
    chain,
    format::{Binding, Graph, Node, Position, Project, TemplatePart},
    graph::{Subject, check_subjects_in, create_node, sequence},
    model::{Branch, NodeKind},
    run::Phase,
    template_text::{Scope, print_reference, print_template},
};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashMap;

/// Every row of a scenario, in the order of the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub main: Vec<Row>,
    pub cleanup: Vec<Row>,
}

/// One row of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The node id of the step.
    pub id: String,
    pub handle: String,
    /// The step's label, or its handle.
    pub name: String,
    /// The step's place in its graph's order, the number of its card on the
    /// board.
    pub number: usize,
    pub phase: Phase,
    /// How many branches the step stands in.
    pub depth: usize,
    /// *then* or *else*, the branch the step stands in.
    pub branch: Option<&'static str>,
    /// The row is the first of its branch.
    pub opens: bool,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Request(Box<Request>),
    /// A component call: its arguments, and how many steps read it.
    Call {
        arguments: String,
        readers: usize,
    },
    /// `value`, `if`, `condition` or `export`, and what follows it.
    Line {
        keyword: &'static str,
        text: String,
    },
    /// A check under no request.
    Check {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub method: &'static str,
    /// The URL without its scheme and host.
    pub path: String,
    pub reads: Vec<String>,
    pub column: Column,
    pub checks: Vec<Check>,
}

/// The middle column of a request.
#[derive(Clone, Debug, PartialEq)]
pub enum Column {
    Sends(Sends),
    /// The fields later steps read, one line per field and reader.
    Sets(Vec<String>),
}

/// A check listed under a request.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub id: String,
    pub handle: String,
    pub text: String,
}

impl Row {
    /// Whether the row is the step `id`, or lists it as a check.
    pub fn holds(&self, id: &str) -> bool {
        self.id == id || self.checks().iter().any(|check| check.id == id)
    }

    pub fn checks(&self) -> &[Check] {
        match &self.kind {
            Kind::Request(request) => &request.checks,
            _ => &[],
        }
    }

    #[cfg(test)]
    pub fn sends(&self) -> Option<&Sends> {
        match &self.kind {
            Kind::Request(request) => match &request.column {
                Column::Sends(sends) => Some(sends),
                Column::Sets(_) => None,
            },
            _ => None,
        }
    }

    /// What the row says after its number.
    pub fn title(&self) -> String {
        match &self.kind {
            Kind::Request(request) => format!("{} {}", request.method, self.name),
            Kind::Call { .. } => format!("call {}", self.name),
            Kind::Line { keyword, text } => format!("{keyword} {text}"),
            Kind::Check { text } => text.clone(),
        }
    }
}

impl Model {
    /// The rows of `main` and of `cleanup`, which may read Main's steps.
    pub fn of(main: &Graph, cleanup: Option<&Graph>, project: &Project) -> Self {
        // A Cleanup step names the Main step it reads.
        let mut both = main.clone();
        if let Some(cleanup) = cleanup {
            both.nodes.extend(cleanup.nodes.iter().cloned());
        }
        let mut graphs = vec![main];
        graphs.extend(cleanup);
        let texts = Texts {
            scope: Scope {
                nodes: &both.nodes,
                project,
                request: None,
            },
            graph: &both,
        };
        let readers = Readers::of(&graphs, &texts);
        let section = |graph, phase| section(graph, phase, &graphs, &texts, &readers);
        Self {
            main: section(main, Phase::Main),
            cleanup: cleanup.map_or_else(Vec::new, |cleanup| section(cleanup, Phase::Cleanup)),
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.main.iter().chain(&self.cleanup)
    }

    /// `16 steps · 33 checks`.
    pub fn summary(&self) -> String {
        let alone = self
            .rows()
            .filter(|row| matches!(row.kind, Kind::Check { .. }))
            .count();
        let steps = self.rows().count() - alone;
        let checks = alone + self.rows().map(|row| row.checks().len()).sum::<usize>();
        let count =
            |count: usize, one: &str| format!("{count} {one}{}", if count == 1 { "" } else { "s" });
        format!("{} · {}", count(steps, "step"), count(checks, "check"))
    }
}

/// The texts of bindings, as they are written.
pub struct Texts<'a> {
    pub scope: Scope<'a>,
    pub graph: &'a Graph,
}

impl Texts<'_> {
    /// A binding as the code writes it, the way a check writes its operand.
    pub fn written(&self, binding: &Binding) -> String {
        let mut probe = create_node(NodeKind::Assert, Position { x: 0.0, y: 0.0 }, None);
        probe.inputs.clear();
        probe.inputs.insert("actual".into(), binding.clone());
        let text = check_text(&probe, self.scope, self.graph, None);
        text.actual
            .and_then(|at| text.parts.into_iter().nth(at))
            .map(|part| part.text)
            .unwrap_or_default()
    }

    /// A URL: a text without its quotes.
    pub fn url(&self, binding: &Binding) -> String {
        print_template(binding, self.scope).unwrap_or_else(|| self.written(binding))
    }

    /// A reference, `None` for one to nothing.
    fn reference(&self, binding: &Binding) -> Option<String> {
        print_reference(binding, self.scope)
    }
}

/// The fields later steps read, by the step read: `body.id → cancelled`.
struct Readers<'a> {
    lines: HashMap<&'a str, IndexSet<String>>,
    steps: HashMap<&'a str, IndexSet<&'a str>>,
}

impl<'a> Readers<'a> {
    /// What `graph::relations` lists as a step's readers, for every step at
    /// once and over both graphs: a check that verifies the step is no reader
    /// of it. In the order the steps are reached.
    fn of(graphs: &[&'a Graph], texts: &Texts<'_>) -> Self {
        let mut readers = Self {
            lines: HashMap::new(),
            steps: HashMap::new(),
        };
        let handles: HashMap<&str, &str> = graphs
            .iter()
            .flat_map(|graph| &graph.nodes)
            .map(|node| (node.id.as_str(), node.handle.as_str()))
            .collect();
        for graph in graphs {
            let nodes: HashMap<&str, &'a Node> = graph
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), node))
                .collect();
            for node in sequence(graph).into_iter().filter_map(|id| nodes.get(id)) {
                let verified: Vec<Subject<'a>> = check_subjects_in(graphs, node).subjects;
                for reference in node.inputs.values().flat_map(Binding::references) {
                    let source = reference.node_id;
                    let Some(handle) = handles.get(source) else {
                        continue;
                    };
                    if verified.iter().any(|subject| subject.step == source) {
                        continue;
                    }
                    let field = texts
                        .reference(&Binding::Reference {
                            node_id: source.to_owned(),
                            port: reference.port.to_owned(),
                            path: reference.path.to_owned(),
                        })
                        .unwrap_or_else(|| reference.port.to_owned());
                    let field = field
                        .strip_prefix(handle)
                        .map(|rest| rest.strip_prefix('.').unwrap_or(rest))
                        .filter(|rest| !rest.is_empty())
                        .unwrap_or(&field);
                    readers
                        .lines
                        .entry(source)
                        .or_default()
                        .insert(format!("{field} → {}", node.handle));
                    readers.steps.entry(source).or_default().insert(&node.id);
                }
            }
        }
        readers
    }
}

fn is_check(kind: NodeKind) -> bool {
    kind == NodeKind::Assert
}

/// The rows of one graph.
fn section(
    graph: &Graph,
    phase: Phase,
    graphs: &[&Graph],
    texts: &Texts<'_>,
    readers: &Readers<'_>,
) -> Vec<Row> {
    let numbers: HashMap<&str, usize> = sequence(graph)
        .into_iter()
        .enumerate()
        .map(|(index, id)| (id, index + 1))
        .collect();
    let braces = chain::braces(graph);
    let mut blocks: IndexMap<Option<(&str, bool)>, Vec<&Node>> = IndexMap::new();
    for node in &graph.nodes {
        let own = braces
            .get(node.id.as_str())
            .map(|(step, branch)| (*step, *branch == Branch::True));
        blocks.entry(own).or_default().push(node);
    }
    let requests: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Request)
        .collect();
    let sends: HashMap<&str, Sends> = requests
        .iter()
        .map(|node| node.id.as_str())
        .zip(sends::sends(&requests, texts))
        .collect();
    let mut section = Section {
        phase,
        graphs,
        texts,
        readers,
        numbers,
        blocks,
        sends,
        rows: Vec::new(),
    };
    section.block(None, 0);
    section.rows
}

struct Section<'a> {
    phase: Phase,
    graphs: &'a [&'a Graph],
    texts: &'a Texts<'a>,
    readers: &'a Readers<'a>,
    numbers: HashMap<&'a str, usize>,
    /// The statements of each braces as they are written; whether the braces
    /// are the *then* ones.
    blocks: IndexMap<Option<(&'a str, bool)>, Vec<&'a Node>>,
    sends: HashMap<&'a str, Sends>,
    rows: Vec<Row>,
}

impl<'a> Section<'a> {
    /// The rows of one braces, those of its `if`s under them. A check stands
    /// where it is written: under the request written before it when it reads
    /// one step, else on a row of its own.
    fn block(&mut self, braces: Option<(&'a str, bool)>, depth: usize) {
        let written = self.blocks.get(&braces).cloned().unwrap_or_default();
        let place = |node: &Node| self.numbers.get(node.id.as_str()).copied().unwrap_or(0);
        // The checks after each step, by the step; `None` before any.
        let mut under: HashMap<Option<&str>, Vec<&'a Node>> = HashMap::new();
        let mut items: Vec<&'a Node> = Vec::new();
        let mut step: Option<&'a Node> = None;
        let mut last: Option<&'a Node> = None;
        for node in written {
            if !is_check(node.kind) {
                if chain::is_step(node.kind) {
                    step = Some(node);
                }
                if last.is_none_or(|last| place(last) < place(node)) {
                    last = Some(node);
                }
                items.push(node);
                continue;
            }
            // Under a request, and else after the last row above it.
            let after = match step {
                Some(step) if step.kind == NodeKind::Request && self.single(node) => Some(step),
                _ => last,
            };
            under
                .entry(after.map(|node| node.id.as_str()))
                .or_default()
                .push(node);
        }
        items.sort_by_key(|node| place(node));
        let first = self.rows.len();
        for check in under.remove(&None).unwrap_or_default() {
            self.check_row(check, depth);
        }
        for node in items {
            let checks = under.remove(&Some(node.id.as_str())).unwrap_or_default();
            let (listed, alone): (Vec<&Node>, Vec<&Node>) = checks
                .into_iter()
                .partition(|check| node.kind == NodeKind::Request && self.single(check));
            if node.kind != NodeKind::Start {
                let kind = self.kind(node, &listed);
                self.row(node, depth, kind);
            }
            if node.kind == NodeKind::If {
                self.block(Some((node.id.as_str(), true)), depth + 1);
                self.block(Some((node.id.as_str(), false)), depth + 1);
            }
            for check in alone {
                self.check_row(check, depth);
            }
        }
        if let Some((_, then)) = braces {
            let rows = self.rows.get_mut(first..).unwrap_or_default();
            if let Some(row) = rows.first_mut() {
                row.opens = true;
            }
            // Those of an inner branch have theirs already.
            for row in rows.iter_mut().filter(|row| row.branch.is_none()) {
                row.branch = Some(if then { "then" } else { "else" });
            }
        }
    }

    /// Whether `check` reads one step.
    fn single(&self, check: &Node) -> bool {
        check_subjects_in(self.graphs, check).subjects.len() == 1
    }

    fn row(&mut self, node: &Node, depth: usize, kind: Kind) {
        self.rows.push(Row {
            id: node.id.clone(),
            handle: node.handle.clone(),
            name: node.display_name().to_owned(),
            number: self.numbers.get(node.id.as_str()).copied().unwrap_or(0),
            phase: self.phase,
            depth,
            branch: None,
            opens: false,
            kind,
        });
    }

    fn check_row(&mut self, check: &Node, depth: usize) {
        let text = self.check(check, None);
        self.row(check, depth, Kind::Check { text });
    }

    /// The check as its card writes it (DIAG-02), without the name of `own`.
    fn check(&self, check: &Node, own: Option<&str>) -> String {
        let scope = Scope {
            request: Some(check),
            ..self.texts.scope
        };
        check_text(check, scope, self.texts.graph, own).text()
    }

    fn kind(&self, node: &Node, checks: &[&Node]) -> Kind {
        let line = |keyword, text| Kind::Line { keyword, text };
        match node.kind {
            NodeKind::Request => Kind::Request(Box::new(self.request(node, checks))),
            NodeKind::Component => Kind::Call {
                arguments: node
                    .inputs
                    .iter()
                    .map(|(name, binding)| format!("{name}: {}", self.texts.written(binding)))
                    .collect::<Vec<_>>()
                    .join(", "),
                readers: self
                    .readers
                    .steps
                    .get(node.id.as_str())
                    .map_or(0, IndexSet::len),
            },
            NodeKind::If => line("if", self.check(node, None)),
            NodeKind::Condition => line(
                "condition",
                format!("{} = {}", node.handle, self.check(node, None)),
            ),
            NodeKind::Assert => Kind::Check {
                text: self.check(node, None),
            },
            NodeKind::Value | NodeKind::Export | NodeKind::Start => {
                let value = node
                    .inputs
                    .values()
                    .next()
                    .map_or_else(|| "missing value".to_owned(), |it| self.texts.written(it));
                line(
                    if node.kind == NodeKind::Export {
                        "export"
                    } else {
                        "value"
                    },
                    format!("{} = {value}", node.handle),
                )
            }
        }
    }

    fn request(&self, node: &Node, checks: &[&Node]) -> Request {
        let id = node.id.as_str();
        let url = node.inputs.get("url").map(|url| self.texts.url(url));
        Request {
            method: sends::method(node),
            path: url
                .as_deref()
                .map(crate::report::url_path)
                .unwrap_or_default()
                .to_owned(),
            reads: self.reads(node),
            column: match self.readers.lines.get(id) {
                Some(lines) => Column::Sets(lines.iter().cloned().collect()),
                None => Column::Sends(self.sends.get(id).cloned().unwrap_or(Sends {
                    common: None,
                    lines: Vec::new(),
                })),
            },
            checks: checks
                .iter()
                .map(|check| Check {
                    id: check.id.clone(),
                    handle: check.handle.clone(),
                    // Under another step than the one it reads the check
                    // keeps that step's name.
                    text: self.check(
                        check,
                        check_subjects_in(self.graphs, check)
                            .subjects
                            .iter()
                            .any(|subject| subject.step == id)
                            .then_some(id),
                    ),
                })
                .collect(),
        }
    }

    /// The names of what `node` reads, each once.
    fn reads(&self, node: &Node) -> Vec<String> {
        fn walk(binding: &Binding, texts: &Texts<'_>, out: &mut IndexSet<String>) {
            match binding {
                // A named input of the request itself.
                Binding::Literal { .. } | Binding::Argument { .. } => {}
                Binding::Object { fields } => {
                    fields.values().for_each(|field| walk(field, texts, out));
                }
                Binding::Array { items } => items.iter().for_each(|item| walk(item, texts, out)),
                Binding::Expression { inputs, .. } => {
                    inputs.values().for_each(|input| walk(input, texts, out));
                }
                Binding::Template { parts } => parts.iter().for_each(|part| {
                    if let TemplatePart::Value { binding } = part {
                        walk(binding, texts, out);
                    }
                }),
                other => {
                    out.insert(
                        texts
                            .reference(other)
                            .unwrap_or_else(|| crate::labels::binding(other, texts.graph)),
                    );
                }
            }
        }
        let mut out = IndexSet::new();
        for binding in node.inputs.values() {
            walk(binding, self.texts, &mut out);
        }
        out.into_iter().collect()
    }
}

#[cfg(test)]
pub(super) mod testing {
    use apiforge_core::format::{Project, Scenario};
    use apiforge_script::{Document, LowerContext, lower, parse};

    /// `source` as the window reads it.
    pub fn scenario(source: &str) -> (Project, Scenario) {
        let project = apiforge_core::demos::empty_project();
        let lowered = lower(&parse(source), &LowerContext::folder(&project));
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics);
        match lowered.document {
            Some(Document::Scenario(scenario)) => (project, scenario),
            other => panic!("a scenario: {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::scenario, *};

    fn model(source: &str) -> Model {
        let (project, scenario) = scenario(source);
        Model::of(&scenario.graph, Some(&scenario.cleanup), &project)
    }

    fn titles(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::title).collect()
    }

    fn checks(row: &Row) -> Vec<&str> {
        row.checks()
            .iter()
            .map(|check| check.text.as_str())
            .collect()
    }

    /// Criterion 3 and the ruling 5.2: a check is listed under the request
    /// written before it in its braces, and keeps the name of the step it
    /// reads when that is another one.
    #[test]
    fn a_check_stands_where_it_is_written() {
        let model = model(
            "scenario \"S\" {
  begin = start
  first = POST \"/a\" { sample 201 { id: 1 } }
  ok = assert first.status == 201
  gate = if first.status == 201 {
    second = POST \"/b\" { sample 200 { id: 1 } }
    own = assert second.body.id == 1
    other = assert first.status == 201
    both = assert second.body.id == first.body.id
  } else {
    missed = assert first.status == 201
  }
  late = assert first.body.id == 1
}",
        );
        assert_eq!(
            titles(&model.main),
            [
                "POST first",
                "if first.status == 201",
                "POST second",
                "second.body.id == first.body.id",
                "first.status == 201",
                "first.body.id == 1",
            ]
        );
        let rows = &model.main;
        assert_eq!(checks(&rows[0]), ["status == 201"]);
        assert_eq!(checks(&rows[2]), ["id == 1", "first.status == 201"]);
        assert_eq!(
            rows.iter()
                .map(|row| (row.depth, row.branch, row.opens))
                .collect::<Vec<_>>(),
            [
                (0, None, false),
                (0, None, false),
                (1, Some("then"), true),
                (1, Some("then"), false),
                (1, Some("else"), true),
                (0, None, false),
            ]
        );
        assert_eq!(model.summary(), "3 steps · 6 checks");
    }

    /// Review of the pull request: where the steps do not run in the order
    /// they are written (`@after`, language 1), a check stands under the step
    /// it reads, never under the one that happens to be written before it.
    #[test]
    fn a_check_of_steps_written_out_of_order_stands_under_the_step_it_reads() {
        let (project, mut scenario) = scenario(
            "scenario \"S\" {
  begin = start
  first = GET \"/a\"
  second = GET \"/b\"
  one = assert first.status == 200
  two = assert second.status == 201
}",
        );
        let at = |scenario: &apiforge_core::format::Scenario, handle: &str| {
            scenario
                .graph
                .nodes
                .iter()
                .position(|node| node.handle == handle)
                .unwrap()
        };
        // The engine still runs `first` before `second`.
        let (first, second) = (at(&scenario, "first"), at(&scenario, "second"));
        scenario.graph.nodes.swap(first, second);
        let model = Model::of(&scenario.graph, None, &project);
        assert_eq!(titles(&model.main), ["GET first", "GET second"]);
        assert_eq!(checks(&model.main[0]), ["status == 200"]);
        assert_eq!(checks(&model.main[1]), ["status == 201"]);
    }

    /// A check after a call, which is one line, has a row of its own.
    #[test]
    fn a_check_of_no_step_has_a_row_of_its_own() {
        let model = model(
            "scenario \"S\" {
  begin = start
  size = 2
  known = assert size == 2
  one = GET \"/a\"
  nothing = assert size == 3
  mine = assert one.status == 200
}",
        );
        assert_eq!(
            titles(&model.main),
            ["value size = 2", "size == 2", "GET one", "size == 3"]
        );
        assert_eq!(checks(&model.main[2]), ["status == 200"]);
    }

    /// Criterion 5: one line per field and reader, Cleanup's readers among
    /// them, as `graph::relations` lists them.
    #[test]
    fn readers_are_the_relations_readers() {
        let (project, scenario) = scenario(
            "scenario \"S\" {
  begin = start
  made = POST \"/a\" { sample 201 { id: 1, name: \"n\" } }
  ok = assert made.status == 201
  read = GET \"/a/{{made.body.id}}/{{made.body.id}}\" { query { n: made.body.name } }
  again = GET \"/b/{{made.body.id}}\"
  cleanup {
    gone = DELETE \"/a/{{made.body.id}}\"
    said = assert gone.status == 204
  }
}",
        );
        let model = Model::of(&scenario.graph, Some(&scenario.cleanup), &project);
        let Kind::Request(request) = &model.main[0].kind else {
            panic!("a request")
        };
        assert_eq!(
            request.column,
            Column::Sets(vec![
                "body.id → read".into(),
                "body.name → read".into(),
                "body.id → again".into(),
                "body.id → gone".into(),
            ])
        );
        // The same readers as the board's relations, Cleanup in its graph.
        let mut both = scenario.graph.clone();
        both.nodes.extend(scenario.cleanup.nodes.iter().cloned());
        for node in &both.nodes {
            let expected = !apiforge_core::graph::relations(&both, node)
                .readers
                .is_empty();
            let sets = model
                .rows()
                .find(|row| row.id == node.id)
                .and_then(|row| match &row.kind {
                    Kind::Request(request) => Some(matches!(request.column, Column::Sets(_))),
                    _ => None,
                });
            assert!(sets.is_none_or(|sets| sets == expected), "{}", node.handle);
        }
        let Kind::Request(gone) = &model.cleanup[0].kind else {
            panic!("a request")
        };
        assert_eq!(gone.reads, ["made.body.id"]);
        assert_eq!(checks(&model.cleanup[0]), ["status == 204"]);
    }

    /// What reads many steps stays quick: the model of a scenario of a
    /// hundred and eighty statements is built in a moment, as it is built at every change of
    /// the code.
    #[test]
    fn a_large_scenario_is_read_at_once() {
        let mut source = String::from("scenario \"S\" {\n  begin = start\n");
        for step in 0..60 {
            source.push_str(&format!(
                "  r{step} = POST \"/orders/{step}\" {{ body {{ n: {step}, kind: \"a\" }} }}\n  \
                 s{step} = assert r{step}.status == 200\n  t{step} = assert r{step}.body.n == {step}\n"
            ));
        }
        source.push('}');
        let (project, scenario) = scenario(&source);
        let start = std::time::Instant::now();
        let model = Model::of(&scenario.graph, Some(&scenario.cleanup), &project);
        let took = start.elapsed();
        assert_eq!(model.summary(), "60 steps · 120 checks");
        assert!(took.as_secs_f32() < 3.0, "{took:?}");
    }
}
