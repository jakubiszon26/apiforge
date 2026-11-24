//! What the details of a check say beside its step's (REP-07b): its
//! predicate as its card writes it (DIAG-02), what it compared with every
//! name, and the step it runs after, read from the list as it is arranged;
//! and the response that holds the field it checked, marked (DIAG-12).

use super::{Line, StepRow, list};
use crate::inspector::CheckIndex;
use apiforge_core::{
    model::NodeKind,
    run::{Phase, StepResult},
};
use serde_json::Value;

/// What the details of a check say beside its step's.
pub(super) struct CheckDetail<'a> {
    /// Its predicate as its card writes it.
    pub predicate: String,
    /// What it compared, with every name, the hover of its title:
    /// `weight_31.body.error.field`.
    pub checked: Option<String>,
    /// The step it is listed under, and that step's number on the board.
    head: Option<(&'a StepResult, Option<usize>)>,
    /// The step whose response holds what it compared, or else the one it
    /// is listed under, by its row.
    pub source: Option<&'a StepRow<'a>>,
    /// The fields it compared.
    reads: Vec<list::Read>,
}

impl CheckDetail<'_> {
    /// `Check weight_31_field · of step 6 Weight 31 kg · POST
    /// /parcels/pc-001/labels → 422`.
    pub fn line(&self, step: &StepResult) -> String {
        let mut line = format!("Check {}", step.display_name());
        if let Some((head, number)) = self.head {
            line.push_str(" · of step ");
            if let Some(number) = number {
                line.push_str(&format!("{number} "));
            }
            line.push_str(head.display_name());
            if let Some(request) = &head.request {
                line.push_str(&format!(
                    " · {} {}",
                    request.method,
                    list::url_path(&request.url)
                ));
                if let Some(status) = head.outputs.get("status").and_then(Value::as_u64) {
                    line.push_str(&format!(" → {status}"));
                }
            }
        }
        line
    }

    /// `Response of Weight 31 kg · the checked field is marked`, the last
    /// part only where the response has the field and `index` still marks
    /// it for the check `check`.
    pub fn heading(&self, check: &str, index: Option<&CheckIndex>) -> Option<String> {
        let source = self.source?;
        let what = match source.step.kind {
            NodeKind::Request => "Response",
            _ => "Outputs",
        };
        let mut heading = format!("{what} of {}", source.step.display_name());
        let marked = self.reads.iter().any(|read| {
            read.node == source.step.node_id
                && source
                    .step
                    .outputs
                    .get(&read.port)
                    .is_some_and(|value| apiforge_core::path::resolve(value, &read.path).is_some())
                && index.is_some_and(|index| index.marks(check, &read.node, &read.port, &read.path))
        });
        if marked {
            heading.push_str(" · the checked field is marked");
        }
        Some(heading)
    }
}

/// The details of `rows[index]` when the list wrote it as a check.
pub(super) fn of<'a>(
    rows: &'a [StepRow<'a>],
    index: usize,
    arranged: &list::Arranged,
) -> Option<CheckDetail<'a>> {
    let row = rows.get(index)?;
    let entry = arranged.entries.get(index)?;
    let predicate = entry.predicate.clone()?;
    let head = arranged
        .lines
        .iter()
        .find(|line: &&Line| line.row == index)
        .and_then(|line| line.under);
    // The last row before the check of a step it compared: one of its
    // level, or of Main for a check of Cleanup.
    let source_of = |node: &str| {
        rows[..index].iter().rposition(|other| {
            other.step.node_id == node
                && list::level(&other.key) == list::level(&row.key)
                && (other.step.phase == row.step.phase
                    || (row.step.phase == Phase::Cleanup && row.calls.is_empty()))
                && matches!(other.step.kind, NodeKind::Request | NodeKind::Component)
        })
    };
    let sources: Vec<usize> = entry
        .reads
        .iter()
        .filter_map(|read| source_of(&read.node))
        .collect();
    let source = head
        .filter(|head| sources.contains(head))
        .or(sources.first().copied())
        .or(head);
    Some(CheckDetail {
        predicate,
        checked: entry.checked.clone(),
        head: head.map(|head| (rows[head].step, arranged.entries[head].number)),
        source: source.map(|source| &rows[source]),
        reads: entry.reads.clone(),
    })
}
