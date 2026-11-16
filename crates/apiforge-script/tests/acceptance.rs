//! MCP-01 (#60), criterion 5: the analysis keeps at most `MAX_DIAGNOSTICS`
//! per document and counts what it drops, so nothing is cut silently.
use apiforge_core::{
    format::{Project, Scenario},
    model::Branch,
};
use apiforge_script::{
    Document, LowerContext, MAX_DIAGNOSTICS, Severity, format, lower, parse, print_folder_document,
};
use std::collections::BTreeSet;

fn with_notes(body: &str, notes: usize) -> String {
    let mut source = format!("scenario \"Notes\" {{\n{body}}}\n");
    for index in 0..notes {
        source.push_str(&format!("// note {index}\n"));
    }
    source
}

#[test]
fn diagnostics_beyond_the_limit_are_counted_by_the_parser_and_kept_by_lowering() {
    let source = with_notes("", 1101);
    let tree = parse(&source);
    assert_eq!(MAX_DIAGNOSTICS, 1000);
    assert_eq!(tree.diagnostics.len(), 1000);
    assert!(
        tree.diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
    assert_eq!(tree.omitted_diagnostics, 101);

    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&tree, &LowerContext::folder(&project));
    assert_eq!(lowered.diagnostics.len(), 1000);
    assert_eq!(lowered.omitted_diagnostics, 101);
}

#[test]
fn an_error_that_replaces_a_warning_at_the_limit_counts_the_warning() {
    // A full list of warnings, then an error found by lowering: the error is
    // kept in place of the last warning, which is counted as omitted.
    let source = with_notes("  export x: number = 1\n", 1000);
    let tree = parse(&source);
    assert_eq!(tree.diagnostics.len(), 1000);
    assert_eq!(tree.omitted_diagnostics, 0);

    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&tree, &LowerContext::folder(&project));
    assert!(lowered.has_errors(), "{:#?}", &lowered.diagnostics[..1]);
    assert_eq!(lowered.diagnostics.len(), 1000);
    assert_eq!(lowered.omitted_diagnostics, 1);
}

// LANG-14 (#390) --------------------------------------------------------------

/// A scenario in canonical form: a chain inside the `then` braces, a branch
/// without a step, and a step after the whole `if`.
const WHOLE_IF: &str = r#"scenario "Whole if" {
  on_error stop
  begin = start
  @after(begin)
  cart = POST "/carts" {
    sample 201 { total: 1 }
  }
  @after(cart)
  paid = if cart.body.total > 0 {
    payment = POST "/payments" {
      sample 201 { id: "p-1" }
    }
    @after(payment)
    receipt = GET "/receipts" {
      sample 200 {}
    }
  } else {
    unpaid = assert cart.status == 201
  }
  @after(paid)
  logout = POST "/logout" {
    sample 204 {}
  }
}
"#;

/// The window's form before LANG-14 of the same ordering: the second step of
/// the braces outside them, after the branch and the first step.
const SECOND_OUTSIDE: &str = r#"scenario "Whole if" {
  on_error stop
  begin = start
  @after(begin)
  cart = POST "/carts" {
    sample 201 { total: 1 }
  }
  @after(cart)
  paid = if cart.body.total > 0 {
    payment = POST "/payments" {
      sample 201 { id: "p-1" }
    }
  } else {
    unpaid = assert cart.status == 201
  }
  @after(paid.then, payment)
  receipt = GET "/receipts" {
    sample 200 {}
  }
  @after(paid)
  logout = POST "/logout" {
    sample 204 {}
  }
}
"#;

/// A step in each branch, and a nested `if` as the last step of `then`.
const NESTED: &str = r#"scenario "Nested" {
  on_error stop
  begin = start
  @after(begin)
  cart = POST "/carts" {
    sample 201 { total: 1 }
  }
  @after(cart)
  paid = if cart.body.total > 0 {
    payment = POST "/payments" {
      sample 201 { id: "p-1" }
    }
    @after(payment)
    mailed = if payment.status == 201 {
      mail = POST "/mail" {
        sample 202 {}
      }
    }
  } else {
    cancel = POST "/cancel" {
      sample 200 {}
    }
  }
  @after(paid)
  logout = POST "/logout" {
    sample 204 {}
  }
}
"#;

/// The scenario lowered in the demo shop project, without a diagnostic.
fn lowered_scenario(source: &str) -> (Project, Scenario) {
    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&parse(source), &LowerContext::folder(&project));
    assert!(lowered.diagnostics.is_empty(), "{:#?}", lowered.diagnostics);
    let Some(Document::Scenario(scenario)) = lowered.document else {
        panic!("not a scenario");
    };
    (project, scenario)
}

/// The ordering a statement follows, as `@after` spells each source.
fn follows(scenario: &Scenario, handle: &str) -> BTreeSet<String> {
    let handle_of = |id: &str| {
        scenario
            .graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.handle.clone())
            .unwrap_or_else(|| panic!("no node {id}"))
    };
    let target = scenario
        .graph
        .nodes
        .iter()
        .find(|node| node.handle == handle)
        .unwrap_or_else(|| panic!("no statement `{handle}`"));
    scenario
        .graph
        .edges
        .iter()
        .filter(|edge| edge.target == target.id)
        .map(|edge| {
            let source = handle_of(&edge.source);
            match edge.branch {
                None => source,
                Some(Branch::True) => format!("{source}.then"),
                Some(Branch::False) => format!("{source}.else"),
            }
        })
        .collect()
}

fn sources(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn printed(project: &Project, scenario: &Scenario) -> String {
    print_folder_document(project, &Document::Scenario(scenario.clone()))
        .unwrap_or_else(|errors| panic!("{errors:#?}"))
}

/// Criterion 1: `@after(x)` on an `if` follows the last step of each branch,
/// or the branch itself when it holds no step. LANG-11's refusal is gone.
#[test]
fn lang14_criterion_1_after_an_if_follows_the_end_of_each_branch() {
    let (_, scenario) = lowered_scenario(WHOLE_IF);
    assert_eq!(
        follows(&scenario, "logout"),
        sources(&["receipt", "paid.else"])
    );

    let (_, scenario) = lowered_scenario(NESTED);
    assert_eq!(
        follows(&scenario, "logout"),
        sources(&["mail", "mailed.else", "cancel"])
    );

    // Neither branch holds a step: the `if` is followed through both branches.
    let (_, scenario) = lowered_scenario(
        "scenario \"Empty\" {\n  x = if true {\n    inside = 1\n  }\n  @after(x)\n  y = 2\n}\n",
    );
    assert_eq!(follows(&scenario, "y"), sources(&["x.then", "x.else"]));
}

/// Criterion 3: the window prints a chain inside its braces and the short
/// form after the `if`; `apiforge fmt` keeps both; text -> window -> text is
/// unchanged.
#[test]
fn lang14_criterion_3_a_chain_in_braces_stays_inside_them() {
    for text in [WHOLE_IF, NESTED] {
        let (project, scenario) = lowered_scenario(text);
        assert_eq!(printed(&project, &scenario), text);
        assert_eq!(format(text).as_deref(), Ok(text));
    }

    // The same ordering written outside the braces is printed inside them.
    let (_, inside) = lowered_scenario(WHOLE_IF);
    let (project, outside) = lowered_scenario(SECOND_OUTSIDE);
    assert_eq!(follows(&outside, "receipt"), follows(&inside, "receipt"));
    assert_eq!(printed(&project, &outside), WHOLE_IF);
}

/// Criterion 4: `x.then` and `x.else` keep their meaning, and the long form
/// LANG-11 described orders the same as `@after(x)`, which the window prints.
#[test]
fn lang14_criterion_4_branches_keep_their_meaning() {
    let with_after = |after: &str| WHOLE_IF.replace("@after(paid)", after);
    let (_, scenario) = lowered_scenario(&with_after("@after(paid.else)"));
    assert_eq!(follows(&scenario, "logout"), sources(&["paid.else"]));
    let (_, scenario) = lowered_scenario(&with_after("@after(cart, paid.then)"));
    assert_eq!(
        follows(&scenario, "logout"),
        sources(&["cart", "paid.then"])
    );

    let (project, scenario) = lowered_scenario(&with_after("@after(receipt, paid.else)"));
    assert_eq!(
        follows(&scenario, "logout"),
        sources(&["receipt", "paid.else"])
    );
    assert_eq!(printed(&project, &scenario), WHOLE_IF);
}
