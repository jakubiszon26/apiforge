use super::{ProjectError, Result};
use serde::Serialize;
use std::io::{self, Write};

/// Bound encoded bytes while serialising, including JSON escape expansion. A
/// limit failure never returns a truncated payload for persistence.
pub(super) fn encode(value: &impl Serialize, limit: usize) -> Result<Vec<u8>> {
    struct Bounded {
        bytes: Vec<u8>,
        limit: usize,
        exceeded: bool,
    }
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                self.exceeded = true;
                return Err(io::Error::other("encoded payload exceeds storage limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded {
        bytes: Vec::new(),
        limit,
        exceeded: false,
    };
    let result = serde_json::to_writer_pretty(&mut output, value);
    if output.exceeded {
        return Err(ProjectError::Limit);
    }
    result.map_err(|_| ProjectError::InvalidMetadata)?;
    Ok(output.bytes)
}
