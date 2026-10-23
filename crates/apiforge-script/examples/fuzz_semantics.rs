//! Dependency-free mutational smoke fuzzer for lowering, printing and
//! reconciliation; not coverage-guided fuzzing. Inputs are generated scenario
//! DAGs mutated by lines and tokens, so many stay syntactically valid and reach
//! semantic checks.
//! Run: cargo run -p apiforge-script --example fuzz_semantics --release -- 60 2718281828
use apiforge_core::{ImportLimits, format::*, import_document};
use apiforge_script::{Document, LowerContext, lower, parse, print_scenario, reconcile, semantic};
use std::time::{Duration, Instant};

#[path = "../tests/support/generator.rs"]
mod generator;
use generator::{generate, random};

const FRAGMENTS: &[&str] = &[
    "@after(n1z)\n",
    "@after(n2z.then)\n",
    "@after(n3z.else, n1z)\n",
    "var ",
    "n1z",
    "n2z.passed",
    "n3z.body.items[0]",
    "input.idz",
    "param.limitz",
    "env.missing",
    "?",
    "start",
    "argument idz = n1z\n",
    "argument body = 1\n",
    "if true {\n",
    "} else {\n",
    "cleanup {\n",
    "}\n",
    "export x: any = 1\n",
    "produkt(id: n1z)",
    "produkt@9()",
    "op.add(left: n2z, right: ?)",
    "condition all { exists n1z }",
    "assert n1z ~= 1 within 0.5",
    "\"{{input.idz}}\"",
    "@config({ \"method\": \"GET\" })\n",
    "@inputs({ value: n1z })\n",
    "@note(\"x\")\n",
];

fn mutate(state: &mut u64, source: &str) -> String {
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    for _ in 0..=random(state) % 3 {
        let n = lines.len().max(1);
        let at = random(state) % n;
        match random(state) % 6 {
            0 if lines.len() > 1 => {
                lines.remove(at);
            }
            1 => {
                let line = lines.get(at).cloned().unwrap_or_default();
                lines.insert(random(state) % n, line);
            }
            2 => {
                let other = random(state) % n;
                if at < lines.len() && other < lines.len() {
                    lines.swap(at, other);
                }
            }
            3 => {
                // Replace one handle with another, keeping syntax intact.
                let from = format!("n{}z", 1 + random(state) % 12);
                let to = format!("n{}z", 1 + random(state) % 12);
                if let Some(line) = lines.get_mut(at) {
                    *line = line.replacen(&from, &to, 1);
                }
            }
            4 => {
                let fragment = FRAGMENTS[random(state) % FRAGMENTS.len()];
                if let Some(line) = lines.get_mut(at) {
                    let mut cut = random(state) % (line.len() + 1);
                    while !line.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    line.insert_str(cut, fragment);
                }
            }
            _ => {
                if let Some(line) = lines.get_mut(at) {
                    let mut cut = random(state) % (line.len() + 1);
                    while !line.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    line.truncate(cut);
                }
            }
        }
    }
    lines.join("\n")
}

/// Returns whether the input lowered without errors.
fn verify(source: &str, project: &Project, current: &Document) -> bool {
    let lowered = lower(&parse(source), &LowerContext::new(project));
    for diagnostic in &lowered.diagnostics {
        assert!(diagnostic.range.start <= diagnostic.range.end);
        assert!(diagnostic.range.end <= source.len());
        assert!(source.is_char_boundary(diagnostic.range.start));
        assert!(source.is_char_boundary(diagnostic.range.end));
    }
    if lowered.has_errors() {
        assert!(reconcile(current, &lowered).is_err());
        return false;
    }
    let Some(Document::Scenario(document)) = lowered.document.clone() else {
        panic!("a scenario without errors has a document: {source}");
    };
    let text = print_scenario(project, &document)
        .unwrap_or_else(|e| panic!("print failed: {e:?}\n{source}"));
    let again = lower(&parse(&text), &LowerContext::new(project));
    assert!(!again.has_errors(), "{:?}\n{text}", again.diagnostics);
    let again = again.document.unwrap();
    assert_eq!(
        semantic(&again, project),
        semantic(&Document::Scenario(document), project),
        "{source}\n---\n{text}"
    );
    let Document::Scenario(reprinted) = &again else {
        unreachable!()
    };
    assert_eq!(
        print_scenario(project, reprinted).unwrap(),
        text,
        "{source}"
    );
    let reconciled = reconcile(current, &lowered).expect("valid source reconciles");
    let Document::Scenario(scenario) = reconciled.document else {
        unreachable!()
    };
    let known = |id: &str| {
        scenario
            .graph
            .nodes
            .iter()
            .chain(&scenario.cleanup.nodes)
            .any(|n| n.id == id)
    };
    for graph in [&scenario.graph, &scenario.cleanup] {
        for edge in &graph.edges {
            assert!(known(&edge.source) && known(&edge.target));
        }
        for node in &graph.nodes {
            for binding in node.inputs.values() {
                for reference in binding.references() {
                    assert!(known(reference.node_id), "dangling reference\n{source}");
                }
            }
        }
    }
    true
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() <= 2,
        "Usage: fuzz_semantics [seconds] [nonzero-seed]"
    );
    let seconds: u64 = args.first().map_or(60, |s| s.parse().expect("seconds"));
    let seed: u64 = args
        .get(1)
        .map_or(2_718_281_828, |s| s.parse().expect("seed"));
    assert!(seconds > 0 && seed != 0);
    let project = import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0);
    // A stored scenario to reconcile every valid input against.
    let base = lower(&parse(&generate(1)), &LowerContext::new(&project));
    let current = base.document.clone().expect("generated source lowers");

    let mut state = seed;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let (mut cases, mut valid) = (0_u64, 0_u64);
    while Instant::now() < deadline {
        let source = generate(random(&mut state) as u64);
        let mutated = mutate(&mut state, &source);
        if verify(&mutated, &project, &current) {
            valid += 1;
        }
        cases += 1;
    }
    println!("{cases} cases, {valid} lowered without errors, seed {seed}");
}
