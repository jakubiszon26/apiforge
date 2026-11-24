//! The two runs of a part of a scenario (ADR 0016).

use apiforge_core::run::RunResult;

/// A run of a part of the main graph.
#[derive(Clone, Debug, PartialEq)]
pub enum Partial {
    /// The scenario up to a node, with the checks and values computed before
    /// the next step, and then the whole Cleanup.
    Through { node_id: String },
    /// One request with the checks that read it, every other value read from
    /// `previous`, an earlier, already redacted report of the same scenario
    /// and mode. No other step and no Cleanup.
    Alone {
        node_id: String,
        previous: Option<Box<RunResult>>,
    },
}
