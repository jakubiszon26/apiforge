//! Byte offsets to line and column: 1-based, counted in Unicode scalar
//! values, a `\r` before `\n` part of the line break.
use std::ops::Range;

/// The line starts of one text, for many lookups.
pub struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub fn new(text: &'a str) -> Self {
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        Self { text, starts }
    }

    /// `(line, column)` of a byte offset. An offset past the end is the end;
    /// one inside a character is that character's start.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|start| *start <= offset);
        let mut before = &self.text[self.starts[line - 1]..offset];
        if before.ends_with('\r') && self.text[offset..].starts_with('\n') {
            before = &before[..before.len() - 1];
        }
        (line, before.chars().count() + 1)
    }

    /// `(line, column, end_line, end_column)`, the end exclusive.
    pub fn range(&self, range: Range<usize>) -> (usize, usize, usize, usize) {
        let (line, column) = self.position(range.start);
        let (end_line, end_column) = self.position(range.end.max(range.start));
        (line, column, end_line, end_column)
    }
}

/// `(line, column, end_line, end_column)` of a byte range, the end exclusive.
pub fn range(text: &str, range: Range<usize>) -> (usize, usize, usize, usize) {
    Lines::new(text).range(range)
}
