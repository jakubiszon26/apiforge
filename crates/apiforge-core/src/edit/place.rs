//! Putting a step at a place on the board (DIAG-06a): the one operation the
//! folder window moves a step with, and that later edits add cases to.
//!
//! A place is directly after a step `a` — after `a`, the statements of its
//! braces and the checks and values written under it, in the braces `a`
//! stands in — or first in the braces of a branch of an `if`. The step goes
//! there with the checks, values and variables written directly under it
//! that read it and no other step; its old neighbours join. The result is a
//! graph whose nodes are in the new written order and whose ordering edges
//! are one chain per braces ([`crate::chain::chain`]).
//!
//! A move is refused, naming the reader, what it reads and the step it stays
//! below, exactly when the result has a cycle through ordering and values.
//! Where a check or a value is written is no reason: data statements stand
//! anywhere in their block. Until DIAG-06d a step moves only inside its own
//! braces, and an `if` with statements in its braces does not move.
use super::EditError;
use crate::{
    format::{Graph, Project},
    model::Branch,
};

/// Where a step is put.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place<'a> {
    /// Directly after the step `a`, its braces and the checks and values
    /// written under it, in the braces `a` stands in: what a wire from
    /// `a`'s *Next* says.
    After(&'a str),
    /// First in the braces of the branch of an `if`: what a wire from its
    /// *Yes* or *No* says.
    First(&'a str, Branch),
}

/// `graph` with the step `step` put at `at`, with the statements that move
/// with it, or the refusal the board shows. The node identifiers of what
/// moved, the step first, are returned beside the graph, so that the text
/// moves those statements' lines and no others.
pub fn place(
    graph: &Graph,
    project: &Project,
    step: &str,
    at: Place<'_>,
) -> Result<(Graph, Vec<String>), EditError> {
    let _ = (graph, project, step, at);
    Err(EditError::NotMovableYet)
}

/// The step *Add step* on the board's bar puts a new step after: with a
/// step selected, that step; with an `if`, the whole `if`; with a check or
/// value, the step it is written under; with several, the one that runs
/// last. `None` with nothing selected, or when no selected statement is
/// written under a step: the new step goes last in the main flow.
pub fn add_step_after<'g>(graph: &'g Graph, selection: &[&str]) -> Option<&'g str> {
    let _ = (graph, selection);
    None
}

#[cfg(test)]
mod tests {
    //! DIAG-06a, criteria 1, 3, 4 and 8 on the operation. The documents are
    //! written in the language and lowered by `apiforge-script`, which links
    //! another build of this crate: they cross between the two builds as
    //! JSON. Every edit ends by printing the edited document, reading it back
    //! without an error and as the same document.
    use super::*;
    use crate::{
        chain::is_step,
        format::{Component, Node, Scenario},
        graph::sequence,
    };
    use serde::{Serialize, de::DeserializeOwned};

    /// A value of one build of this crate as the same type of the other.
    fn crossed<A: Serialize, B: DeserializeOwned>(value: &A) -> B {
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap()
    }

    fn lowered(project: &Project, text: &str) -> apiforge_script::Document {
        let theirs = crossed(project);
        let lowered = apiforge_script::lower(
            &apiforge_script::parse(text),
            &apiforge_script::LowerContext::folder(&theirs),
        );
        assert!(!lowered.has_errors(), "{:#?}", lowered.diagnostics);
        lowered.document.expect("a document")
    }

    fn scenario(project: &Project, text: &str) -> Scenario {
        match lowered(project, text) {
            apiforge_script::Document::Scenario(scenario) => crossed(&scenario),
            other => panic!("not a scenario: {other:?}"),
        }
    }

    fn component(project: &Project, text: &str) -> Component {
        match lowered(project, text) {
            apiforge_script::Document::Component(component) => crossed(&component),
            other => panic!("not a component: {other:?}"),
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
        let scenario = scenario(&project, &fixture("shop-30.afs"));
        (project, scenario)
    }

    fn small(text: &str) -> (Project, Scenario) {
        let project = super::super::test_support::empty_project();
        let scenario = scenario(&project, text);
        (project, scenario)
    }

    fn node<'g>(graph: &'g Graph, handle: &str) -> &'g Node {
        graph
            .nodes
            .iter()
            .find(|node| node.handle == handle)
            .unwrap_or_else(|| panic!("a statement `{handle}`"))
    }

    fn id(graph: &Graph, handle: &str) -> String {
        node(graph, handle).id.clone()
    }

    fn handles(graph: &Graph, ids: &[String]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
            .collect()
    }

    /// The steps in the order they run.
    fn steps(graph: &Graph) -> Vec<String> {
        sequence(graph)
            .into_iter()
            .filter_map(|id| graph.nodes.iter().find(|node| node.id == id))
            .filter(|node| is_step(node.kind))
            .map(|node| node.handle.clone())
            .collect()
    }

    /// Every statement in written order.
    fn written(graph: &Graph) -> Vec<&str> {
        graph
            .nodes
            .iter()
            .map(|node| node.handle.as_str())
            .collect()
    }

    /// The document printed reads back without an error and as itself.
    fn reads_back(project: &Project, document: apiforge_script::Document) {
        let theirs = crossed(project);
        let text = apiforge_script::print_folder_document(&theirs, &document)
            .expect("the edited document prints");
        let read = lowered(project, &text);
        assert_eq!(
            apiforge_script::semantic(&read, &theirs),
            apiforge_script::semantic(&document, &theirs),
            "the text reads back as the document:\n{text}"
        );
    }

    fn after(graph: &Graph, handle: &str) -> Place<'static> {
        Place::After(Box::leak(id(graph, handle).into_boxed_str()))
    }

    /// `step` of the main flow put at `at`, checked by printing.
    fn moved(project: &Project, scenario: &Scenario, step: &str, at: Place<'_>) -> Scenario {
        let (graph, _) = place(&scenario.graph, project, &id(&scenario.graph, step), at)
            .unwrap_or_else(|error| panic!("`{step}` moves: {error}"));
        let mut result = scenario.clone();
        result.graph = graph;
        reads_back(
            project,
            apiforge_script::Document::Scenario(crossed(&result)),
        );
        result
    }

    fn refused(graph: &Graph, project: &Project, step: &str, at: Place<'_>) -> String {
        match place(graph, project, &id(graph, step), at) {
            Ok(_) => panic!("moving `{step}` is refused"),
            Err(error) => error.to_string(),
        }
    }

    // Criterion 1 -------------------------------------------------------------

    #[test]
    fn place_moves_a_step() {
        let (project, shop) = shop();
        // Forwards in the main flow: `address` runs directly after `cart`
        // and its checks; `cart_view`, which followed `cart`, follows it.
        let forwards = moved(&project, &shop, "address", after(&shop.graph, "cart"));
        let order = steps(&forwards.graph);
        let at = |step: &str| order.iter().position(|s| s == step).unwrap();
        assert_eq!(at("address"), at("cart") + 1, "{order:?}");
        assert_eq!(at("cart_view"), at("address") + 1, "{order:?}");
        assert_eq!(
            at("checkout"),
            at("cart_view") + 1,
            "the old neighbours join"
        );
        // Backwards: `cart_view` runs after `address`.
        let backwards = moved(&project, &shop, "cart_view", after(&shop.graph, "address"));
        let order = steps(&backwards.graph);
        let at = |step: &str| order.iter().position(|s| s == step).unwrap();
        assert_eq!(at("address"), at("cart") + 1, "the old neighbours join");
        assert_eq!(at("cart_view"), at("address") + 1, "{order:?}");
        assert_eq!(at("checkout"), at("cart_view") + 1, "{order:?}");
        // In a branch, both ways.
        let branch = moved(&project, &shop, "refund", after(&shop.graph, "payment"));
        assert_eq!(
            steps(&branch.graph)[10..],
            ["paid", "payment", "refund", "receipt", "logout"]
        );
        let branch = moved(&project, &shop, "refund", after(&shop.graph, "logout"));
        assert_eq!(
            steps(&branch.graph)[10..],
            ["paid", "payment", "receipt", "logout", "refund"]
        );
        // First in the braces of `paid`.
        let paid = id(&shop.graph, "paid");
        let first = moved(&project, &shop, "refund", Place::First(&paid, Branch::True));
        assert_eq!(
            steps(&first.graph)[10..],
            ["paid", "refund", "payment", "receipt", "logout"]
        );
    }

    const CLEANUP: &str = r#"scenario "Cleanup" {
  begin = start
  made = POST "/made" {
    sample 201 { id: 1 }
  }
  cleanup {
    a = DELETE "/a/{{made.body.id}}" {
      sample 204 {}
    }
    b = DELETE "/b" {
      sample 204 {}
    }
    b_gone = assert b.status == 204
    c = DELETE "/c" {
      sample 204 {}
    }
  }
}
"#;

    #[test]
    fn place_moves_a_step_in_the_cleanup() {
        let (project, scenario) = small(CLEANUP);
        let cleanup = &scenario.cleanup;
        for (step, anchor, expected) in [("c", "a", ["a", "c", "b"]), ("a", "b", ["b", "a", "c"])] {
            let (graph, _) = place(
                cleanup,
                &project,
                &id(cleanup, step),
                after(cleanup, anchor),
            )
            .unwrap_or_else(|error| panic!("`{step}` moves: {error}"));
            assert_eq!(steps(&graph), expected);
            let mut result = scenario.clone();
            result.cleanup = graph;
            reads_back(
                &project,
                apiforge_script::Document::Scenario(crossed(&result)),
            );
        }
    }

    const TRIO: &str = r#"component trio "Trio" {
  a = GET "/a" {
    sample 200 { id: 1 }
  }
  b = GET "/b" {
    sample 200 { id: 2 }
  }
  c = GET "/c" {
    sample 200 { id: 3 }
  }
  export id: number = a.body.id
}
"#;

    #[test]
    fn place_moves_a_step_in_a_component() {
        let project = super::super::test_support::empty_project();
        let trio = component(&project, TRIO);
        for (step, anchor, expected) in [("c", "a", ["a", "c", "b"]), ("a", "c", ["b", "c", "a"])] {
            let (graph, moved) = place(
                &trio.graph,
                &project,
                &id(&trio.graph, step),
                after(&trio.graph, anchor),
            )
            .unwrap_or_else(|error| panic!("`{step}` moves: {error}"));
            assert_eq!(steps(&graph), expected);
            assert_eq!(handles(&graph, &moved), [step]);
            let mut result = trio.clone();
            result.graph = graph;
            reads_back(
                &project,
                apiforge_script::Document::Component(crossed(&result)),
            );
        }
    }

    // Criterion 3 -------------------------------------------------------------

    #[test]
    fn a_step_moves_with_its_checks() {
        let (project, shop) = shop();
        // `cart_created` reads only `cart`; `cart_total` also reads
        // `expected_total`, a value of `promoted`, and stays where it is.
        let (graph, moved) = place(
            &shop.graph,
            &project,
            &id(&shop.graph, "cart"),
            after(&shop.graph, "catalogue"),
        )
        .expect("`cart` moves");
        assert_eq!(handles(&graph, &moved), ["cart", "cart_created"]);
        let order = written(&graph);
        let at = |handle: &str| order.iter().position(|h| *h == handle).unwrap();
        assert_eq!(
            at("cart"),
            at("second") + 1,
            "after `catalogue`'s statements"
        );
        assert_eq!(at("cart_created"), at("cart") + 1);
        assert_eq!(at("product"), at("cart_created") + 1);
        assert!(at("cart_total") > at("expected_total"), "{order:?}");
        let mut result = shop.clone();
        result.graph = graph;
        reads_back(
            &project,
            apiforge_script::Document::Scenario(crossed(&result)),
        );
    }

    #[test]
    fn a_step_moves_with_a_check_of_it_and_a_constant() {
        let (project, scenario) = small(
            r#"scenario "Constant" {
  begin = start
  limit = 30
  a = GET "/a" {
    sample 200 { n: 1 }
  }
  b = GET "/b" {
    sample 200 { n: 2 }
  }
  b_ok = assert b.status == 200
  b_fast = assert b.body.n < limit
  b_v = b.body.n
  b_both = assert b.body.n == a.body.n
}
"#,
        );
        let (graph, moved) = place(
            &scenario.graph,
            &project,
            &id(&scenario.graph, "b"),
            after(&scenario.graph, "begin"),
        )
        .expect("`b` moves");
        assert_eq!(handles(&graph, &moved), ["b", "b_ok", "b_fast", "b_v"]);
        // After *Start* and the value written under it.
        assert_eq!(
            written(&graph),
            [
                "begin", "limit", "b", "b_ok", "b_fast", "b_v", "a", "b_both"
            ]
        );
    }

    // Criterion 4 -------------------------------------------------------------

    const PAY: &str = r#"scenario "Pay" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1", total: 10 }
  }
  pay = POST "/pay/{{cart.body.id}}" {
    sample 200 { ok: true }
  }
  done = GET "/done" {
    sample 200 {}
  }
}
"#;

    #[test]
    fn place_refuses_the_step_above_what_it_reads() {
        let (project, scenario) = small(PAY);
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "pay", after(graph, "begin")),
            "`pay` reads `cart.body.id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_a_step_below_a_step_that_reads_it() {
        let (project, scenario) = small(PAY);
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "cart", after(graph, "done")),
            "`pay` reads `cart.body.id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_through_a_value_between_them() {
        let (project, scenario) = small(
            r#"scenario "Value" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1" }
  }
  pay = POST "/pay/{{cart_id}}" {
    sample 200 { ok: true }
  }
  cart_id = cart.body.id
}
"#,
        );
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "pay", after(graph, "begin")),
            "`pay` reads `cart_id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_through_a_variable_between_them() {
        let (project, scenario) = small(
            r#"scenario "Variable" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1" }
  }
  done = GET "/done" {
    sample 200 {}
  }
  var cart_id = cart.body.id
  pay = POST "/pay/{{cart_id}}" {
    sample 200 { ok: true }
  }
}
"#,
        );
        let graph = &scenario.graph;
        assert_eq!(
            refused(graph, &project, "cart", after(graph, "pay")),
            "`pay` reads `cart_id`; it stays below `cart`."
        );
    }

    #[test]
    fn place_refuses_through_a_statement_that_travels() {
        let (project, scenario) = small(
            r#"scenario "Travels" {
  begin = start
  cart = POST "/cart" {
    sample 201 { id: "c-1" }
  }
  cart_id = cart.body.id
  done = GET "/done" {
    sample 200 {}
  }
  pay = POST "/pay/{{cart_id}}" {
    sample 200 { ok: true }
  }
}
"#,
        );
        let graph = &scenario.graph;
        // `cart_id` goes with `cart` below `pay`, which reads it.
        assert_eq!(
            refused(graph, &project, "cart", after(graph, "pay")),
            "`pay` reads `cart_id`; it stays below `cart`."
        );
        // Below `done`, which reads nothing, it moves.
        let (moved, _) = place(graph, &project, &id(graph, "cart"), after(graph, "done"))
            .expect("`cart` moves below `done`");
        assert_eq!(written(&moved), ["begin", "done", "cart", "cart_id", "pay"]);
    }

    #[test]
    fn place_allows_a_data_statement_written_above() {
        let (project, scenario) = small(
            r#"scenario "Above" {
  begin = start
  ok = assert b.status == 200
  v = a.body.id
  a = GET "/a" {
    sample 200 { id: 1 }
  }
  c = GET "/c" {
    sample 200 {}
  }
  b = POST "/b/{{v}}" {
    sample 200 {}
  }
}
"#,
        );
        let after_a = moved(&project, &scenario, "b", after(&scenario.graph, "a"));
        assert_eq!(steps(&after_a.graph), ["begin", "a", "b", "c"]);
        let first = moved(&project, &scenario, "c", after(&scenario.graph, "begin"));
        assert_eq!(steps(&first.graph), ["begin", "c", "a", "b"]);
    }

    // Criterion 8 -------------------------------------------------------------

    const NOT_YET: &str =
        "Moving a condition with its steps, or a step into another branch, is not possible yet.";

    #[test]
    fn place_refuses_an_if_with_statements_in_its_braces() {
        let (project, shop) = shop();
        let graph = &shop.graph;
        assert_eq!(
            refused(graph, &project, "paid", after(graph, "login")),
            NOT_YET
        );
    }

    #[test]
    fn place_refuses_a_step_of_other_braces() {
        let (project, shop) = shop();
        let graph = &shop.graph;
        assert_eq!(
            refused(graph, &project, "payment", after(graph, "cart")),
            NOT_YET
        );
        assert_eq!(
            refused(graph, &project, "address", after(graph, "payment")),
            NOT_YET
        );
        let paid = id(graph, "paid");
        assert_eq!(
            refused(
                graph,
                &project,
                "address",
                Place::First(&paid, Branch::False)
            ),
            NOT_YET
        );
    }

    #[test]
    fn place_moves_an_if_with_empty_braces() {
        let (project, scenario) = small(
            r#"scenario "Empty" {
  begin = start
  a = GET "/a" {
    sample 200 {}
  }
  b = GET "/b" {
    sample 200 {}
  }
  x = if 1 == 1 {
  }
}
"#,
        );
        let moved = moved(&project, &scenario, "x", after(&scenario.graph, "a"));
        assert_eq!(steps(&moved.graph), ["begin", "a", "x", "b"]);
    }

    // Criterion 5 -------------------------------------------------------------

    #[test]
    fn add_step_after_the_selection() {
        let (_, shop) = shop();
        let graph = &shop.graph;
        let pick = |selected: &[&str]| -> Option<String> {
            let ids: Vec<String> = selected.iter().map(|handle| id(graph, handle)).collect();
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            add_step_after(graph, &ids).map(|id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .unwrap()
                    .handle
                    .clone()
            })
        };
        assert_eq!(pick(&[]), None);
        assert_eq!(pick(&["cart"]).as_deref(), Some("cart"));
        assert_eq!(pick(&["paid"]).as_deref(), Some("paid"));
        assert_eq!(pick(&["cart_total"]).as_deref(), Some("cart"));
        assert_eq!(pick(&["receipt_total"]).as_deref(), Some("receipt"));
        assert_eq!(
            pick(&["checkout", "login", "cart"]).as_deref(),
            Some("checkout")
        );
        assert_eq!(pick(&["refund", "order"]).as_deref(), Some("refund"));
    }
}
