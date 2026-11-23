//! #556 reproduction: a data wire dropped on a card but not on one of its
//! inputs is refused with *Drop the wire on an input port.* in the board's
//! banner, and the banner stays through later edits that succeed. The
//! canvas clears its banner only on *Dismiss* or when the board's context
//! changes; nothing clears it when an edit goes through.
use super::diag06a::{graph, id_of, shop_with, show};
use super::diag07::board;
use super::lay16::{centre, move_to, press, release};
use super::*;
use crate::canvas::Command;
use apiforge_core::{block::Block, format::Position};

const SCENARIO: &str = r#"scenario "Wires" {
  begin = start
  first = GET "/first" {
    sample 200 { id: "f-1" }
  }
  second = GET "/second/{{first.body.id}}" {
    sample 200 {}
  }
}
"#;

const REFUSED: &str = "Drop the wire on an input port.";

#[test]
#[ignore = "#556: reproduction, a refused wire's banner outlives later successful edits"]
fn issue556_the_refusal_goes_once_an_edit_succeeds() {
    let (_project, mut harness, id) = shop_with(SCENARIO);
    show(&mut harness, &id, &["first", "second"]);
    // A data wire from `first`'s body dropped on `second`'s header, not on
    // an input: refused.
    let second = id_of(&graph(&harness, &id), "second");
    let from = centre(&harness, "Output body of first");
    let to = board(&harness, &id)
        .debug_header(&second)
        .expect("second's header is drawn")
        .center();
    press(&mut harness, from);
    move_to(&mut harness, from, to);
    release(&mut harness, to);
    harness.step();
    assert!(has_label(&harness, REFUSED), "the wire is refused: {REFUSED}");

    // Two edits that go through: a request added from the Library, twice.
    for round in 1..=2 {
        let before = graph(&harness, &id).nodes.len();
        app(&mut harness).active_board_command(Command::AddNode {
            block: Block::Request,
            component: None,
            position: Position {
                x: 40.0,
                y: 300.0 * f64::from(round),
            },
            source: None,
        });
        settle(&mut harness);
        harness.step();
        assert_eq!(
            graph(&harness, &id).nodes.len(),
            before + 1,
            "the edit goes through"
        );
    }
    assert!(
        !has_label(&harness, REFUSED),
        "the refusal of the earlier wire is still shown after two successful edits"
    );
}
