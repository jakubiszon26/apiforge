//! The comment-loss decision (ADR 0012). Printing a whole file from its
//! document keeps a comment that belongs to a statement as that statement's
//! note, and has no place for a free comment, one that belongs to none
//! (Format keeps those, LANG-06). A graphical edit that cannot be written
//! into the text and a publication both print the whole file, so the first
//! such print that would lose free comments asks first and leaves the
//! original text alone until the caller has decided.

use super::{ProjectSession, Result};
use crate::{
    Error,
    document::{AnalysisState, EditedText},
};

impl ProjectSession {
    /// The comments in this document's text that canonical printing would
    /// drop, in source order. Empty when nothing would be lost.
    pub fn unattached_comments(&self, document_id: &str) -> Result<Vec<String>> {
        Ok(self.documents[self.index(document_id)?].unattached_comments())
    }

    /// Accepts comment loss for this source version only. Any source edit
    /// invalidates the decision, including an edit retaining the same comments.
    pub fn accept_comment_loss(&mut self, document_id: &str) -> Result<()> {
        let index = self.index(document_id)?;
        self.documents[index].accept_comment_loss()
    }

    /// Forgets a decision, so the next destructive reprint asks again.
    pub fn reject_comment_loss(&mut self, document_id: &str) -> Result<()> {
        let index = self.index(document_id)?;
        self.documents[index].comments_accepted = None;
        Ok(())
    }

    pub(crate) fn require_comment_decision(&self, index: usize) -> Result<()> {
        self.documents[index].require_comment_decision()
    }
}

impl EditedText {
    /// Accepts comment loss for this version of the text only.
    pub(crate) fn accept_comment_loss(&mut self) -> Result<()> {
        if self.analysis != AnalysisState::Valid {
            return Err(Error::ModelLocked);
        }
        self.comments_accepted = Some(self.source_version);
        Ok(())
    }

    pub(crate) fn require_comment_decision(&self) -> Result<()> {
        let comments = self.unattached_comments();
        if comments.is_empty() || self.comments_accepted == Some(self.source_version) {
            return Ok(());
        }
        Err(Error::CommentsWouldBeLost {
            comments: comments.len(),
        })
    }

    /// The free comments of the analysed text (LANG-06). A document whose
    /// analysis has not settled reports nothing rather than a stale set.
    pub fn unattached_comments(&self) -> Vec<String> {
        if self.analysis != AnalysisState::Valid {
            return Vec::new();
        }
        apiforge_script::free_comments(self.text())
            .into_iter()
            .filter_map(|range| self.text().get(range))
            .map(str::to_owned)
            .collect()
    }
}
