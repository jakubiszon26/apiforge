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

#[test]
fn diagnostics_beyond_the_limit_are_counted_by_the_parser_and_kept_by_lowering() {
    // 1 100 characters the lexer refuses after the file, and the parser's
    // error for text after the file: 1 101 errors. A free comment, which
    // filled the list before, gives no warning since LANG-06 (#143).
    let source = format!("scenario \"Notes\" {{\n}}\n{}", "$".repeat(1100));
    let tree = parse(&source);
    assert_eq!(MAX_DIAGNOSTICS, 1000);
    assert_eq!(tree.diagnostics.len(), 1000);
    assert!(
        tree.diagnostics
            .iter()
            .all(|d| d.severity == Severity::Error)
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
    // kept in place of the last warning, which is counted as omitted. A free
    // comment gives no warning since LANG-06 (#143): the warnings are 1 000
    // operands of the wrong type, two in each of 500 values, and the error is
    // the graph's limit of 200 blocks.
    let body: String = (0..500)
        .map(|index| format!("  wrong_{index} = \"a\" - \"b\"\n"))
        .collect();
    let source = format!("scenario \"Notes\" {{\n{body}}}\n");
    let tree = parse(&source);
    assert_eq!(tree.diagnostics.len(), 0);
    assert_eq!(tree.omitted_diagnostics, 0);

    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&tree, &LowerContext::folder(&project));
    assert!(lowered.has_errors(), "{:#?}", &lowered.diagnostics[..1]);
    assert_eq!(lowered.diagnostics.len(), 1000);
    assert_eq!(lowered.omitted_diagnostics, 1);
}

/// A project-level error found after a full list of errors takes the last
/// one's place and counts it (`push_error` with `keep`, as the session pushes
/// the kind check). The session reaches its project-level checks only on a
/// text without errors, which since LANG-06 (#143) cannot hold 1 000
/// warnings, so this branch is shown here.
#[test]
fn a_kept_error_replaces_the_last_error_of_a_full_list() {
    let mut diagnostics: Vec<_> = (0..MAX_DIAGNOSTICS)
        .map(|index| apiforge_script::Diagnostic::error(index..index + 1, "Filler"))
        .collect();
    let mut omitted = 101;
    let kept = apiforge_script::Diagnostic::error(0..0, "Project-level");
    apiforge_script::push_error(&mut diagnostics, &mut omitted, kept.clone(), false);
    assert_eq!(
        (diagnostics.len(), omitted),
        (1000, 102),
        "dropped without keep"
    );
    assert!(!diagnostics.contains(&kept));
    apiforge_script::push_error(&mut diagnostics, &mut omitted, kept.clone(), true);
    assert_eq!((diagnostics.len(), omitted), (1000, 103));
    assert_eq!(diagnostics.last(), Some(&kept));
}

// LANG-14 (#390) --------------------------------------------------------------

/// A scenario in canonical form: a chain inside the `then` braces, a branch
/// without a step, and a step after the whole `if`. In the written language
/// since LANG-13 (#386): the order of the lines is the chain.
const WHOLE_IF: &str = r#"scenario "Whole if" {
  on_error stop
  begin = start
  cart = POST "/carts" {
    sample 201 { total: 1 }
  }
  paid = if cart.body.total > 0 {
    payment = POST "/payments" {
      sample 201 { id: "p-1" }
    }
    receipt = GET "/receipts" {
      sample 200 {}
    }
  } else {
    unpaid = assert cart.status == 201
  }
  logout = POST "/logout" {
    sample 204 {}
  }
}
"#;

/// `WHOLE_IF` as 0.1.0 wrote it, every ordering an `@after`: the stored
/// spelling of a published revision since LANG-13.
const WHOLE_IF_STORED: &str = r#"scenario "Whole if" {
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
/// the braces outside them, after the branch and the first step. Since
/// LANG-13 only a published revision is read in this spelling.
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
  cart = POST "/carts" {
    sample 201 { total: 1 }
  }
  paid = if cart.body.total > 0 {
    payment = POST "/payments" {
      sample 201 { id: "p-1" }
    }
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
  logout = POST "/logout" {
    sample 204 {}
  }
}
"#;

/// The scenario lowered in the demo shop project, without a diagnostic.
fn lowered_scenario(source: &str) -> (Project, Scenario) {
    lowered_in(source, |project| LowerContext::folder(project))
}

/// The same in the stored spelling of 0.1.0, where `@after` is read: how a
/// published revision is read since LANG-13.
fn lowered_stored(source: &str) -> (Project, Scenario) {
    lowered_in(source, |project| LowerContext::folder(project).stored())
}

fn lowered_in(
    source: &str,
    context: impl for<'p> Fn(&'p Project) -> LowerContext<'p>,
) -> (Project, Scenario) {
    let project = apiforge_core::demos::shop_project();
    let lowered = lower(&parse(source), &context(&project));
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
        "scenario \"Empty\" {\n  begin = start\n  x = if true {\n    inside = 1\n  }\n  y = GET \"/y\"\n}\n",
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
    let (project, outside) = lowered_stored(SECOND_OUTSIDE);
    assert_eq!(follows(&outside, "receipt"), follows(&inside, "receipt"));
    assert_eq!(printed(&project, &outside), WHOLE_IF);
}

/// Criterion 4: `x.then` and `x.else` keep their meaning, and the long form
/// LANG-11 described orders the same as `@after(x)`, which the window prints.
#[test]
fn lang14_criterion_4_branches_keep_their_meaning() {
    let with_after = |after: &str| WHOLE_IF_STORED.replace("@after(paid)", after);
    let (_, scenario) = lowered_stored(&with_after("@after(paid.else)"));
    assert_eq!(follows(&scenario, "logout"), sources(&["paid.else"]));
    let (_, scenario) = lowered_stored(&with_after("@after(cart, paid.then)"));
    assert_eq!(
        follows(&scenario, "logout"),
        sources(&["cart", "paid.then"])
    );

    let (project, scenario) = lowered_stored(&with_after("@after(receipt, paid.else)"));
    assert_eq!(
        follows(&scenario, "logout"),
        sources(&["receipt", "paid.else"])
    );
    assert_eq!(printed(&project, &scenario), WHOLE_IF);
}

/// `@after(x)` on an `if` that a cycle holds, or that follows one, reports
/// the cycle, not an edge the text never wrote.
#[test]
fn lang14_after_an_if_in_or_after_a_cycle_reports_the_cycle() {
    let request =
        |handle: &str| format!("  {handle} = GET \"/{handle}\" {{\n    sample 200 {{}}\n  }}\n");
    let through_the_if = format!(
        "scenario \"Cycle\" {{\n  @after(b)\n  x = if true {{\n  }}\n  @after(x)\n{}}}\n",
        request("b")
    );
    let after_a_cycle = format!(
        "scenario \"Cycle\" {{\n  @after(d)\n{}  @after(c)\n{}  @after(c)\n  x = if true {{\n  }}\n  @after(x)\n{}}}\n",
        request("c"),
        request("d"),
        request("b")
    );
    let project = apiforge_core::demos::shop_project();
    for source in [through_the_if, after_a_cycle] {
        let lowered = lower(&parse(&source), &LowerContext::folder(&project).stored());
        let messages: Vec<_> = lowered
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(
            messages,
            ["Graph contains a data/control cycle"],
            "{source}"
        );
    }
}

/// `@after(x)` written inside `x`'s own braces, at any depth, stays an error,
/// as under LANG-11: a statement of the `if` cannot run after the whole `if`.
/// The message names the statement and the `if`.
#[test]
fn lang14_after_the_if_from_inside_its_braces_is_an_error() {
    let project = apiforge_core::demos::shop_project();
    for (source, statement, x) in [
        (
            "scenario \"Inside\" {\n  x = if true {\n    a = GET \"/a\" {\n      sample 200 {}\n    }\n    @after(x)\n    s = GET \"/s\" {\n      sample 200 {}\n    }\n  }\n}\n",
            "s",
            "x",
        ),
        (
            "scenario \"Inside\" {\n  x = if true {\n  } else {\n    y = if false {\n      @after(x)\n      t = GET \"/t\" {\n        sample 200 {}\n      }\n    }\n  }\n}\n",
            "t",
            "x",
        ),
    ] {
        let lowered = lower(&parse(source), &LowerContext::folder(&project).stored());
        let errors: Vec<_> = lowered
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "{source}\n{:#?}", lowered.diagnostics);
        assert_eq!(&source[errors[0].range.clone()], x, "{source}");
        let message = &errors[0].message;
        for part in [format!("`{statement}`"), format!("`{x}`")] {
            assert!(message.contains(&part), "{part} in {message:?}");
        }
    }
}
