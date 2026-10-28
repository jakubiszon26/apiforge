//! The `.afs` language reference for models (MCP-02), embedded in the binary.

/// `docs/language/reference.md` as it was when the binary was built.
pub const TEXT: &str = include_str!("../../../docs/language/reference.md");

/// The version its first line declares (`Reference version: <n> · …`); empty
/// when the line does not declare one.
pub fn version() -> &'static str {
    TEXT.lines()
        .next()
        .and_then(|line| line.strip_prefix("Reference version: "))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_first_line_declares_the_version() {
        assert!(!super::version().is_empty());
        assert!(super::version().chars().all(|c| c.is_ascii_digit()));
    }
}
