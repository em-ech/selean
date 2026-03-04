//! Append-only operation log with per-user, per-page indexing.
//!
//! The server maintains an [`OpLog`] per room. Operations are appended with
//! monotonically increasing sequence numbers. The per-user index enables
//! efficient undo/redo lookups.

use std::collections::HashMap;

use selean_common::types::{PageId, UserId};

use crate::types::{Operation, SeqNum};

/// Append-only log of all operations in a room, ordered by server sequence number.
#[derive(Debug, Default)]
pub struct OpLog {
    /// All operations in append order.
    ops: Vec<Operation>,
    /// Seq-to-index lookup for O(1) retrieval by sequence number.
    seq_index: HashMap<SeqNum, usize>,
    /// Per-user, per-page index of sequence numbers for undo traversal.
    user_ops: HashMap<(UserId, PageId), Vec<SeqNum>>,
}

impl OpLog {
    /// Creates a new empty operation log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an operation to the log.
    ///
    /// The operation must have a `seq` set (server-assigned).
    ///
    /// # Panics
    ///
    /// Panics if `op.seq` is `None`.
    pub fn append(&mut self, op: Operation) {
        let seq = op
            .seq
            .unwrap_or_else(|| panic!("operation must have a seq when appending to OpLog"));
        let key = (op.user_id, op.page_id);
        self.user_ops.entry(key).or_default().push(seq);
        let idx = self.ops.len();
        self.ops.push(op);
        self.seq_index.insert(seq, idx);
    }

    /// Returns the operation with the given sequence number in O(1).
    #[must_use]
    pub fn get_by_seq(&self, seq: SeqNum) -> Option<&Operation> {
        self.seq_index.get(&seq).map(|&idx| &self.ops[idx])
    }

    /// Returns the sequence number of the most recent operation by this user
    /// on this page, without removing it.
    #[must_use]
    pub fn user_latest(&self, user_id: UserId, page_id: PageId) -> Option<SeqNum> {
        self.user_ops
            .get(&(user_id, page_id))
            .and_then(|v| v.last().copied())
    }

    /// Removes and returns the sequence number of the most recent operation
    /// by this user on this page. Used for undo.
    pub fn pop_user_latest(&mut self, user_id: UserId, page_id: PageId) -> Option<SeqNum> {
        self.user_ops
            .get_mut(&(user_id, page_id))
            .and_then(Vec::pop)
    }

    /// Returns the total number of operations in the log.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns `true` if the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Returns all sequence numbers for a user on a page.
    #[must_use]
    pub fn user_page_seqs(&self, user_id: UserId, page_id: PageId) -> &[SeqNum] {
        self.user_ops
            .get(&(user_id, page_id))
            .map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::types::SessionId;
    use selean_common::types::NodeId;
    use selean_engine::command::CommandDescriptor;
    use selean_engine::scene::BoundingBox;

    fn make_op(seq: SeqNum, user_id: UserId, page_id: PageId) -> Operation {
        Operation {
            seq: Some(seq),
            client_seq: seq,
            user_id,
            session_id: SessionId::new(),
            page_id,
            descriptor: CommandDescriptor::SetBounds {
                node_id: NodeId::new(),
                bounds: BoundingBox::new(0.0, 0.0, 100.0, 100.0),
            },
            timestamp: Some(1_700_000_000 + seq),
        }
    }

    #[test]
    fn new_log_is_empty() {
        let log = OpLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn append_and_get_by_seq() {
        let mut log = OpLog::new();
        let user = UserId::new();
        let page = PageId::new();
        let op = make_op(1, user, page);
        log.append(op);

        assert_eq!(log.len(), 1);
        let retrieved = log.get_by_seq(1).unwrap();
        assert_eq!(retrieved.seq, Some(1));
    }

    #[test]
    fn get_by_seq_not_found() {
        let log = OpLog::new();
        assert!(log.get_by_seq(999).is_none());
    }

    #[test]
    fn user_latest_returns_most_recent() {
        let mut log = OpLog::new();
        let user = UserId::new();
        let page = PageId::new();

        log.append(make_op(1, user, page));
        log.append(make_op(2, user, page));
        log.append(make_op(3, user, page));

        assert_eq!(log.user_latest(user, page), Some(3));
    }

    #[test]
    fn user_latest_empty_returns_none() {
        let log = OpLog::new();
        assert!(log.user_latest(UserId::new(), PageId::new()).is_none());
    }

    #[test]
    fn pop_user_latest_removes() {
        let mut log = OpLog::new();
        let user = UserId::new();
        let page = PageId::new();

        log.append(make_op(1, user, page));
        log.append(make_op(2, user, page));

        assert_eq!(log.pop_user_latest(user, page), Some(2));
        assert_eq!(log.user_latest(user, page), Some(1));
        assert_eq!(log.pop_user_latest(user, page), Some(1));
        assert!(log.pop_user_latest(user, page).is_none());
    }

    #[test]
    fn per_user_isolation() {
        let mut log = OpLog::new();
        let alice = UserId::new();
        let bob = UserId::new();
        let page = PageId::new();

        log.append(make_op(1, alice, page));
        log.append(make_op(2, bob, page));
        log.append(make_op(3, alice, page));

        assert_eq!(log.user_latest(alice, page), Some(3));
        assert_eq!(log.user_latest(bob, page), Some(2));
    }

    #[test]
    fn per_page_isolation() {
        let mut log = OpLog::new();
        let user = UserId::new();
        let page_a = PageId::new();
        let page_b = PageId::new();

        log.append(make_op(1, user, page_a));
        log.append(make_op(2, user, page_b));

        assert_eq!(log.user_latest(user, page_a), Some(1));
        assert_eq!(log.user_latest(user, page_b), Some(2));
    }

    #[test]
    fn user_page_seqs_returns_ordered() {
        let mut log = OpLog::new();
        let user = UserId::new();
        let page = PageId::new();

        log.append(make_op(1, user, page));
        log.append(make_op(5, user, page));
        log.append(make_op(10, user, page));

        assert_eq!(log.user_page_seqs(user, page), &[1, 5, 10]);
    }

    #[test]
    fn user_page_seqs_empty_for_unknown() {
        let log = OpLog::new();
        assert!(log.user_page_seqs(UserId::new(), PageId::new()).is_empty());
    }

    #[test]
    #[should_panic(expected = "operation must have a seq")]
    fn append_without_seq_panics() {
        let mut log = OpLog::new();
        let op = Operation {
            seq: None,
            client_seq: 1,
            user_id: UserId::new(),
            session_id: SessionId::new(),
            page_id: PageId::new(),
            descriptor: CommandDescriptor::SetBounds {
                node_id: NodeId::new(),
                bounds: BoundingBox::new(0.0, 0.0, 50.0, 50.0),
            },
            timestamp: None,
        };
        log.append(op);
    }

    #[test]
    fn multiple_ops_preserved() {
        let mut log = OpLog::new();
        let user = UserId::new();
        let page = PageId::new();

        for i in 1..=10 {
            log.append(make_op(i, user, page));
        }

        assert_eq!(log.len(), 10);
        for i in 1..=10 {
            assert!(log.get_by_seq(i).is_some());
        }
    }
}
