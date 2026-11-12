//! LAY-01: the quality measurement.
//!
//! These tests run the command the ticket names, the way a person does, and read
//! what it prints. Its measures are checked on hand-made boards by the example's
//! own unit tests.

use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
    sync::OnceLock,
};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn quality(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO"))
        .current_dir(root())
        .args(["run", "--quiet", "--locked", "-p", "apiforge-layout"])
        .args(["--example", "quality", "--"])
        .args(arguments)
        .output()
        .expect("cargo starts")
}

fn printed(arguments: &[&str]) -> String {
    let output = quality(arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

/// The table of a plain run, printed once for the tests that read it.
fn report() -> &'static str {
    static REPORT: OnceLock<String> = OnceLock::new();
    REPORT.get_or_init(|| printed(&[]))
}

/// The rows of the table whose header starts with `first`, as column → value.
fn table(text: &str, first: &str) -> Vec<HashMap<String, String>> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines
        .by_ref()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|columns| columns.first() == Some(&first))
        .unwrap_or_else(|| panic!("no table starting with `{first}` in:\n{text}"));
    lines
        .map_while(|line| {
            let cells: Vec<&str> = line.split_whitespace().collect();
            (cells.len() == header.len()).then(|| {
                header
                    .iter()
                    .zip(cells)
                    .map(|(name, cell)| ((*name).to_owned(), cell.to_owned()))
                    .collect()
            })
        })
        .collect()
}

fn number(row: &HashMap<String, String>, column: &str) -> f64 {
    row.get(column)
        .unwrap_or_else(|| panic!("no column `{column}` in {row:?}"))
        .parse()
        .unwrap_or_else(|_| panic!("`{column}` is not a number in {row:?}"))
}

#[test]
fn lay01_criterion_1_generated_boards_are_measured() {
    let rows = table(report(), "cards");
    let boards: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row["cards"].clone(), row["order"].clone()))
        .collect();
    let expected: Vec<(String, String)> = ["6", "12", "25", "50"]
        .iter()
        .flat_map(|cards| ["yes", "no"].map(|order| ((*cards).to_owned(), order.to_owned())))
        .collect();
    assert_eq!(boards, expected);
    for row in &rows {
        for column in [
            "wires",
            "crossings",
            "bends",
            "straight",
            "close",
            "coincident",
            "long",
            "incomplete",
        ] {
            assert!(number(row, column) >= 0.0, "{column} in {row:?}");
        }
        assert!(number(row, "wires") > 0.0, "{row:?}");
    }
    // The ordering chain is one more wire per card.
    let wires = |index: usize| number(&rows[index], "wires");
    assert!(wires(6) > wires(7), "with and without the chain");
    // A larger board has more wires and something to cross.
    assert!(wires(6) > wires(0));
    assert!(number(&rows[6], "crossings") > 0.0);
}

#[test]
fn lay01_criterion_2_two_runs_print_the_same() {
    assert_eq!(report(), printed(&[]));
}

#[test]
fn lay01_criterion_3_dump_prints_one_board() {
    let dump = printed(&["--dump", "3", "12"]);
    let cards = dump
        .lines()
        .filter(|line| line.starts_with("card "))
        .count();
    let wires = dump
        .lines()
        .filter(|line| line.starts_with("wire "))
        .count();
    assert_eq!(cards, 12, "{dump}");
    assert!(wires >= 11, "{dump}");
    assert_eq!(dump, printed(&["--dump", "3", "12"]));
    assert_ne!(dump, printed(&["--dump", "4", "12"]));
}

#[test]
fn lay01_criterion_3_a_wrong_dump_is_refused() {
    for arguments in [&["--dump"][..], &["--dump", "x", "12"], &["--dump", "3"]] {
        assert!(!quality(arguments).status.success(), "{arguments:?}");
    }
}

#[test]
fn lay01_criterion_4_the_guide_names_the_command_beside_the_benchmark() {
    let guide = std::fs::read_to_string(root().join("docs/development.md")).expect("the guide");
    let bench = guide
        .find("--example bench --release")
        .expect("the benchmark");
    let quality = guide
        .find("cargo run -p apiforge-layout --example quality --release")
        .expect("the command");
    assert!(
        quality.abs_diff(bench) < 1000,
        "the two commands are in the same place"
    );
}

/// Since LAY-06 each scenario is measured as the desktop tidies it, at the
/// compact and the full density, with its checks collapsed and expanded.
#[test]
fn lay01_criterion_6_the_fixtures_are_measured() {
    let rows = table(report(), "scenario");
    let mut names: Vec<&str> = rows.iter().map(|row| row["scenario"].as_str()).collect();
    names.dedup();
    assert_eq!(names, ["order-flow", "shop-30", "shop-50"]);
    assert_eq!(rows.len(), 12, "two densities and two states each");
    let mut steps: Vec<f64> = rows.iter().map(|row| number(row, "steps")).collect();
    steps.dedup();
    assert_eq!(steps, [14.0, 30.0, 56.0]);
    for row in &rows {
        for column in [
            "wires",
            "crossings",
            "bends",
            "straight",
            "close",
            "coincident",
            "incomplete",
            "card_hits",
            "chip_hits",
            "clearance",
        ] {
            assert!(number(row, column) >= 0.0, "{column} in {row:?}");
        }
        assert!(number(row, "width") >= 280.0, "{row:?}");
        assert!(number(row, "height") > 0.0, "{row:?}");
        let (large, small) = (number(row, "fit_1600"), number(row, "fit_960"));
        assert!(0.0 < small && small < large && large <= 1.0, "{row:?}");
        // Tidy up leaves no card on another.
        assert_eq!(number(row, "card_hits"), 0.0, "{row:?}");
    }
}
