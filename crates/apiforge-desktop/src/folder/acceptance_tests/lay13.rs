//! Mock-up of LAY-13 (#297): branch bands. Throwaway; `APIFORGE_LAY13`
//! chooses what the board draws (unset: `main`).
use super::lay23::{Checks, board, fixture, graph, show, tidied_text};
use super::*;

fn checkout() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scripts/checkout.afs");
    fs::read_to_string(path).unwrap()
}

#[test]
#[ignore = "design review capture; run with tools/gui/review.sh"]
fn review_lay13_mock() {
    let variant = std::env::var("APIFORGE_LAY13_NAME")
        .or_else(|_| std::env::var("APIFORGE_LAY13"))
        .unwrap_or_else(|_| "main".to_owned());
    for (size, suffix) in [(WIDE, "1600"), (crate::testing::MINIMUM, "960")] {
        for (name, text, at, checks) in [
            ("shop-30", fixture("shop-30"), "payment", Checks::Expanded),
            ("shop-30-folded", fixture("shop-30"), "payment", Checks::Collapsed),
            ("shop-50", fixture("shop-50"), "paid", Checks::Expanded),
            ("checkout", checkout(), "created", Checks::Expanded),
        ] {
            if std::env::var("APIFORGE_LAY13_ONLY").is_ok_and(|only| only != name) {
                continue;
            }
            let (_project, mut harness, id) = tidied_text(&text, size, checks);
            choose(&mut harness, "Run simulation");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while folder(&harness).runs.handle.is_some() {
                assert!(std::time::Instant::now() < deadline, "the run must finish");
                harness.step();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            settle(&mut harness);
            harness.step();
            choose(&mut harness, "Run results");
            settle(&mut harness);
            harness.step();
            for _ in 0..3 {
                harness.step();
            }
            choose(&mut harness, "Fit the view");
            settle(&mut harness);
            harness.step();
            choose(&mut harness, "Fit the view");
            harness.step();
            if suffix == "1600" {
                let graph = graph(&harness, &id);
                let canvas = board(&harness, &id);
                for node in &graph.nodes {
                    errln!("LAY-13 {name} card {} {:?} {:?}", node.handle, node.kind, canvas.debug_world(&node.id));
                }
                for edge in &graph.edges {
                    let h = |id: &str| super::lay23::handle_of(&graph, id);
                    errln!("LAY-13 {name} edge {} -> {} {:?}", h(&edge.source), h(&edge.target), edge.branch);
                }
            }
            crate::testing::capture(&mut harness, &format!("lay13-{variant}-{name}-fit-{suffix}"), None);
            show(&mut harness, &id, at, 0.6);
            crate::testing::capture(&mut harness, &format!("lay13-{variant}-{name}-60-{suffix}"), None);
            if suffix == "1600" {
                show(&mut harness, &id, at, 1.0);
                crate::testing::capture(&mut harness, &format!("lay13-{variant}-{name}-100-{suffix}"), None);
            }
        }
    }
}
