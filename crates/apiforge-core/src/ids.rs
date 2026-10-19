//! Identifiers for new document entities.

/// A fresh identifier such as `node-1a2b3c4d`, in the reference implementation's
/// format: a prefix and the first eight hexadecimal digits of a random UUID.
///
/// Eight digits keep documents readable; callers that insert into an existing
/// collection should still check for a (very unlikely) collision.
pub fn new_id(prefix: &str) -> String {
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}-{}", &uuid[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_have_the_reference_shape_and_differ() {
        let first = new_id("node");
        let second = new_id("node");
        assert_eq!(first.len(), "node-".len() + 8);
        assert!(first[5..].bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
