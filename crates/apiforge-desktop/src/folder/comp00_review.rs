//! COMP-00: the design review views of the component authoring mock-up
//! (#239). Throwaway branch `mock/comp-00`: the switches in
//! `crate::mock_comp00` change the real board, Inspector and header for these
//! views only.
use super::*;
use crate::mock_comp00::{self as mock, Adding, Flags, Inputs, Panel, Publish};
use crate::testing::{FolderHarness, MINIMUM, WIDE, capture, folder, folder_harness, settle};
use egui_kittest::kittest::Queryable;

const SIZES: [(egui::Vec2, &str); 2] = [(MINIMUM, "960x540"), (WIDE, "1600x1000")];

fn app(harness: &mut FolderHarness) -> &mut FolderApp {
    harness.state_mut().as_mut().unwrap()
}

fn click(harness: &mut FolderHarness, label: &str) {
    harness.get_by_label(label).click();
    harness.hover_at(egui::pos2(480.0, 3.0));
    settle(harness);
}

fn by_path(harness: &FolderHarness, path: &str) -> String {
    folder(harness)
        .session
        .as_ref()
        .unwrap()
        .documents()
        .iter()
        .find(|document| document.entry().path.as_str() == path)
        .unwrap_or_else(|| panic!("no {path}"))
        .id()
        .to_owned()
}

fn open(harness: &mut FolderHarness, path: &str) {
    let id = by_path(harness, path);
    app(harness).activate_tab(&id);
    settle(harness);
}

const CART: &str = "components/add-to-cart.afs";
const SCENARIO: &str = "scenarios/promotion-and-cart-price.afs";

/// The demo shop at `size` with `path` open in `mode`, under `flags`.
fn window(
    size: egui::Vec2,
    path: &str,
    mode: &str,
    flags: Flags,
) -> (tempfile::TempDir, FolderHarness) {
    mock::set(flags);
    mock::close_dialog();
    let project = crate::testing::shop_project();
    let mut harness = folder_harness(project.path(), size);
    open(&mut harness, path);
    click(&mut harness, mode);
    (project, harness)
}

fn shot(harness: &mut FolderHarness, name: &str) {
    settle(harness);
    capture(harness, name, None);
}

/// Replaces `from` with `to` in a document's source, as typing would.
fn edit(harness: &mut FolderHarness, path: &str, from: &str, to: &str) {
    let id = by_path(harness, path);
    let app = app(harness);
    let session = app.session.as_mut().unwrap();
    let text = session.document(&id).unwrap().text().to_owned();
    let edited = text.replacen(from, to, 1);
    assert_ne!(edited, text, "{from} must be in {path}");
    session.edit_source(&id, &edited, CODE).unwrap();
    app.analyser.typed(&id, Instant::now());
    settle(harness);
}

/// Q0: the component board as it is on `main`, for comparison.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_comp00_q1_inputs_on_the_board() {
    for (size, suffix) in SIZES {
        for (option, inputs) in [
            ("now", Inputs::Off),
            ("a-card", Inputs::Card),
            ("b-chips", Inputs::Chips),
            ("c-band", Inputs::Band),
        ] {
            for mode in ["Diagram", "Split"] {
                let (_project, mut harness) = window(
                    size,
                    CART,
                    mode,
                    Flags {
                        inputs,
                        ..Flags::default()
                    },
                );
                shot(
                    &mut harness,
                    &format!("comp00-q1-{option}-{}-{suffix}", mode.to_lowercase()),
                );
                // C has no wires: selecting a pill rings the steps that read it.
                if inputs == Inputs::Band && mode == "Diagram" {
                    mock::update(|flags| flags.selected_input = Some("quantity".to_owned()));
                    shot(
                        &mut harness,
                        &format!("comp00-q1-c-band-selected-diagram-{suffix}"),
                    );
                }
            }
        }
    }
}

/// Q2: *+ Add input*: the Inspector's form (A) or an inline row (B), and
/// the state right after adding, with the new `input` line in the code.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_comp00_q2_adding_an_input() {
    for (size, suffix) in SIZES {
        let (_project, mut harness) = window(
            size,
            CART,
            "Diagram",
            Flags {
                inputs: Inputs::Card,
                adding: Adding::InspectorForm,
                panel: Panel::NewInput,
                new_name: "currency".to_owned(),
                reveal_inputs: true,
                ..Flags::default()
            },
        );
        shot(&mut harness, &format!("comp00-q2-a-form-diagram-{suffix}"));
        mock::set(Flags {
            inputs: Inputs::Card,
            adding: Adding::InlineRow,
            new_name: "currency".to_owned(),
            reveal_inputs: true,
            ..Flags::default()
        });
        shot(
            &mut harness,
            &format!("comp00-q2-b-inline-diagram-{suffix}"),
        );
        // Added: the code has the new line, the row is selected and the
        // Inspector edits it.
        edit(
            &mut harness,
            CART,
            "  action product",
            "  @label(\"Currency\")\n  input currency: string = \"EUR\"\n  action product",
        );
        mock::set(Flags {
            inputs: Inputs::Card,
            panel: Panel::Input {
                name: "currency".to_owned(),
                draft_name: None,
            },
            selected_input: Some("currency".to_owned()),
            reveal_inputs: true,
            ..Flags::default()
        });
        for mode in ["Diagram", "Split"] {
            click(&mut harness, mode);
            shot(
                &mut harness,
                &format!("comp00-q2-added-{}-{suffix}", mode.to_lowercase()),
            );
        }
    }
}

/// Q3: editing an input in the Inspector; a duplicate name is refused inline
/// and the row stays selected.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_comp00_q3_editing_an_input() {
    for (size, suffix) in SIZES {
        let (_project, mut harness) = window(
            size,
            CART,
            "Diagram",
            Flags {
                inputs: Inputs::Card,
                panel: Panel::Input {
                    name: "quantity".to_owned(),
                    draft_name: None,
                },
                selected_input: Some("quantity".to_owned()),
                reveal_inputs: true,
                ..Flags::default()
            },
        );
        shot(&mut harness, &format!("comp00-q3-edit-diagram-{suffix}"));
        mock::update(|flags| {
            flags.panel = Panel::Input {
                name: "quantity".to_owned(),
                draft_name: Some("product".to_owned()),
            }
        });
        for mode in ["Diagram", "Split"] {
            click(&mut harness, mode);
            shot(
                &mut harness,
                &format!("comp00-q3-duplicate-{}-{suffix}", mode.to_lowercase()),
            );
        }
    }
}

/// Q4: the component's settings in the Inspector with nothing selected.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_comp00_q4_component_settings() {
    for (size, suffix) in SIZES {
        for mode in ["Diagram", "Split"] {
            let (_project, mut harness) = window(
                size,
                CART,
                mode,
                Flags {
                    inputs: Inputs::Card,
                    panel: Panel::Component,
                    publish: Publish::Latest(1),
                    ..Flags::default()
                },
            );
            shot(
                &mut harness,
                &format!("comp00-q4-settings-{}-{suffix}", mode.to_lowercase()),
            );
        }
    }
}

/// Q6: *Publish* with nothing to publish, beside the rest of the header.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_comp00_q6_publish() {
    for (size, suffix) in SIZES {
        let (_project, mut harness) = window(
            size,
            CART,
            "Diagram",
            Flags {
                inputs: Inputs::Card,
                publish: Publish::Latest(1),
                ..Flags::default()
            },
        );
        shot(&mut harness, &format!("comp00-q6-a-published-{suffix}"));
        let header = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(size.x, 60.0));
        capture(
            &mut harness,
            &format!("comp00-q6-a-published-header-{suffix}"),
            Some(header),
        );
        mock::update(|flags| flags.publish = Publish::Changed(1));
        settle(&mut harness);
        capture(
            &mut harness,
            &format!("comp00-q6-a-changed-header-{suffix}"),
            Some(header),
        );
        mock::update(|flags| flags.publish = Publish::LatestCaption(1));
        settle(&mut harness);
        capture(
            &mut harness,
            &format!("comp00-q6-b-caption-header-{suffix}"),
            Some(header),
        );
    }
}

/// The ids of the blocks with these handles on the scenario's main board.
fn scenario_graph(harness: &FolderHarness) -> apiforge_core::format::Graph {
    let id = by_path(harness, SCENARIO);
    let session = folder(harness).session.as_ref().unwrap();
    match session
        .document(&id)
        .unwrap()
        .shared_model()
        .unwrap()
        .as_ref()
    {
        Document::Scenario(scenario) => scenario.graph.clone(),
        Document::Component(_) => unreachable!("a scenario"),
    }
}

fn ids(graph: &apiforge_core::format::Graph, handles: &[&str]) -> Vec<String> {
    handles
        .iter()
        .map(|handle| {
            graph
                .nodes
                .iter()
                .find(|node| node.handle == *handle)
                .unwrap_or_else(|| panic!("no {handle}"))
                .id
                .clone()
        })
        .collect()
}

const EXTRACTED: &str = r#"component check_cart_price "Check cart price" {
  description "Add the product to a cart and check the cart's total."
  @contract("Product")
  @required(true)
  input product: object
  input quantity: number = 1
  @required(true)
  input expected: number
  action product: object contract "Product"
  @label("Add to cart")
  add_to_cart = add_to_cart@1(product: param.product, quantity: param.quantity)
  @label("Promotional price")
  promotional_price = assert add_to_cart.total ~= param.expected within 0.005
  @label("Cart")
  export cart: object = add_to_cart
}
"#;

/// Q5: *Save as component* in a folder project: the dialog, then the
/// scenario calling the new component and the component's own board.
#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_comp00_q5_save_as_component() {
    for (size, suffix) in SIZES {
        let (_project, mut harness) = window(
            size,
            SCENARIO,
            "Diagram",
            Flags {
                inputs: Inputs::Card,
                save_dialog: true,
                publish_choice: true,
                publish: Publish::Latest(1),
                ..Flags::default()
            },
        );
        let graph = scenario_graph(&harness);
        let selected = ids(&graph, &["add_to_cart", "promotional_price"]);
        let scenario = by_path(&harness, SCENARIO);
        app(&mut harness)
            .selections
            .insert(scenario.clone(), selected.clone());
        let project = app(&mut harness).context_project();
        let mut dialog = crate::dialogs::ComponentDialog::new(&graph, &selected, &project, None);
        dialog.mock_folder("Check cart price");
        eprintln!("proposed inputs: {:?}", dialog.mock_inputs());
        mock::open_dialog(dialog);
        shot(&mut harness, &format!("comp00-q5-dialog-{suffix}"));
        // The rest of the dialog, scrolled.
        for (step, delta) in [("inputs", -250.0), ("end", -2000.0)] {
            harness.hover_at(egui::pos2(size.x / 2.0, size.y / 2.0));
            harness.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            settle(&mut harness);
            if size == MINIMUM || step == "end" {
                shot(&mut harness, &format!("comp00-q5-dialog-{step}-{suffix}"));
            }
        }
        mock::close_dialog();
        mock::update(|flags| flags.save_dialog = false);
        // The result: revision 1 published, the scenario calling it.
        {
            let app = app(&mut harness);
            let session = app.session.as_mut().unwrap();
            let id = session
                .create_document(
                    DocumentKind::Component,
                    "components/check-cart-price.afs",
                    EXTRACTED,
                )
                .unwrap();
            session.publish(&id, CODE).unwrap();
        }
        settle(&mut harness);
        edit(
            &mut harness,
            SCENARIO,
            "  @label(\"Add to cart\")\n  add_to_cart = add_to_cart@1(product: active_promotion, quantity: 1)\n  @label(\"Promotional price\")\n  promotional_price = assert add_to_cart.total ~= expected_price within 0.005\n",
            "  @label(\"Check cart price\")\n  check_cart_price = check_cart_price@1(product: active_promotion, expected: expected_price)\n",
        );
        edit(
            &mut harness,
            SCENARIO,
            "{{add_to_cart.id}}",
            "{{check_cart_price.id}}",
        );
        let graph = scenario_graph(&harness);
        let call = ids(&graph, &["check_cart_price"]);
        app(&mut harness).selections.insert(scenario, call);
        open(&mut harness, SCENARIO);
        shot(
            &mut harness,
            &format!("comp00-q5-after-scenario-diagram-{suffix}"),
        );
        open(&mut harness, "components/check-cart-price.afs");
        for mode in ["Diagram", "Split"] {
            click(&mut harness, mode);
            shot(
                &mut harness,
                &format!("comp00-q5-after-component-{}-{suffix}", mode.to_lowercase()),
            );
        }
    }
}
