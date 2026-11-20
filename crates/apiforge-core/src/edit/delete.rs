//! Deleting statements on the board (DIAG-06e).
//!
//! One function, [`delete_preview`], answers what a delete would do: which
//! statements go and which statements that stay would be left reading
//! something that went. The refusal, the confirmation and the delete itself
//! all use its answer, so what the person is told is what happens.
//!
//! What goes is the selection; for an `if`, every statement in its braces at
//! any depth; and a check, value or variable that reads at least one
//! statement that goes and no statement that stays, until nothing more is
//! added. A statement that reads no statement at all — a literal, a check of
//! constants, a value of a parameter or of `env` only — belongs to nothing
//! and stays.
//!
//! A delete with a statement left reading is refused, and a delete that is
//! done changes no input of a statement that stays. The legacy window keeps
//! [`crate::graph::delete_nodes`], which drops such inputs instead.
use super::EditError;
use crate::format::Graph;
use std::collections::HashSet;

/// What deleting a selection would do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deletion {
    /// The node identifiers of every statement that goes, in the order of
    /// the graph.
    pub gone: Vec<String>,
    /// Statements that stay and read something that goes: of the graph
    /// first, then of the graphs beside it, each in its order.
    pub readers: Vec<Reader>,
    /// *Start* would go while a step of its flow stays.
    pub start: bool,
    /// Every selected `if` with statements in its braces, outermost only.
    pub braces: Vec<Braced>,
}

/// A statement that stays and reads something that goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reader {
    pub id: String,
    pub handle: String,
    /// What it reads that goes, in the order of the graph.
    pub reads: Vec<Read>,
}

/// A statement that goes, as a reader reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub handle: String,
    /// The selected statements it goes with, when it is not selected itself:
    /// the `if` whose braces hold it, or the statements a check, value or
    /// variable reads.
    pub with: Vec<String>,
}

/// A selected `if` with statements in its braces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Braced {
    pub handle: String,
    /// The steps in its braces at any depth.
    pub steps: usize,
    /// Every statement in its braces at any depth.
    pub statements: usize,
}

impl Deletion {
    /// Why the delete is refused, as the board says it; `None` when it may
    /// be done.
    pub fn refusal(&self) -> Option<String> {
        todo!("DIAG-06e")
    }

    /// The one question a delete asks before it is done: when a selected
    /// `if` has statements in its braces. `None` when there is nothing to
    /// ask.
    pub fn question(&self) -> Option<String> {
        todo!("DIAG-06e")
    }
}

/// What deleting `selection` from `graph` would do. `beside` are the graphs
/// whose statements may read `graph`'s (a scenario's cleanup, which sees the
/// main flow); their statements stay and are never taken with the delete.
/// `needs_start` is true for the main flow of a scenario, which starts at
/// *Start* while it has a step.
pub fn delete_preview(
    _graph: &Graph,
    _beside: &[&Graph],
    _selection: &HashSet<&str>,
    _needs_start: bool,
) -> Deletion {
    todo!("DIAG-06e")
}

/// `graph` without what `deletion` takes. No input of a statement that
/// stays is changed.
pub fn apply_deletion(_graph: &Graph, _deletion: &Deletion) -> Graph {
    todo!("DIAG-06e")
}

/// Deletes `selection` from `graph` by [`delete_preview`]'s answer, or
/// refuses with the board's sentence.
pub fn delete_statements(
    _graph: &Graph,
    _beside: &[&Graph],
    _selection: &HashSet<&str>,
    _needs_start: bool,
) -> Result<Graph, EditError> {
    todo!("DIAG-06e")
}

#[cfg(test)]
mod tests {
    //! DIAG-06e, criteria 1–5 on the preview. The scenarios are written in
    //! the language and lowered by `apiforge-script`, which links another
    //! build of this crate: the documents cross between the two builds as
    //! JSON. Every edit ends by printing the edited scenario, reading it back
    //! without an error and as the same document.
    use super::*;
    use crate::{
        chain::{edited, is_step, with_start},
        format::{Project, Scenario},
    };
    use serde::{Serialize, de::DeserializeOwned};

    /// A value of one build of this crate as the same type of the other.
    fn crossed<A: Serialize, B: DeserializeOwned>(value: &A) -> B {
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
    }

    fn lowered(project: &Project, text: &str) -> Scenario {
        let theirs = crossed(project);
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(text),
            &apiforge_script::LowerContext::folder(&theirs),
        );
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        match lowered.document {
            Some(apiforge_script::Document::Scenario(scenario)) => crossed(&scenario),
            other => panic!("not a scenario: {other:?}"),
        }
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/scenarios/lay")
                .join(name),
        )
        .unwrap()
    }

    fn shop() -> (Project, Scenario) {
        let project = crate::demos::shop_project();
        let scenario = lowered(&project, &fixture("shop-30.afs"));
        (project, scenario)
    }

    fn small(text: &str) -> (Project, Scenario) {
        let project = super::super::test_support::empty_project();
        let scenario = lowered(&project, text);
        (project, scenario)
    }

    fn id(scenario: &Scenario, handle: &str) -> String {
        scenario
            .graph
            .nodes
            .iter()
            .chain(&scenario.cleanup.nodes)
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a statement `{handle}`"))
            .id
            .clone()
    }

    fn handles(scenario: &Scenario, ids: &[String]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                scenario
                    .graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
            .collect()
    }

    /// The preview of deleting `selected` from the main flow.
    fn preview(scenario: &Scenario, selected: &[&str]) -> Deletion {
        let ids: Vec<String> = selected.iter().map(|handle| id(scenario, handle)).collect();
        delete_preview(
            &scenario.graph,
            &[&scenario.cleanup],
            &ids.iter().map(String::as_str).collect(),
            true,
        )
    }

    fn reader_handles(deletion: &Deletion) -> Vec<&str> {
        deletion
            .readers
            .iter()
            .map(|reader| reader.handle.as_str())
            .collect()
    }

    /// Deletes `selected` from the main flow, as the session writes it, and
    /// checks the text: it passes the check and reads back as the document.
    fn deleted(project: &Project, scenario: &Scenario, selected: &[&str]) -> Scenario {
        let ids: Vec<String> = selected.iter().map(|handle| id(scenario, handle)).collect();
        let graph = delete_statements(
            &scenario.graph,
            &[&scenario.cleanup],
            &ids.iter().map(String::as_str).collect(),
            true,
        )
        .expect("the delete is done");
        let mut after = scenario.clone();
        after.graph = with_start(&edited(&scenario.graph, &graph), &[&after.cleanup]);
        let theirs = crossed(project);
        let document = apiforge_script::Document::Scenario(crossed(&after));
        let text = apiforge_script::print_folder_document(&theirs, &document)
            .expect("the edited scenario prints");
        let read = lowered(project, &text);
        assert_eq!(
            apiforge_script::semantic(
                &apiforge_script::Document::Scenario(crossed(&read)),
                &theirs
            ),
            apiforge_script::semantic(&document, &theirs),
            "the text reads back as the document:\n{text}"
        );
        after
    }

    fn steps(scenario: &Scenario) -> Vec<String> {
        crate::graph::sequence(&scenario.graph)
            .into_iter()
            .filter_map(|id| scenario.graph.nodes.iter().find(|node| node.id == id))
            .filter(|node| is_step(node.kind))
            .map(|node| node.handle.clone())
            .collect()
    }

    const PAY: &str = r#"scenario "Pay" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1", total: 10 }
  }
  pay = POST "/pay/{{cart.body.id}}" {
    sample 200 { ok: true }
  }
  receipt = GET "/receipts" {
    query { cart: cart.body.id }
    sample 200 { total: 10 }
  }
  done = GET "/done" {
    sample 200 {}
  }
}
"#;

    // Criterion 1 -------------------------------------------------------------

    #[test]
    fn delete_preview_refuses_a_step_that_is_read_and_names_every_reader() {
        let (_, scenario) = small(PAY);
        let deletion = preview(&scenario, &["cart"]);
        assert_eq!(reader_handles(&deletion), ["pay", "receipt"]);
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`pay` and `receipt` read `cart`. Change or delete them first.")
        );
        let ids = [id(&scenario, "cart")];
        let refused = delete_statements(
            &scenario.graph,
            &[&scenario.cleanup],
            &ids.iter().map(String::as_str).collect(),
            true,
        );
        assert!(matches!(refused, Err(EditError::DeleteRefused(_))));
    }

    #[test]
    fn delete_preview_with_the_readers_selected_deletes_all_and_joins_the_neighbours() {
        let (project, scenario) = small(PAY);
        let deletion = preview(&scenario, &["cart", "pay", "receipt"]);
        assert_eq!(deletion.refusal(), None);
        let after = deleted(&project, &scenario, &["cart", "pay", "receipt"]);
        assert_eq!(steps(&after), ["begin", "done"]);
    }

    #[test]
    fn delete_preview_refuses_a_reader_in_the_cleanup() {
        let (_, scenario) = shop();
        let deletion = preview(&scenario, &["cart"]);
        assert_eq!(
            reader_handles(&deletion),
            [
                "cart_total",
                "cart_view",
                "address",
                "order",
                "receipt_total",
                "removed"
            ],
            "`cart_created` reads only `cart` and goes with it"
        );
        assert!(
            deletion.gone.contains(&id(&scenario, "cart_created")),
            "{:?}",
            handles(&scenario, &deletion.gone)
        );
    }

    #[test]
    fn delete_preview_refuses_a_reader_whose_input_is_optional() {
        let (_, scenario) = shop();
        // `checkout`'s body reads `expected_total`: refused, nothing dropped.
        let deletion = preview(&scenario, &["expected_total"]);
        assert!(reader_handles(&deletion).contains(&"checkout"));
        let ids = [id(&scenario, "expected_total")];
        assert!(
            delete_statements(
                &scenario.graph,
                &[&scenario.cleanup],
                &ids.iter().map(String::as_str).collect(),
                true,
            )
            .is_err()
        );
    }

    // Criterion 2 -------------------------------------------------------------

    #[test]
    fn delete_preview_refuses_a_reader_of_something_else_that_stays() {
        let (_, scenario) = small(
            r#"scenario "Both" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  b = GET "/b" {
    sample 200 {}
  }
  both = assert a.status == b.status
}
"#,
        );
        let deletion = preview(&scenario, &["a"]);
        assert_eq!(reader_handles(&deletion), ["both"]);
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`both` reads `a`. Change or delete it first.")
        );
    }

    // Criterion 3 -------------------------------------------------------------

    const EXPANDS: &str = r#"scenario "Expands" {
  param limit = 5
  begin = start
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  v = a.body.n
  timeout = 30
  ok = assert 1 == 1
  limited = param.limit
  b = POST "/b" {
    body { n: v }
    sample 200 {}
  }
  c = POST "/c" {
    body { wait: timeout }
    sample 200 {}
  }
}
"#;

    #[test]
    fn delete_preview_expands() {
        let (project, scenario) = small(EXPANDS);
        let deletion = preview(&scenario, &["a"]);
        assert_eq!(handles(&scenario, &deletion.gone), ["a", "v"]);
        assert_eq!(
            reader_handles(&deletion),
            ["b"],
            "`b` reads `a` through `v`"
        );
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`b` reads `v`. Change or delete it first. `v` would go with `a`.")
        );
        let deletion = preview(&scenario, &["a", "b"]);
        assert_eq!(handles(&scenario, &deletion.gone), ["a", "v", "b"]);
        assert_eq!(deletion.refusal(), None);
        let after = deleted(&project, &scenario, &["a", "b"]);
        assert_eq!(steps(&after), ["begin", "c"]);
    }

    #[test]
    fn delete_preview_leaves_what_reads_no_statement() {
        let (project, scenario) = small(EXPANDS);
        let deletion = preview(&scenario, &["a", "b"]);
        for kept in ["timeout", "ok", "limited", "c"] {
            assert!(
                !deletion.gone.contains(&id(&scenario, kept)),
                "`{kept}` reads no statement that goes and stays"
            );
        }
        assert!(
            deletion.readers.is_empty(),
            "`c` reads `timeout`, which stays"
        );
        let after = deleted(&project, &scenario, &["a", "b"]);
        for kept in ["timeout", "ok", "limited", "c"] {
            assert!(after.graph.nodes.iter().any(|node| node.handle == kept));
        }
    }

    // Criterion 4 -------------------------------------------------------------

    #[test]
    fn delete_preview_surviving_inputs_are_untouched() {
        let (project, scenario) = shop();
        let after = deleted(&project, &scenario, &["refund", "refund_amount"]);
        for node in after.graph.nodes.iter().chain(&after.cleanup.nodes) {
            let before = scenario
                .graph
                .nodes
                .iter()
                .chain(&scenario.cleanup.nodes)
                .find(|old| old.id == node.id)
                .expect("a statement that stayed");
            assert_eq!(before.inputs.len(), node.inputs.len(), "`{}`", node.handle);
            for (name, binding) in &before.inputs {
                assert_eq!(
                    node.inputs.get(name),
                    Some(binding),
                    "`{}`.{name} is unchanged",
                    node.handle
                );
            }
        }
    }

    // Criterion 5 -------------------------------------------------------------

    #[test]
    fn delete_preview_an_if_takes_its_braces_and_asks_once() {
        let (project, scenario) = shop();
        let deletion = preview(&scenario, &["paid"]);
        assert_eq!(deletion.refusal(), None);
        assert_eq!(
            handles(&scenario, &deletion.gone),
            [
                "paid",
                "payment",
                "receipt",
                "receipt_total",
                "refund",
                "refund_amount",
                "logout",
                "not_created"
            ]
        );
        assert_eq!(
            deletion.question().as_deref(),
            Some("Delete `paid` and the 4 steps inside it?")
        );
        let after = deleted(&project, &scenario, &["paid"]);
        assert_eq!(steps(&after).last().map(String::as_str), Some("order"));
    }

    #[test]
    fn delete_preview_an_if_read_from_outside_is_refused_before_any_question() {
        let (_, scenario) = small(
            r#"scenario "Outside" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  x = if a.status == 200 {
    inner = GET "/inner" {
      sample 200 { id: 1 }
    }
  }
  after = GET "/after/{{inner.body.id}}" {
    sample 200 {}
  }
}
"#,
        );
        let deletion = preview(&scenario, &["x"]);
        assert_eq!(
            deletion.refusal().as_deref(),
            Some("`after` reads `inner`. Change or delete it first. `inner` would go with `x`.")
        );
    }

    #[test]
    fn delete_preview_an_if_with_empty_braces_asks_nothing() {
        let (project, scenario) = small(
            r#"scenario "Empty" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  x = if a.status == 200 {
  }
  b = GET "/b" {
    sample 200 {}
  }
}
"#,
        );
        let deletion = preview(&scenario, &["x"]);
        assert_eq!((deletion.refusal(), deletion.question()), (None, None));
        let after = deleted(&project, &scenario, &["x"]);
        assert_eq!(steps(&after), ["begin", "a", "b"]);
    }

    // Criterion 6 -------------------------------------------------------------

    #[test]
    fn delete_preview_start_with_steps_is_refused_alone_it_goes() {
        let (_, scenario) = small(PAY);
        assert_eq!(
            preview(&scenario, &["begin"]).refusal().as_deref(),
            Some("A scenario starts at Start. Delete its steps first.")
        );
        let (project, scenario) = small("scenario \"Alone\" {\n  begin = start\n}\n");
        assert_eq!(preview(&scenario, &["begin"]).refusal(), None);
        let after = deleted(&project, &scenario, &["begin"]);
        assert!(after.graph.nodes.is_empty());
    }
}
