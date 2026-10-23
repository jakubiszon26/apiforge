//! Generated graph topologies: random DAGs of every statement form, checked for
//! semantic round trips, print idempotence and identity-preserving reconciliation.
use apiforge_core::{ImportLimits, format::*, import_document, validate_workspace};
use apiforge_script::{
    Document, LowerContext, Lowered, lower, parse, print_scenario, reconcile, semantic,
};
use std::collections::{BTreeMap, BTreeSet};

#[path = "support/generator.rs"]
mod generator;
use generator::generate;

fn project() -> Project {
    import_document(
        include_bytes!("../../../fixtures/workspace.json"),
        &ImportLimits::default(),
    )
    .unwrap()
    .into_workspace()
    .projects
    .remove(0)
}

fn compile(source: &str, project: &Project) -> Lowered {
    let result = lower(&parse(source), &LowerContext::new(project));
    assert!(!result.has_errors(), "{:#?}\n{source}", result.diagnostics);
    result
}

fn scenario(document: Document) -> Scenario {
    match document {
        Document::Scenario(scenario) => scenario,
        Document::Component(_) => panic!("expected a scenario"),
    }
}

/// Node ID and position by handle.
type Nodes = BTreeMap<String, (String, f64, f64)>;

/// Node IDs and positions by handle, and edge IDs by their handle triples.
fn identities(scenario: &Scenario) -> (Nodes, BTreeSet<String>) {
    let mut nodes = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for graph in [&scenario.graph, &scenario.cleanup] {
        for node in &graph.nodes {
            names.insert(node.id.clone(), node.handle.clone());
            nodes.insert(
                node.handle.clone(),
                (node.id.clone(), node.position.x, node.position.y),
            );
        }
    }
    for graph in [&scenario.graph, &scenario.cleanup] {
        for edge in &graph.edges {
            edges.insert(format!(
                "{} {}->{} {:?}",
                edge.id, names[&edge.source], names[&edge.target], edge.branch
            ));
        }
    }
    (nodes, edges)
}

/// Gives the lowered document stable identities and distinct positions, as a
/// stored document has.
fn stored(lowered: &Lowered, project: &Project) -> Scenario {
    let placeholder = Document::Scenario(Scenario {
        id: "stored".into(),
        ..scenario(lowered.document.clone().unwrap())
    });
    let mut scenario = scenario(reconcile(&placeholder, lowered).unwrap().document);
    scenario.id = "stored".into();
    for (index, node) in scenario
        .graph
        .nodes
        .iter_mut()
        .chain(scenario.cleanup.nodes.iter_mut())
        .enumerate()
    {
        node.position = Position {
            x: index as f64 * 10.0,
            y: index as f64 * -7.0,
        };
        node.auto_placed = false;
    }
    let mut workspace = Workspace {
        version: project.version,
        active_id: project.id.clone(),
        projects: vec![project.clone()],
    };
    workspace.projects[0].scenarios.push(scenario.clone());
    validate_workspace(&workspace).unwrap_or_else(|e| panic!("{e:?}"));
    scenario
}

#[test]
fn generated_topologies_round_trip_and_reconcile_without_losing_identity() {
    let project = project();
    let mut nodes = 0;
    let mut edges = 0;
    let mut branches = 0;
    for seed in 0..400 {
        let source = generate(seed);
        let original = stored(&compile(&source, &project), &project);
        nodes += original.graph.nodes.len() + original.cleanup.nodes.len();
        edges += original.graph.edges.len() + original.cleanup.edges.len();
        branches += original
            .graph
            .edges
            .iter()
            .filter(|e| e.branch.is_some())
            .count();
        let current = Document::Scenario(original.clone());

        let text = print_scenario(&project, &original)
            .unwrap_or_else(|e| panic!("seed={seed}: {e:#?}\n{source}"));
        let reprinted = compile(&text, &project);
        assert_eq!(
            semantic(reprinted.document.as_ref().unwrap(), &project),
            semantic(&current, &project),
            "seed={seed}\n{source}\n---\n{text}"
        );
        let again = scenario(reprinted.document.clone().unwrap());
        assert_eq!(
            print_scenario(&project, &again).unwrap(),
            text,
            "seed={seed}"
        );

        // Reapplying the printed source is an identity on IDs and layout.
        let same = reconcile(&current, &reprinted).unwrap();
        assert!(same.unpositioned.is_empty(), "seed={seed}");
        assert_eq!(
            identities(&scenario(same.document)),
            identities(&original),
            "seed={seed}"
        );

        // A complete rename of one handle keeps every identity.
        let candidates: Vec<_> = original
            .graph
            .nodes
            .iter()
            .chain(&original.cleanup.nodes)
            .map(|n| n.handle.clone())
            .collect();
        let target = &candidates[seed as usize % candidates.len()];
        let renamed_source = text.replace(target.as_str(), "renamedz");
        let renamed = reconcile(&current, &compile(&renamed_source, &project)).unwrap();
        assert!(
            renamed.unpositioned.is_empty(),
            "seed={seed} rename {target}"
        );
        let (mut expected, _) = identities(&original);
        let moved = expected.remove(target).unwrap();
        expected.insert("renamedz".into(), moved);
        assert_eq!(
            identities(&scenario(renamed.document)).0,
            expected,
            "seed={seed}"
        );

        // An inserted independent statement is the only node needing layout.
        let inserted_source = text.replacen("{\n", "{\n  insertedz = 42\n", 1);
        let inserted = reconcile(&current, &compile(&inserted_source, &project)).unwrap();
        assert_eq!(inserted.unpositioned.len(), 1, "seed={seed}");
        let (with_insert, _) = identities(&scenario(inserted.document));
        let (before, _) = identities(&original);
        assert_eq!(with_insert.len(), before.len() + 1);
        for (handle, identity) in before {
            assert_eq!(with_insert[&handle], identity, "seed={seed} {handle}");
        }
    }
    // Guard against a generator that silently degenerates.
    assert!(nodes > 4000, "{nodes}");
    assert!(edges > 800, "{edges}");
    assert!(branches > 150, "{branches}");
}
