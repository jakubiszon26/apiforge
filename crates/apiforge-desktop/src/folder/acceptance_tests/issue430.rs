//! #430: `@after(<if>)` (LANG-14, #390) in a folder window. The example of
//! the language reference, § Branches, opened as the demo shop's scenario,
//! is valid, prints back with `@after(created)` outside the braces, and its
//! simulation runs `logout` after the last step of the branch taken.
use super::*;
use crate::testing::{MINIMUM, folder_harness_in};
use apiforge_core::run::{Phase, StepStatus};

const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
/// The example of `docs/language/reference.md` § Branches.
const SOURCE: &str = r#"scenario "Branches" {
  param quantity = 2
  cart = POST "/carts" {
    body { productId: 123, quantity: param.quantity }
    sample 201 { id: "c-1", total: 160 }
  }
  created = if cart.status == 201 {
    receipt = GET "/carts/{{cart.body.id}}" {
      sample 200 { id: "c-1" }
    }
    @after(receipt)
    items = GET "/carts/{{cart.body.id}}/items" {
      sample 200 { items: [] }
    }
  } else {
    failed = assert cart.status == 201
  }
  @after(created)
  logout = POST "/logout" {
    sample 204 {}
  }
}
"#;

#[test]
fn issue430_the_reference_example_is_valid_and_simulates_in_a_folder_window() {
    let project = crate::testing::shop_project();
    fs::write(project.path().join(SCENARIO), SOURCE).unwrap();
    let mut harness = folder_harness_in(project.path(), MINIMUM, Mode::Diagram);
    app(&mut harness).review_open(SCENARIO, None);
    settle(&mut harness);
    let id = only(&harness, DocumentKind::Scenario);

    let session = folder(&harness).session.as_ref().unwrap();
    let document = session.document(&id).unwrap();
    assert!(
        !document
            .diagnostics()
            .iter()
            .any(|d| d.severity == apiforge_script::Severity::Error),
        "{:?}",
        document.diagnostics()
    );
    let model = document.model().expect("a valid model").clone();
    let written = apiforge_script::print_folder_document(session.context(), &model)
        .expect("the example prints");
    let line = |text: &str| written.lines().position(|line| line.trim() == text);
    let after = line("@after(created)").unwrap_or_else(|| panic!("{written}"));
    assert!(line("} else {") < Some(after), "{written}");
    assert_eq!(
        written.lines().nth(after + 1).map(str::trim),
        Some("logout = POST \"/logout\" {"),
        "{written}"
    );

    choose(&mut harness, "Run simulation");
    let report = finish_run(&mut harness);
    let graph = graph_of(&model, false);
    let handle = |node: &str| {
        graph
            .nodes
            .iter()
            .find(|candidate| candidate.id == node)
            .map_or("?", |candidate| candidate.handle.as_str())
    };
    let main: Vec<(&str, StepStatus)> = report
        .steps
        .iter()
        .filter(|step| step.phase == Phase::Main)
        .map(|step| (handle(&step.node_id), step.status))
        .collect();
    let position = |name: &str| main.iter().position(|(handle, _)| *handle == name);
    assert!(position("items").is_some(), "{main:?}");
    assert!(position("logout") > position("items"), "{main:?}");
    for (handle, status) in &main {
        let expected = if *handle == "failed" {
            StepStatus::Skipped
        } else {
            StepStatus::Passed
        };
        assert_eq!(*status, expected, "{handle}: {main:?}");
    }
}
