//! #565: an edit of the board the window refuses says why, in the notice
//! under the tabs, and leaves the document as it was. The session's side is
//! `session::tests` of `apiforge-project`.
use super::diag06a::{graph, id_of, shop_with};
use super::*;
use crate::canvas::Command;
use crate::testing::MINIMUM;

const SCENARIO: &str = r#"scenario "Pay for an order" {
  begin = start
  order = GET "/orders/1" {
    sample 200 { id: "o-1" }
  }
  pay = POST "/payments"
  a_check_whose_name_is_as_long_as_a_step_s_name_is_allowed_to_be_ = condition all {
    order.status == 200,
    order.body.id == "o-1",
  }
}
"#;

const DUPLICATE: &str = "This edit was not applied: duplicate handle. Nothing was changed.";
const START: &str =
    "This edit was not applied: Start cannot have inputs (step \"begin\"). Nothing was changed.";
const NO_URL: &str =
    "This edit was not applied: a request needs a URL (step \"pay\"). Nothing was changed.";
/// The longest reason of the model check, with the longest name of a step.
const LONGEST: &str = "This edit was not applied: data condition inputs must be value, actual<n> and expected<n> of its checks (step \"a_check_whose_name_is_as_long_as_a_step_s_name_is_allowed_to_be_\"). Nothing was changed.";

/// The notice's sentence as the window holds it.
fn notice(harness: &FolderHarness) -> Option<String> {
    let notice = folder(harness).notice()?;
    assert!(notice.error, "a refusal is an error");
    Some(notice.text.clone())
}

/// A value written on an input the step cannot have, as the board writes it.
fn set_input(harness: &mut FolderHarness, id: &str, handle: &str) {
    let node = id_of(&graph(harness, id), handle);
    app(harness).active_board_command(Command::SetLiteral {
        node,
        input: "x".to_owned(),
        path: Vec::new(),
        value: "1".into(),
    });
    settle(harness);
    harness.step();
}

/// A changed graph of the main board, by the call every edit of
/// `active_board_command` ends with: the board itself offers no duplicate
/// handle since DIAG-17.
fn edit(harness: &mut FolderHarness, id: &str, change: impl FnOnce(&mut Graph)) {
    let model = folder(harness).snapshot(id).unwrap().model.unwrap();
    let mut changed = graph(harness, id);
    change(&mut changed);
    app(harness).apply_edit(id, &model, false, Ok(changed.into()));
    settle(harness);
    harness.step();
}

#[test]
fn issue565_a_refused_edit_says_why() {
    let (_project, mut harness, id) = shop_with(SCENARIO);
    let before = graph(&harness, &id);
    let unchanged = |harness: &FolderHarness| {
        assert_eq!(source_of(harness, &id), SCENARIO);
        assert_eq!(graph(harness, &id), before);
    };

    edit(&mut harness, &id, |graph| {
        graph
            .nodes
            .iter_mut()
            .find(|node| node.handle == "pay")
            .unwrap()
            .handle = "order".to_owned();
    });
    assert_eq!(notice(&harness).as_deref(), Some(DUPLICATE));
    assert!(has_label(&harness, DUPLICATE), "the banner is drawn");
    unchanged(&harness);

    edit(&mut harness, &id, |graph| {
        graph
            .nodes
            .iter_mut()
            .find(|node| node.handle == "pay")
            .unwrap()
            .inputs
            .clear();
    });
    assert_eq!(notice(&harness).as_deref(), Some(NO_URL));
    assert!(has_label(&harness, NO_URL), "the banner is drawn");
    unchanged(&harness);

    set_input(&mut harness, &id, "begin");
    assert_eq!(notice(&harness).as_deref(), Some(START));
    assert!(has_label(&harness, START), "the banner is drawn");
    unchanged(&harness);
}

/// Criterion 6: the sentence wraps, whole and beside *Dismiss*.
#[test]
fn issue565_the_longest_refusal_fits() {
    let (_project, mut harness, id) = shop_with(SCENARIO);
    for size in [MINIMUM, WIDE] {
        harness.set_size(size);
        settle(&mut harness);
        set_input(
            &mut harness,
            &id,
            "a_check_whose_name_is_as_long_as_a_step_s_name_is_allowed_to_be_",
        );
        assert_eq!(notice(&harness).as_deref(), Some(LONGEST));
        crate::testing::assert_within_window(&harness, &[LONGEST, "Dismiss"]);
        crate::testing::assert_disjoint(&harness, &[LONGEST, "Split"]);
    }
}

/// The notice with the shortest and the longest reason, at 960 × 540 and
/// 1600 × 1000.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_issue565_refused_edit() {
    for (size, suffix) in [(MINIMUM, "960x540"), (WIDE, "1600x1000")] {
        let (_project, mut harness, id) = shop_with(SCENARIO);
        harness.set_size(size);
        settle(&mut harness);
        edit(&mut harness, &id, |graph| {
            graph.nodes[2].handle = graph.nodes[1].handle.clone();
        });
        assert_eq!(notice(&harness).as_deref(), Some(DUPLICATE));
        crate::testing::capture(&mut harness, &format!("issue565-short-{suffix}"), None);
        set_input(
            &mut harness,
            &id,
            "a_check_whose_name_is_as_long_as_a_step_s_name_is_allowed_to_be_",
        );
        assert_eq!(notice(&harness).as_deref(), Some(LONGEST));
        crate::testing::capture(&mut harness, &format!("issue565-long-{suffix}"), None);
    }
}
