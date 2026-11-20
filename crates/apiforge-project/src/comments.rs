//! The comment-loss decision (ADR 0012). Printing a whole file from its
//! document keeps a comment that belongs to a statement as that statement's
//! note, and has no place for a free comment, one that belongs to none
//! (Format keeps those, LANG-06). A graphical edit that cannot be written
//! into the text and a publication both print the whole file, so the first
//! such print that would lose free comments asks first and leaves the
//! original text alone until the caller has decided.

use super::{ProjectSession, Result};
use crate::{Error, document::AnalysisState};

impl ProjectSession {
    /// The comments in this document's text that canonical printing would
    /// drop, in source order. Empty when nothing would be lost.
    pub fn unattached_comments(&self, document_id: &str) -> Result<Vec<String>> {
        Ok(self.unattached(self.index(document_id)?))
    }

    /// Accepts comment loss for this source version only. Any source edit
    /// invalidates the decision, including an edit retaining the same comments.
    pub fn accept_comment_loss(&mut self, document_id: &str) -> Result<()> {
        let index = self.index(document_id)?;
        if self.documents[index].analysis != AnalysisState::Valid {
            return Err(Error::ModelLocked);
        }
        self.documents[index].comments_accepted = Some(self.documents[index].source_version);
        Ok(())
    }

    /// Forgets a decision, so the next destructive reprint asks again.
    pub fn reject_comment_loss(&mut self, document_id: &str) -> Result<()> {
        let index = self.index(document_id)?;
        self.documents[index].comments_accepted = None;
        Ok(())
    }

    pub(crate) fn require_comment_decision(&self, index: usize) -> Result<()> {
        let comments = self.unattached(index);
        if comments.is_empty()
            || self.documents[index].comments_accepted == Some(self.documents[index].source_version)
        {
            return Ok(());
        }
        Err(Error::CommentsWouldBeLost {
            comments: comments.len(),
        })
    }

    /// The free comments of the analysed text (LANG-06). A document whose
    /// analysis has not settled reports nothing rather than a stale set.
    fn unattached(&self, index: usize) -> Vec<String> {
        let document = &self.documents[index];
        if document.analysis != AnalysisState::Valid {
            return Vec::new();
        }
        apiforge_script::free_comments(document.text())
            .into_iter()
            .filter_map(|range| document.text().get(range))
            .map(str::to_owned)
            .collect()
    }
}
