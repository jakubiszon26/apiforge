//! The `.afs` language reference for models (MCP-02), embedded in the binary.

/// `docs/language/reference.md` as it was when the binary was built.
pub const TEXT: &str = include_str!("../../../docs/language/reference.md");

/// The version its first line declares (`Reference version: <n> · …`).
pub fn version() -> &'static str {
    unimplemented!("MCP-03")
}
