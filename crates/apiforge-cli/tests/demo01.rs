//! DEMO-01 (#523): the project a first start writes, `fixtures/demo/`, read
//! through the command line. Every file is formatted and checks with no
//! diagnostic at all; both scenarios pass as a simulation and live against
//! the demo API, and the live run leaves no cart behind. The window's side
//! (what a first start writes, the board) is
//! `crates/apiforge-desktop/src/folder/acceptance_tests/demo01.rs`.

use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const SCENARIOS: [&str; 2] = [
    "scenarios/buy-coffee-on-promotion.afs",
    "scenarios/the-cart-refuses-bad-orders.afs",
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// A copy of the demo, as a first start writes it, in a directory of its own.
fn demo() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("shop");
    copy(&repository().join("fixtures/demo"), &root);
    (directory, root)
}

/// Every `.afs` file of the demo, the published revision included, by its
/// path from the project's root.
fn sources(root: &Path) -> Vec<String> {
    fn walk(root: &Path, directory: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, found);
            } else if path.extension().is_some_and(|extension| extension == "afs") {
                let relative = path.strip_prefix(root).unwrap();
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

fn apiforge(args: &[&str], root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn run(root: &Path, scenario: &str, live: bool) -> Value {
    let mut args = vec!["run", "--folder", ".", "--scenario", scenario, "--json"];
    if live {
        args.push("--live");
    }
    let output = apiforge(&args, root);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "{scenario}: no JSON report (exit {:?}); stderr: {}",
            output.status.code(),
            stderr(&output)
        )
    })
}

fn step<'a>(report: &'a Value, handle: &str) -> &'a Value {
    report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["handle"] == handle)
        .unwrap_or_else(|| panic!("a step {handle}: {report:#}"))
}

/// The demo's requests sent to `base_url` instead of the default port.
fn point_at(root: &Path, base_url: &str) {
    let path = root.join("apiforge.project.json");
    let manifest = fs::read_to_string(&path).unwrap();
    let changed = manifest.replace("http://127.0.0.1:4318/demo/shop", base_url);
    assert_ne!(changed, manifest, "the demo names the demo API");
    fs::write(&path, changed).unwrap();
}

/// The status the demo API answers `GET path` with.
fn status_of(api: &apiforge_demo_api::DemoServerHandle, path: &str) -> u16 {
    let address = api.local_addr();
    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
        .split_whitespace()
        .nth(1)
        .and_then(|status| status.parse().ok())
        .unwrap_or_else(|| panic!("an HTTP answer: {answer}"))
}

/// Criterion 2: every file of the demo is in the form `fmt` writes, and
/// `check` reports nothing for it, not even a warning.
#[test]
fn demo01_criterion_2_clean() {
    let (_directory, root) = demo();
    let sources = sources(&root);
    assert_eq!(
        sources,
        [
            ".apiforge/revisions/product/1.afs",
            "components/product.afs",
            "scenarios/buy-coffee-on-promotion.afs",
            "scenarios/the-cart-refuses-bad-orders.afs",
        ],
        "the demo's sources"
    );
    for source in &sources {
        let output = apiforge(&["fmt", source, "--check"], &root);
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{source}: fmt --check: {}",
            stderr(&output)
        );
    }
    let output = apiforge(&["check", "--folder", "."], &root);
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "check: {}",
        stderr(&output)
    );
    for source in sources.iter().filter(|path| !path.starts_with(".apiforge")) {
        let output = apiforge(&["check", "--folder", ".", "--file", source], &root);
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{source}: check: {}",
            stderr(&output)
        );
    }
}

/// Criterion 3: both scenarios pass as a simulation and live against the demo
/// API; the live run deletes the cart it made.
#[test]
fn demo01_criterion_3_runs_both_ways() {
    let (_directory, root) = demo();
    for scenario in SCENARIOS {
        let report = run(&root, scenario, false);
        assert_eq!(report["mode"], "simulation", "{scenario}");
        assert_eq!(report["status"], "passed", "{scenario}: {report:#}");
    }
    // The simulation takes the branch the sample's promotion chooses.
    let report = run(&root, SCENARIOS[0], false);
    assert_eq!(step(&report, "discounted")["status"], "passed");
    assert_eq!(step(&report, "full_price")["status"], "skipped");

    let api = apiforge_demo_api::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    point_at(&root, &format!("{}/shop", api.base_url()));
    for scenario in SCENARIOS {
        let report = run(&root, scenario, true);
        assert_eq!(report["mode"], "live", "{scenario}");
        assert_eq!(report["status"], "passed", "{scenario}: {report:#}");
    }

    let report = run(&root, SCENARIOS[0], true);
    assert_eq!(report["status"], "passed", "{report:#}");
    let cart = step(&report, "cartId")["outputs"]["value"]
        .as_str()
        .expect("the API named the cart")
        .to_owned();
    assert_ne!(cart, "sample-cart", "the cart came from the API");
    assert_eq!(step(&report, "removed")["status"], "passed");
    assert_eq!(
        status_of(&api, &format!("/demo/shop/carts/{cart}")),
        404,
        "the cleanup left no cart"
    );

    // The three bad orders were refused, so they made no cart either.
    let report = run(&root, SCENARIOS[1], true);
    for handle in ["no_quantity", "too_many", "unknown_product"] {
        assert_eq!(step(&report, handle)["outputs"]["status"], 400, "{handle}");
    }
}
