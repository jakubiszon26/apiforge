//! MCP resources: `apiforge://reference`, the `.afs` language reference, and
//! `apiforge://source/<path>`, the saved text of each registered document.
//! A source URI's path is percent-decoded and must pass
//! `ProjectPath::working_source` before the folder is touched.
use crate::{
    error::{ErrorCode, ToolError},
    reference, tools,
};
use apiforge_project::DocumentKind;
use std::path::Path;

pub const REFERENCE_URI: &str = "apiforge://reference";
pub const SOURCE_PREFIX: &str = "apiforge://source/";
pub const MIME_TYPE: &str = "text/plain";

/// One entry of `resources/list`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: String,
}

/// The reference and one source per registered document, in registry order.
pub fn list(folder: &Path) -> Result<Vec<Resource>, ToolError> {
    let session = tools::open(folder)?;
    let mut resources = vec![Resource {
        uri: REFERENCE_URI.to_owned(),
        name: "language-reference".to_owned(),
        description: format!(
            "The .afs language reference for models, version {}",
            reference::version()
        ),
    }];
    for document in session.documents() {
        let path = document.entry().path.as_str();
        resources.push(Resource {
            uri: format!("{SOURCE_PREFIX}{}", encode(path)),
            name: path.to_owned(),
            description: match document.entry().kind {
                DocumentKind::Scenario => "The saved source of a scenario",
                DocumentKind::Component => "The saved source of a working component",
            }
            .to_owned(),
        });
    }
    Ok(resources)
}

/// The text behind `uri`: the reference, or the saved bytes of a registered
/// document as text.
pub fn read(folder: &Path, uri: &str) -> Result<String, ToolError> {
    read_with(folder, uri, &[])
}

/// As `read`; a saved source that contains one of `secrets` is withheld
/// with `secret_in_source`.
pub fn read_with(folder: &Path, uri: &str, secrets: &[String]) -> Result<String, ToolError> {
    if uri == REFERENCE_URI {
        return Ok(reference::TEXT.to_owned());
    }
    let Some(encoded) = uri.strip_prefix(SOURCE_PREFIX) else {
        return Err(ToolError::new(
            ErrorCode::UnknownDocument,
            "No resource has this URI; resources/list names them",
        ));
    };
    let path = decode(encoded).ok_or_else(|| {
        ToolError::new(
            ErrorCode::PathRejected,
            "The resource path is not percent-encoded UTF-8",
        )
    })?;
    let source = tools::read_source_with(folder, &path, secrets)?;
    match source.unavailable.as_deref() {
        None => Ok(source.text),
        Some("too_large") => Err(ToolError::new(
            ErrorCode::TooLarge,
            "The saved source exceeds the 1 MiB limit",
        )),
        Some(_) => Err(ToolError::new(
            ErrorCode::ProjectUnreadable,
            "The saved source cannot be read as text; read_source says why",
        )),
    }
}

/// Percent-encodes every byte but the unreserved characters and `/`.
fn encode(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// Percent-decodes `encoded`; `None` for a malformed escape or a result that
/// is not UTF-8.
fn decode(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes.get(index + 1..index + 3)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            decoded.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn paths_round_trip_through_percent_encoding() {
        for path in ["components/product.afs", "a b/ż.afs", "x%y.afs"] {
            assert_eq!(decode(&encode(path)).as_deref(), Some(path));
        }
        assert_eq!(decode("%2e%2E/x").as_deref(), Some("../x"));
        assert_eq!(decode("%"), None);
        assert_eq!(decode("%zz"), None);
        assert_eq!(decode("%ff"), None);
    }
}
