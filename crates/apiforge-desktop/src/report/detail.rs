//! What the details of a check say beside its step's (REP-07b): its
//! predicate as its card writes it (DIAG-02), what it compared with every
//! name, and the step it runs after, whose response is shown with the
//! checked field marked (DIAG-12).

use super::{RunGraphs, StepRow};
use apiforge_core::{
    format::Graph,
    graph::{component_definition, sequence},
    model::NodeKind,
    run::{Phase, StepResult},
};
use serde_json::Value;

/// What the details of a check say beside its step's.
pub(super) struct CheckDetail<'a> {
    /// Its predicate as its card writes it.
    pub predicate: String,
    /// What it compared, with every name: `weight_31.body.error.field`.
    pub checked: Option<String>,
    /// The step it runs after (the anchor of `canvas::collapsible`), and
    /// that step's number on the board.
    pub head: Option<(StepRow<'a>, Option<usize>)>,
}

impl CheckDetail<'_> {
    /// `Check weight_31_field · of step 6 Weight 31 kg · POST
    /// /parcels/pc-001/labels → 422`.
    pub fn line(&self, step: &StepResult) -> String {
        let mut line = format!("Check {}", step.handle);
        if let Some((head, number)) = &self.head {
            line.push_str(" · of step ");
            if let Some(number) = number {
                line.push_str(&format!("{number} "));
            }
            line.push_str(head.step.display_name());
            if let Some(request) = &head.step.request {
                line.push_str(&format!(" · {} {}", request.method, url_path(&request.url)));
                if let Some(status) = head.step.outputs.get("status").and_then(Value::as_u64) {
                    line.push_str(&format!(" → {status}"));
                }
            }
        }
        line
    }
}

/// The details of `rows[index]` when it is a check the graphs hold.
pub(super) fn of<'a>(
    rows: &[StepRow<'a>],
    index: usize,
    graphs: RunGraphs<'_>,
) -> Option<CheckDetail<'a>> {
    let row = rows.get(index)?;
    if !matches!(row.step.kind, NodeKind::Assert | NodeKind::Condition) {
        return None;
    }
    let phase = row.step.phase;
    let graph = graph_of(graphs, phase, &row.calls)?;
    let check = graph
        .nodes
        .iter()
        .find(|node| node.id == row.step.node_id)?;
    // The step it runs after, the last row of that step at its level that
    // ran before it.
    let later = (phase == Phase::Main && row.calls.is_empty())
        .then_some(graphs.cleanup)
        .flatten();
    let anchor = crate::canvas::collapsible(graph, later)
        .into_iter()
        .find(|collapsible| collapsible.check == check.id)
        .map(|collapsible| collapsible.anchor);
    let head = anchor.and_then(|anchor| {
        rows[..index]
            .iter()
            .rev()
            .find(|other| {
                other.step.phase == phase
                    && level(&other.key) == level(&row.key)
                    && other.step.node_id == anchor
            })
            .cloned()
    });
    let number = head.as_ref().and_then(|head| {
        sequence(graph)
            .iter()
            .position(|id| *id == head.step.node_id)
            .map(|at| at + 1)
    });
    // Cleanup's checks read Main's steps too, as DIAG-12's index reads them.
    let lookup = match (phase, row.calls.is_empty(), graphs.cleanup) {
        (Phase::Cleanup, true, Some(_)) => {
            let mut both = graphs.main.clone();
            both.nodes.extend(graph.nodes.iter().cloned());
            both
        }
        _ => graph.clone(),
    };
    let scope = apiforge_core::template_text::Scope {
        nodes: &lookup.nodes,
        project: graphs.project,
        request: Some(check),
    };
    // Under its step a check leaves the step's name out, as its card does.
    let own = head
        .is_some()
        .then(|| crate::canvas::own_step(check, &lookup))
        .flatten();
    let predicate = crate::canvas::check_text(check, scope, &lookup, own).text();
    let whole = crate::canvas::check_text(check, scope, &lookup, None);
    let checked = whole
        .actual
        .and_then(|at| whole.parts.get(at))
        .filter(|part| part.role == crate::canvas::PartRole::Operand)
        .map(|part| part.text.clone());
    Some(CheckDetail {
        predicate,
        checked,
        head: head.map(|head| (head, number)),
    })
}

/// The graph the steps of one level ran in: Main or Cleanup, through the
/// component calls that hold them.
fn graph_of<'a>(graphs: RunGraphs<'a>, phase: Phase, calls: &[&str]) -> Option<&'a Graph> {
    let mut graph = match phase {
        Phase::Main => graphs.main,
        Phase::Cleanup => graphs.cleanup?,
    };
    for call in calls {
        let call = graph.nodes.iter().find(|node| node.id == *call)?;
        graph = &component_definition(graphs.project, call)?.graph;
    }
    Some(graph)
}

/// The key of the level a row is at: its own key up to its last step.
fn level(key: &str) -> &str {
    key.rfind('/').map_or("", |at| &key[..=at])
}

/// The path of a request's URL, without its scheme and host. `://` is a
/// scheme's only after a scheme at the start.
fn url_path(url: &str) -> &str {
    let scheme = url.split_once("://").filter(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|char| char.is_ascii_alphanumeric() || matches!(char, '+' | '-' | '.'))
    });
    match scheme {
        Some((_, rest)) => rest.find('/').map_or("/", |at| &rest[at..]),
        None => url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_path_leaves_out_its_scheme_and_host() {
        assert_eq!(
            url_path("https://parcels.example.test/parcels/pc-001/labels"),
            "/parcels/pc-001/labels"
        );
        assert_eq!(url_path("http://localhost:3333"), "/");
        assert_eq!(
            url_path("/login?next=https://example.test/home"),
            "/login?next=https://example.test/home"
        );
    }
}
