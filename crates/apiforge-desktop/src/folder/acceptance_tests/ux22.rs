//! UX-22 (#230, from #166; owner decision 13 = A of 2026-11-02): a
//! component is not offered on its own board, neither under PROJECT
//! COMPONENTS in the block library nor in *+ Add step*, so it cannot call
//! itself; every other component still is. *New component* in the Explorer
//! creates in `components/` and *New scenario* in `scenarios/`, whatever row
//! is selected.
use super::ux31::{by_path, window};
use super::*;
use crate::folder::tests::project_interaction_tests::reveal;
use crate::testing::{MINIMUM, capture};

const CHECKOUT: &str = "components/checkout-flow.afs";
const CHECKOUT_NAME: &str = "Checkout flow";
/// Another component of the demo shop.
const OTHER: &str = "Add to cart";
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";
const LIBRARY_SEARCH: &str = "Search blocks";
const POPUP_SEARCH: &str = "Find an action";

/// The demo shop with *CheckoutFlow* written to `checkout-flow.afs` and
/// published once, as revision 1.
fn shop_with_checkout() -> tempfile::TempDir {
    let project = crate::testing::shop_project();
    let mut session = apiforge_project::ProjectSession::open(project.path()).unwrap();
    let id = session
        .create_document(
            DocumentKind::Component,
            CHECKOUT,
            "component CheckoutFlow \"Checkout flow\" {\n}\n",
        )
        .unwrap();
    session.settle(&id).unwrap();
    let published = session
        .publish(&id, apiforge_project::Origin::SESSION)
        .unwrap();
    assert_eq!(published.revision, 1);
    drop(session);
    project
}

/// Whether an item named `name` is listed under the search field `search`:
/// a node of that name in the field's column, below it.
fn listed(harness: &FolderHarness, search: &str, name: &str) -> bool {
    let field = harness.get_by_label(search).rect();
    harness.query_all_by_label(name).any(|node| {
        let rect = node.rect();
        rect.top() >= field.bottom()
            && rect.left() >= field.left() - 8.0
            && rect.right() <= field.right() + 8.0
    })
}

/// Narrows the list under `search` to `text`, so the item is in view.
fn search(harness: &mut FolderHarness, field: &str, text: &str) {
    replace_text(harness, field, text);
}

/// Opens *+ Add step* from the board's toolbar.
fn open_add_step(harness: &mut FolderHarness) {
    choose(harness, "Add step");
    assert!(
        has_label(harness, POPUP_SEARCH),
        "*Add step* opens the popup"
    );
}

/// Criteria 1 and 2 in the library panel: on `checkout-flow.afs` after
/// publishing *CheckoutFlow*, PROJECT COMPONENTS does not list it and still
/// lists another component; on the scenario's board it lists both.
#[test]
fn ux22_criteria_1_2_the_library_leaves_out_the_component_being_edited() {
    let project = shop_with_checkout();
    let (mut harness, _) = window(project.path(), WIDE, CHECKOUT, "Diagram");
    assert!(
        has_label(&harness, "PROJECT COMPONENTS"),
        "the panel is shown"
    );
    search(&mut harness, LIBRARY_SEARCH, CHECKOUT_NAME);
    assert!(
        !listed(&harness, LIBRARY_SEARCH, CHECKOUT_NAME),
        "CheckoutFlow is not offered inside itself"
    );
    search(&mut harness, LIBRARY_SEARCH, OTHER);
    assert!(
        listed(&harness, LIBRARY_SEARCH, OTHER),
        "another component is still offered"
    );

    let scenario = by_path(&harness, SCENARIO);
    app(&mut harness).activate_tab(&scenario);
    settle(&mut harness);
    search(&mut harness, LIBRARY_SEARCH, CHECKOUT_NAME);
    assert!(
        listed(&harness, LIBRARY_SEARCH, CHECKOUT_NAME),
        "a scenario's board offers CheckoutFlow"
    );
}

/// Criteria 1 and 2 in *+ Add step*, at the minimum window, where Split has
/// no library panel beside the board.
#[test]
fn ux22_criteria_1_2_add_step_leaves_out_the_component_being_edited() {
    let project = shop_with_checkout();
    let (mut harness, _) = window(project.path(), MINIMUM, CHECKOUT, "Split");
    assert!(
        !has_label(&harness, LIBRARY_SEARCH),
        "no library panel in Split at 960 × 540"
    );
    open_add_step(&mut harness);
    search(&mut harness, POPUP_SEARCH, CHECKOUT_NAME);
    assert!(
        !listed(&harness, POPUP_SEARCH, CHECKOUT_NAME),
        "*Add step* does not offer CheckoutFlow inside itself"
    );
    assert!(
        has_label(&harness, "No matching steps"),
        "nothing else matches its name"
    );
    search(&mut harness, POPUP_SEARCH, OTHER);
    assert!(
        listed(&harness, POPUP_SEARCH, OTHER),
        "another component is still offered"
    );

    // The same window on another component's board offers CheckoutFlow.
    harness.key_press(egui::Key::Escape);
    settle(&mut harness);
    let cart = by_path(&harness, "components/add-to-cart.afs");
    app(&mut harness).activate_tab(&cart);
    settle(&mut harness);
    open_add_step(&mut harness);
    search(&mut harness, POPUP_SEARCH, CHECKOUT_NAME);
    assert!(
        listed(&harness, POPUP_SEARCH, CHECKOUT_NAME),
        "another component's board offers CheckoutFlow"
    );
    search(&mut harness, POPUP_SEARCH, OTHER);
    assert!(
        !listed(&harness, POPUP_SEARCH, OTHER),
        "nor is *Add to cart* offered inside itself"
    );
}

/// Clicks the Explorer row named `name`.
fn click_row(harness: &mut FolderHarness, name: &str) {
    let tree = harness.get_by_label("Project files").rect();
    harness
        .query_all_by_label(name)
        .find(|node| tree.contains(node.rect().center()))
        .unwrap_or_else(|| panic!("no explorer row {name:?}"))
        .click();
    settle(harness);
}

/// Creates a document named `name` with the Explorer's *+ → `item`*.
fn create(harness: &mut FolderHarness, item: &str, name: &str) {
    choose(harness, "New");
    choose(harness, item);
    // The field's caption is also named *Name*.
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .type_text(name);
    settle(harness);
    choose(harness, "Create");
    assert!(
        folder(harness).prompt.is_none(),
        "{item}: the prompt closes after creating {name}"
    );
}

fn paths(harness: &FolderHarness) -> Vec<String> {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .map(|document| document.entry().path.as_str().to_owned())
        .collect()
}

/// Asserts that `path` is registered and on disk, and `not` is not registered.
fn assert_created(harness: &FolderHarness, root: &std::path::Path, path: &str, not: &str) {
    let paths = paths(harness);
    assert!(
        paths.iter().any(|known| known == path),
        "{path} in {paths:?}"
    );
    assert!(root.join(path).is_file(), "{path} is written");
    assert!(!paths.iter().any(|known| known == not), "no {not}");
}

/// Criterion 3: with a scenario's row selected, *+ → New component* creates
/// `components/<name>.afs` and *New scenario* `scenarios/<name>.afs`; with a
/// component's row or the `components` folder selected, *New scenario*
/// still creates in `scenarios/`.
#[test]
fn ux22_criterion_3_new_documents_go_into_their_kind_s_folder() {
    let project = crate::testing::shop_project();
    let (mut harness, _) = window(project.path(), WIDE, SCENARIO, "Code");
    click_row(&mut harness, "promotion-and-cart-price.afs");
    assert_eq!(
        folder(&harness).explorer.selected.as_deref(),
        Some(SCENARIO)
    );

    create(&mut harness, "New component", "Checkout flow");
    assert_created(
        &harness,
        project.path(),
        "components/checkout-flow.afs",
        "scenarios/checkout-flow.afs",
    );

    click_row(&mut harness, "promotion-and-cart-price.afs");
    create(&mut harness, "New scenario", "Smoke test");
    assert_created(
        &harness,
        project.path(),
        "scenarios/smoke-test.afs",
        "components/smoke-test.afs",
    );

    click_row(&mut harness, "product.afs");
    assert_eq!(
        folder(&harness).explorer.selected.as_deref(),
        Some("components/product.afs")
    );
    create(&mut harness, "New scenario", "Second smoke test");
    assert_created(
        &harness,
        project.path(),
        "scenarios/second-smoke-test.afs",
        "components/second-smoke-test.afs",
    );

    click_row(&mut harness, "components");
    assert_eq!(
        folder(&harness).explorer.selected.as_deref(),
        Some("components")
    );
    create(&mut harness, "New scenario", "Third smoke test");
    assert_created(
        &harness,
        project.path(),
        "scenarios/third-smoke-test.afs",
        "components/third-smoke-test.afs",
    );
}

/// The two captures of the QA report, at 960 × 540: the component's own
/// *Add step* scrolled to PROJECT COMPONENTS, and the Explorer after *New
/// component* with a scenario selected.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_ux22() {
    let project = shop_with_checkout();
    let (mut harness, _) = window(project.path(), MINIMUM, CHECKOUT, "Split");
    open_add_step(&mut harness);
    reveal(&mut harness, OTHER);
    harness.step();
    capture(&mut harness, "ux22-own-add-step-960x540", None);

    let project = crate::testing::shop_project();
    let (mut harness, _) = window(project.path(), MINIMUM, SCENARIO, "Code");
    click_row(&mut harness, "promotion-and-cart-price.afs");
    create(&mut harness, "New component", "Checkout flow");
    harness.step();
    capture(&mut harness, "ux22-explorer-new-component-960x540", None);
}
