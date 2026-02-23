//! Command history with undo/redo stacks and group batching.
//!
//! `CommandHistory` is the top-level manager for the undo/redo system. It
//! maintains two stacks (undo and redo) of executed commands, supports
//! grouping multiple commands into a single undo entry (for drags and
//! multi-step operations), and enforces a configurable maximum size.

use super::batch::CommandGroup;
use super::traits::Command;
use crate::scene::SceneGraph;

/// Default maximum number of entries in the undo stack.
const DEFAULT_MAX_SIZE: usize = 100;

/// Manages undo/redo stacks for scene graph commands.
///
/// Commands are executed through this struct, which pushes them onto
/// the undo stack. Undo pops from the undo stack and pushes to redo.
/// Redo pops from redo and pushes back to undo.
///
/// Any new command execution clears the redo stack (branching history
/// is not supported).
#[derive(Debug)]
pub struct CommandHistory {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    max_size: usize,
    /// Active group being built. When `Some`, executed commands are
    /// added to the group instead of the undo stack directly.
    active_group: Option<CommandGroup>,
}

impl CommandHistory {
    /// Creates a new command history with the default max size (100).
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: DEFAULT_MAX_SIZE,
            active_group: None,
        }
    }

    /// Creates a new command history with a custom max size.
    #[must_use]
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: max_size.max(1),
            active_group: None,
        }
    }

    /// Executes a command and pushes it to the undo stack (or active group).
    ///
    /// Clears the redo stack on success (unless inside a group). Returns
    /// `false` if the command's `execute` returns `false` (command is not
    /// pushed to any stack).
    pub fn execute(&mut self, mut command: Box<dyn Command>, scene: &mut SceneGraph) -> bool {
        if !command.execute(scene) {
            return false;
        }

        if let Some(group) = &mut self.active_group {
            group.push(command);
        } else {
            self.redo_stack.clear();
            self.undo_stack.push(command);
            self.enforce_size_limit();
        }

        true
    }

    /// Undoes the most recent command.
    ///
    /// Blocked during an active group (returns `false`). Returns `false`
    /// if the undo stack is empty.
    pub fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        if self.active_group.is_some() {
            return false;
        }

        let Some(mut command) = self.undo_stack.pop() else {
            return false;
        };

        if command.undo(scene) {
            self.redo_stack.push(command);
            true
        } else {
            // Push back if undo failed to avoid losing the command.
            self.undo_stack.push(command);
            false
        }
    }

    /// Redoes the most recently undone command.
    ///
    /// Blocked during an active group (returns `false`). Returns `false`
    /// if the redo stack is empty.
    pub fn redo(&mut self, scene: &mut SceneGraph) -> bool {
        if self.active_group.is_some() {
            return false;
        }

        let Some(mut command) = self.redo_stack.pop() else {
            return false;
        };

        if command.execute(scene) {
            self.undo_stack.push(command);
            self.enforce_size_limit();
            true
        } else {
            // Push back if execute failed.
            self.redo_stack.push(command);
            false
        }
    }

    /// Begins a new command group.
    ///
    /// All commands executed while a group is active will be collected
    /// into a single `CommandGroup` entry. Nested groups are not supported;
    /// calling this while a group is already active is a no-op.
    pub fn begin_group(&mut self, label: impl Into<String>) {
        if self.active_group.is_none() {
            self.active_group = Some(CommandGroup::new(label));
        }
    }

    /// Ends the active group and pushes it as a single undo entry.
    ///
    /// No-op if no group is active. Empty groups are discarded.
    pub fn end_group(&mut self) {
        if let Some(group) = self.active_group.take() {
            if !group.is_empty() {
                self.redo_stack.clear();
                self.undo_stack.push(Box::new(group));
                self.enforce_size_limit();
            }
        }
    }

    /// Cancels the active group, undoing all commands that were executed
    /// as part of the group.
    ///
    /// No-op if no group is active.
    pub fn cancel_group(&mut self, scene: &mut SceneGraph) {
        if let Some(mut group) = self.active_group.take() {
            group.undo(scene);
        }
    }

    /// Returns `true` if there are commands that can be undone.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are commands that can be redone.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns the number of commands on the undo stack.
    #[must_use]
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of commands on the redo stack.
    #[must_use]
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Returns `true` if a command group is currently active.
    #[must_use]
    pub fn is_grouping(&self) -> bool {
        self.active_group.is_some()
    }

    /// Clears both undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.active_group = None;
    }

    /// Drops the oldest entries when the undo stack exceeds max size.
    fn enforce_size_limit(&mut self) {
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used, clippy::cast_precision_loss)]

    use super::*;
    use crate::command::property::{SetFillCommand, SetOpacityCommand};
    use crate::scene::{BoundingBox, Color, SceneNode, SceneNodeKind};
    use selean_common::types::NodeId;

    fn make_frame(name: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        )
    }

    #[test]
    fn execute_pushes_to_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::new();

        let cmd = Box::new(SetFillCommand::new(id, Some(Color::WHITE)));
        assert!(history.execute(cmd, &mut scene));
        assert_eq!(history.undo_count(), 1);
        assert_eq!(history.redo_count(), 0);
    }

    #[test]
    fn undo_redo_cycle() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::new();

        let cmd = Box::new(SetOpacityCommand::new(id, 0.5));
        history.execute(cmd, &mut scene);
        assert_eq!(scene.get(id).unwrap().opacity, 0.5);

        // Undo.
        assert!(history.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().opacity, 1.0);
        assert_eq!(history.undo_count(), 0);
        assert_eq!(history.redo_count(), 1);

        // Redo.
        assert!(history.redo(&mut scene));
        assert_eq!(scene.get(id).unwrap().opacity, 0.5);
        assert_eq!(history.undo_count(), 1);
        assert_eq!(history.redo_count(), 0);
    }

    #[test]
    fn new_execute_clears_redo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::new();

        history.execute(Box::new(SetOpacityCommand::new(id, 0.5)), &mut scene);
        history.undo(&mut scene);
        assert!(history.can_redo());

        // New command clears redo.
        history.execute(
            Box::new(SetFillCommand::new(id, Some(Color::WHITE))),
            &mut scene,
        );
        assert!(!history.can_redo());
    }

    #[test]
    fn size_limit_drops_oldest() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::with_max_size(3);

        for i in 0..5 {
            let opacity = (i as f32) * 0.1;
            history.execute(Box::new(SetOpacityCommand::new(id, opacity)), &mut scene);
        }

        assert_eq!(history.undo_count(), 3);
    }

    #[test]
    fn can_undo_can_redo_states() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::new();

        assert!(!history.can_undo());
        assert!(!history.can_redo());

        history.execute(Box::new(SetOpacityCommand::new(id, 0.5)), &mut scene);
        assert!(history.can_undo());
        assert!(!history.can_redo());

        history.undo(&mut scene);
        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn group_creates_single_undo_entry() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::new();

        history.begin_group("Drag");
        history.execute(Box::new(SetOpacityCommand::new(id, 0.8)), &mut scene);
        history.execute(Box::new(SetOpacityCommand::new(id, 0.6)), &mut scene);
        history.execute(Box::new(SetOpacityCommand::new(id, 0.4)), &mut scene);
        history.end_group();

        // Three commands, but one undo entry.
        assert_eq!(history.undo_count(), 1);
        assert_eq!(scene.get(id).unwrap().opacity, 0.4);

        // Single undo reverts all three.
        assert!(history.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().opacity, 1.0);
    }

    #[test]
    fn cancel_group_rolls_back() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);
        let mut history = CommandHistory::new();

        history.begin_group("Drag");
        history.execute(Box::new(SetOpacityCommand::new(id, 0.5)), &mut scene);
        history.execute(
            Box::new(SetFillCommand::new(id, Some(Color::WHITE))),
            &mut scene,
        );

        // Cancel: should undo both commands, no undo entry created.
        history.cancel_group(&mut scene);
        assert_eq!(scene.get(id).unwrap().opacity, 1.0);
        assert!(scene.get(id).unwrap().fill.is_none());
        assert_eq!(history.undo_count(), 0);
        assert!(!history.is_grouping());
    }

    #[test]
    fn undo_redo_on_empty_returns_false() {
        let mut scene = SceneGraph::new();
        let mut history = CommandHistory::new();

        assert!(!history.undo(&mut scene));
        assert!(!history.redo(&mut scene));
    }
}
