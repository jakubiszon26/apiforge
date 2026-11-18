//! A change of a field's source, held until *Apply* (LAY-42): which fields
//! the Inspector stages, and the summary it shows above *Apply*.
//!
//! The summary is computed from the document — its bindings and the static
//! order of the run — and says where the step runs by its number and its
//! branch, never by the lines of the board, so it stays true when the order
//! is the written order.

use super::response;
use apiforge_core::{
    format::{Binding, Graph, Node, Project},
    graph::{input_ports, output_ports, sequence},
    model::Branch,
    path,
    run::RunResult,
};
use std::collections::HashSet;

/// A reference as a binding holds it: the step, its output and the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub node_id: String,
    pub output: String,
    pub field: String,
}

/// The change a held choice makes: `reader`'s `input` at `path` reads `new`
/// instead of `old`.
pub struct Change<'a> {
    pub reader: &'a Node,
    pub input: &'a str,
    pub path: &'a [String],
    pub old: &'a Source,
    pub new: &'a Source,
}

/// What the summary reads: the nodes a reference may point to, the Cleanup
/// steps that may read them too, the project and the last run.
#[derive(Clone, Copy)]
pub struct Document<'a> {
    pub graph: &'a Graph,
    pub cleanup: Option<&'a Graph>,
    pub project: &'a Project,
    pub run: Option<&'a RunResult>,
}

/// One part of a sentence, and whether it is written as code.
pub type Part = (String, bool);

/// A sentence of the summary, in parts.
pub type Sentence = Vec<Part>;

/// The text of a sentence.
#[cfg(test)]
pub fn text(sentence: &[Part]) -> String {
    sentence.iter().map(|(text, _)| text.as_str()).collect()
}

/// A path inside an input as the code writes it: `roomId`, `items[1].id`.
pub fn field_text(path: &[String]) -> String {
    let mut text = String::new();
    for segment in path {
        if segment.parse::<usize>().is_ok() {
            text.push_str(&format!("[{segment}]"));
        } else {
            if !text.is_empty() {
                text.push('.');
            }
            text.push_str(segment);
        }
    }
    text
}

fn code(text: impl Into<String>) -> Part {
    (text.into(), true)
}

fn words(text: impl Into<String>) -> Part {
    (text.into(), false)
}

fn node<'a>(document: &Document<'a>, id: &str) -> Option<&'a Node> {
    document.graph.nodes.iter().find(|node| node.id == id)
}

fn handle(document: &Document<'_>, id: &str) -> String {
    node(document, id).map_or_else(|| id.to_owned(), |node| node.handle.clone())
}

/// The reference as the code writes it: a variable, or a step with one
/// output, is named without its output (`slot.start`, `first.body.roomId`).
pub fn written(document: &Document<'_>, source: &Source) -> String {
    let Some(node) = node(document, &source.node_id) else {
        return source.node_id.clone();
    };
    let named = !node.is_variable() && output_ports(node, document.project).len() != 1;
    let mut text = node.handle.clone();
    if named {
        text.push('.');
        text.push_str(&source.output);
    }
    if !source.field.is_empty() {
        if !source.field.starts_with('[') {
            text.push('.');
        }
        text.push_str(&source.field);
    }
    text
}

/// The value `source` holds: the last run's when there is one, else the
/// sample response's; `None` when neither has the field.
fn sample(document: &Document<'_>, source: &Source) -> Option<String> {
    let node = node(document, &source.node_id)?;
    let data = response::source(node, &source.output, document.project, document.run);
    let value = data.value?;
    let value = path::resolve(&value, &source.field)?;
    Some(apiforge_core::js::json_stringify(&value))
}

fn sampled(document: &Document<'_>, source: &Source) -> Vec<Part> {
    match sample(document, source) {
        Some(value) => vec![words(" (sample "), code(value), words(")")],
        None => vec![words(" (no sample)")],
    }
}

/// `1st`, `2nd`, `8th`, `11th`.
pub fn ordinal(number: usize) -> String {
    let suffix = match (number % 10, number % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{number}{suffix}")
}

/// The branches `id` stands in, innermost first: each condition step and
/// the branch of it whose steps lead to `id`, unless both of its branches
/// do.
pub fn branches(graph: &Graph, id: &str) -> Vec<(String, Branch)> {
    let order = sequence(graph);
    let place = |id: &str| order.iter().position(|step| *step == id);
    let mut found: Vec<(String, Branch)> = Vec::new();
    for edge in &graph.edges {
        let Some(branch) = edge.branch else {
            continue;
        };
        // Every step reached from the branch's first step.
        let mut reached: HashSet<&str> = HashSet::from([edge.target.as_str()]);
        let mut pending = vec![edge.target.as_str()];
        while let Some(at) = pending.pop() {
            for next in graph.edges.iter().filter(|next| next.source == at) {
                if reached.insert(next.target.as_str()) {
                    pending.push(next.target.as_str());
                }
            }
        }
        if reached.contains(id) && !found.contains(&(edge.source.clone(), branch)) {
            found.push((edge.source.clone(), branch));
        }
    }
    // A step both branches lead to runs after the condition, in neither.
    let joined: Vec<String> = found
        .iter()
        .filter(|(condition, branch)| {
            found
                .iter()
                .any(|(other, other_branch)| other == condition && other_branch != branch)
        })
        .map(|(condition, _)| condition.clone())
        .collect();
    found.retain(|(condition, _)| !joined.contains(condition));
    found.sort_by_key(|(condition, _)| std::cmp::Reverse(place(condition)));
    found
}

fn branch_name(document: &Document<'_>, (condition, branch): &(String, Branch)) -> String {
    format!(
        "{} → {}",
        handle(document, condition),
        match branch {
            Branch::True => "Yes",
            Branch::False => "No",
        }
    )
}

/// `a`, `a and b`, `a, b and c`, each written as code.
fn listed(names: &[String]) -> Vec<Part> {
    let mut parts = Vec::new();
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            parts.push(words(if index + 1 == names.len() {
                " and "
            } else {
                ", "
            }));
        }
        parts.push(code(name.clone()));
    }
    parts
}

/// The graph as it is once `change` is applied.
fn applied(graph: &Graph, change: &Change<'_>) -> Graph {
    let mut graph = graph.clone();
    let reference = Binding::reference(
        change.new.node_id.clone(),
        change.new.output.clone(),
        change.new.field.clone(),
    );
    if let Some(reader) = graph
        .nodes
        .iter_mut()
        .find(|node| node.id == change.reader.id)
        && let Some(binding) = reader.inputs.get(change.input)
    {
        let next = if change.path.is_empty() {
            Some(reference)
        } else {
            binding.replace_at(change.path, reference)
        };
        if let Some(next) = next {
            reader.inputs.insert(change.input.to_owned(), next);
        }
    }
    graph
}

/// The summary of `change`, sentence by sentence (LAY-42):
///
/// 1. the field, the new and the old reference as the code writes them,
///    each with its sample;
/// 2. that only this field changes;
/// 3. the step's number in the run, its branch and what it runs after;
/// 4. who still reads the old source;
/// 5. when the new source stands in a branch the reader does not, the
///    branch in which its value does not exist.
pub fn summary(document: &Document<'_>, change: &Change<'_>) -> Vec<Sentence> {
    let reader = change.reader;
    let mut sentences = Vec::new();

    // 1. The field, the new reference and the old.
    let input = input_ports(reader, document.project)
        .into_iter()
        .find(|port| port.name == change.input)
        .map_or_else(
            || change.input.to_owned(),
            |port| port.display_name().to_owned(),
        );
    let mut first = vec![code(reader.handle.clone()), words(format!(" · {input}"))];
    if !change.path.is_empty() {
        first.push(words(" · "));
        first.push(code(field_text(change.path)));
    }
    first.push(words(" will read "));
    first.push(code(written(document, change.new)));
    first.extend(sampled(document, change.new));
    first.push(words(" instead of "));
    first.push(code(written(document, change.old)));
    first.extend(sampled(document, change.old));
    first.push(words("."));
    sentences.push(first);

    // 2.
    sentences.push(vec![words("Only this field changes.")]);

    // 3. Where the step runs: its number, its branch, after the new source.
    let before = sequence(document.graph)
        .iter()
        .position(|step| *step == reader.id);
    let after_change = applied(document.graph, change);
    let order = sequence(&after_change);
    let place = order.iter().position(|step| *step == reader.id);
    if let Some(place) = place {
        let mut runs = vec![code(reader.handle.clone())];
        runs.push(words(if before == Some(place) {
            " still runs "
        } else {
            " will run "
        }));
        runs.push(words(ordinal(place + 1)));
        let own = branches(document.graph, &reader.id);
        if let Some(innermost) = own.first() {
            runs.push(words(", in "));
            runs.push(code(branch_name(document, innermost)));
        }
        if order
            .iter()
            .position(|step| *step == change.new.node_id)
            .is_some_and(|source| source < place)
        {
            runs.push(words(", after "));
            runs.push(code(handle(document, &change.new.node_id)));
        }
        runs.push(words("."));
        sentences.push(runs);
    }

    // 4. Who still reads the old source.
    let old = handle(document, &change.old.node_id);
    let mut kept: Vec<String> = Vec::new();
    for (input, binding) in &reader.inputs {
        binding.for_each_leaf(&mut |leaf, at| {
            if leaf.references_node(&change.old.node_id)
                && !(input == change.input && at == change.path)
            {
                let field = if input == change.input {
                    field_text(at)
                } else if at.is_empty() {
                    input.clone()
                } else {
                    format!("{input}.{}", field_text(at))
                };
                if !kept.contains(&field) {
                    kept.push(field);
                }
            }
        });
    }
    let still_reads = !kept.is_empty() || change.new.node_id == change.old.node_id;
    if still_reads {
        let mut sentence = vec![
            code(reader.handle.clone()),
            words(" still reads "),
            code(old.clone()),
        ];
        if !kept.is_empty() {
            sentence.push(words(" in "));
            sentence.extend(listed(&kept));
        }
        sentence.push(words("."));
        sentences.push(sentence);
    }
    let others: Vec<String> = document
        .graph
        .nodes
        .iter()
        .chain(document.cleanup.into_iter().flat_map(|graph| &graph.nodes))
        .filter(|node| node.id != reader.id)
        .filter(|node| {
            node.inputs
                .values()
                .any(|binding| binding.references_node(&change.old.node_id))
        })
        .map(|node| node.handle.clone())
        .fold(Vec::new(), |mut names, name| {
            if !names.contains(&name) {
                names.push(name);
            }
            names
        });
    if !others.is_empty() {
        let mut sentence = vec![code(old.clone()), words(" is still read by ")];
        sentence.extend(listed(&others));
        sentence.push(words("."));
        sentences.push(sentence);
    } else if !still_reads {
        sentences.push(vec![code(old), words(" is no longer read by anything.")]);
    }

    // 5. A source in a branch the reader does not stand in.
    let own = branches(document.graph, &reader.id);
    if let Some((condition, branch)) = branches(document.graph, &change.new.node_id)
        .into_iter()
        .find(|branch| !own.contains(branch))
    {
        let other = match branch {
            Branch::True => Branch::False,
            Branch::False => Branch::True,
        };
        sentences.push(vec![
            words("This value does not exist when "),
            code(branch_name(document, &(condition, other))),
            words(" runs."),
        ]);
    }
    sentences
}

#[cfg(test)]
mod tests {
    //! The summary's sentences on a small booking scenario: a sign-in, a
    //! list of rooms, two bookings, the second in the `Yes` branch of a
    //! condition, a step after the whole condition.
    use super::*;
    use apiforge_script::{Document as Script, LowerContext, lower, parse};

    const SCENARIO: &str = r#"scenario "Double booking" {
  begin = start
  @after(begin)
  token = "secret"
  rooms = GET "/rooms" {
    headers { Authorization: "Bearer {{token}}" }
    sample 200 { items: [{ id: "room-a", name: "Room A" }, { id: "room-b", name: "Room B" }] }
  }
  slot = { start: "10:00", end: "11:00" }
  first = POST "/bookings" {
    body { roomId: rooms.body.items[0].id, start: slot.start, end: slot.end }
    sample 201 { id: "bk-1", roomId: "room-a" }
  }
  booked = if first.status == 201 {
    second = POST "/bookings" {
      body { roomId: rooms.body.items[1].id, start: slot.start, end: slot.end }
      sample 409 { error: { code: "room_unavailable" } }
    }
    calendar = GET "/rooms/{{rooms.body.items[0].id}}/bookings" {
      sample 200 { items: [] }
    }
  } else {
    not_booked = assert first.status == 201
  }
  @after(booked)
  report = POST "/reports" {
    body { booking: first.body.id }
  }
}"#;

    fn graph(text: &str) -> (Graph, Project) {
        let project = apiforge_core::demos::empty_project();
        let lowered = lower(&parse(text), &LowerContext::folder(&project));
        let Some(Script::Scenario(scenario)) = lowered.document else {
            panic!("the scenario lowers: {:#?}", lowered.diagnostics);
        };
        (scenario.graph, project)
    }

    fn id(graph: &Graph, handle: &str) -> String {
        graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a step {handle}"))
            .id
            .clone()
    }

    /// The summary of `reader.input.path` reading `new` (`step`, output,
    /// field) instead of what it reads now.
    fn says(
        text: &str,
        reader: &str,
        input: &str,
        path: &[&str],
        new: (&str, &str, &str),
    ) -> Vec<String> {
        let (graph, project) = graph(text);
        let node = graph
            .nodes
            .iter()
            .find(|node| node.handle == reader)
            .unwrap()
            .clone();
        let path: Vec<String> = path.iter().map(|part| (*part).to_owned()).collect();
        let Some(Binding::Reference {
            node_id,
            port,
            path: field,
        }) = node.inputs[input].at(&path)
        else {
            panic!("{reader}.{input} reads a step there");
        };
        let old = Source {
            node_id: node_id.clone(),
            output: port.clone(),
            field: field.clone(),
        };
        let new = Source {
            node_id: id(&graph, new.0),
            output: new.1.to_owned(),
            field: new.2.to_owned(),
        };
        let document = Document {
            graph: &graph,
            cleanup: None,
            project: &project,
            run: None,
        };
        let lines: Vec<String> = summary(
            &document,
            &Change {
                reader: &node,
                input,
                path: &path,
                old: &old,
                new: &new,
            },
        )
        .iter()
        .map(|sentence| super::text(sentence))
        .collect();
        for line in &lines {
            let lower = line.to_lowercase();
            assert!(
                !["@after", "wire", "edge"]
                    .iter()
                    .any(|word| lower.contains(word)),
                "{line}"
            );
        }
        lines
    }

    #[test]
    fn the_summary_of_a_change_in_a_branch() {
        assert_eq!(
            says(
                SCENARIO,
                "second",
                "body",
                &["roomId"],
                ("first", "body", "roomId")
            ),
            [
                "second · Body · roomId will read first.body.roomId (sample \"room-a\") instead of rooms.body.items[1].id (sample \"room-b\").",
                "Only this field changes.",
                "second still runs 8th, in booked → Yes, after first.",
                "rooms is still read by first and calendar.",
            ]
        );
    }

    #[test]
    fn the_old_source_left_with_no_reader() {
        let text = SCENARIO
            .replacen("roomId: rooms.body.items[0].id", "roomId: \"room-a\"", 1)
            .replacen("{{rooms.body.items[0].id}}", "room-a", 1);
        let lines = says(
            &text,
            "second",
            "body",
            &["roomId"],
            ("first", "body", "roomId"),
        );
        assert_eq!(lines[3], "rooms is no longer read by anything.");
    }

    #[test]
    fn the_reader_keeps_another_read() {
        let text = SCENARIO.replacen(
            "roomId: rooms.body.items[1].id, start: slot.start",
            "roomId: rooms.body.items[1].id, start: rooms.body.items[1].name",
            1,
        );
        let lines = says(
            &text,
            "second",
            "body",
            &["roomId"],
            ("first", "body", "roomId"),
        );
        assert_eq!(lines[3], "second still reads rooms in start.");
        assert_eq!(lines[4], "rooms is still read by first and calendar.");
    }

    #[test]
    fn a_step_in_no_branch_has_no_branch() {
        let lines = says(
            SCENARIO,
            "first",
            "body",
            &["roomId"],
            ("slot", "value", "start"),
        );
        assert_eq!(
            lines[0],
            "first · Body · roomId will read slot.start (no sample) instead of rooms.body.items[0].id (sample \"room-a\")."
        );
        assert_eq!(lines[2], "first still runs 5th, after slot.");
    }

    #[test]
    fn a_field_with_no_sample() {
        let lines = says(
            SCENARIO,
            "second",
            "body",
            &["roomId"],
            ("first", "body", "room.number"),
        );
        assert!(
            lines[0].contains("will read first.body.room.number (no sample) instead of"),
            "{}",
            lines[0]
        );
    }

    #[test]
    fn a_source_in_another_branch() {
        let lines = says(
            SCENARIO,
            "report",
            "body",
            &["booking"],
            ("second", "body", "error"),
        );
        assert_eq!(
            lines.last().unwrap(),
            "This value does not exist when booked → No runs."
        );
        assert!(!lines[2].contains(" in "), "{}", lines[2]);
    }

    #[test]
    fn ordinals() {
        let said: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 103, 111]
            .into_iter()
            .map(ordinal)
            .collect();
        assert_eq!(
            said,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "103rd",
                "111th"
            ]
        );
    }
}
