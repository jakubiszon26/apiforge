//! The rows of *Steps* (REQ-01a).

use super::sends::Sends;
use apiforge_core::{
    format::{Graph, Project},
    run::Phase,
    template_text::Scope,
};

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
    pub fn of(_main: &Graph, _cleanup: Option<&Graph>, _project: &Project) -> Self {
        Self::default()
    }

    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.main.iter().chain(&self.cleanup)
    }

    pub fn summary(&self) -> String {
        String::new()
    }
}

/// The texts of bindings, as they are written.
pub struct Texts<'a> {
    pub scope: Scope<'a>,
    pub graph: &'a Graph,
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
