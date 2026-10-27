//! Byte offsets to line and column: 1-based, counted in Unicode scalar
//! values, a `\r` before `\n` part of the line break.
use std::ops::Range;

/// Bytes between two character-count checkpoints.
const BLOCK: usize = 256;

/// The line starts and character-count checkpoints of one text, computed
/// once, so a lookup costs at most one block's scan however long its line
/// is.
pub struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
    /// The number of characters before byte `index * BLOCK`.
    checkpoints: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub fn new(text: &'a str) -> Self {
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        let mut checkpoints = Vec::with_capacity(text.len() / BLOCK + 1);
        let mut characters = 0;
        checkpoints.push(0);
        for block in text.as_bytes().chunks(BLOCK) {
            characters += leading_bytes(block);
            checkpoints.push(characters);
        }
        Self {
            text,
            starts,
            checkpoints,
        }
    }

    /// `(line, column)` of a byte offset. An offset past the end is the end;
    /// one inside a character is that character's start.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|start| *start <= offset);
        let start = self.starts[line - 1];
        let mut end = offset;
        if end > start
            && self.text.as_bytes()[end - 1] == b'\r'
            && self.text[end..].starts_with('\n')
        {
            end -= 1;
        }
        (
            line,
            self.characters_before(end) - self.characters_before(start) + 1,
        )
    }

    /// `(line, column, end_line, end_column)`, the end exclusive.
    pub fn range(&self, range: Range<usize>) -> (usize, usize, usize, usize) {
        let (line, column) = self.position(range.start);
        let (end_line, end_column) = self.position(range.end.max(range.start));
        (line, column, end_line, end_column)
    }

    /// Characters before byte `offset`, a character boundary.
    fn characters_before(&self, offset: usize) -> usize {
        let block = offset / BLOCK;
        self.checkpoints[block] + leading_bytes(&self.text.as_bytes()[block * BLOCK..offset])
    }
}

/// The bytes that start a character: every byte but a UTF-8 continuation.
fn leading_bytes(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte & 0xC0 != 0x80).count()
}

/// `(line, column, end_line, end_column)` of a byte range, the end exclusive.
pub fn range(text: &str, range: Range<usize>) -> (usize, usize, usize, usize) {
    Lines::new(text).range(range)
}
