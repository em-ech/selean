//! Batch command grouping for the undo/redo system.
//!
//! `CommandGroup` wraps multiple commands into a single undoable unit.
//! Used for multi-step operations (e.g., drag gestures) that should
//! undo/redo as one atomic action.

use super::traits::Command;
use crate::scene::SceneGraph;

/// A group of commands that execute and undo as a single unit.
///
/// On execute, commands run in forward order. On undo, commands run in
/// reverse order. If any command fails during execute, all previously
/// executed commands in the group are rolled back.
#[derive(Debug)]
pub struct CommandGroup {
    label: String,
    commands: Vec<Box<dyn Command>>,
}

impl CommandGroup {
    /// Creates a new command group with the given label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            commands: Vec::new(),
        }
    }

    /// Adds a command to the group.
    pub fn push(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    /// Returns the number of commands in the group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the group has no commands.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Command for CommandGroup {
    fn execute(&mut self, scene: &mut SceneGraph) -> bool {
        if self.commands.is_empty() {
            return true;
        }

        for (i, cmd) in self.commands.iter_mut().enumerate() {
            if !cmd.execute(scene) {
                // Rollback already-executed commands in reverse.
                for cmd in self.commands[..i].iter_mut().rev() {
                    cmd.undo(scene);
                }
                return false;
            }
        }
        true
    }

    fn undo(&mut self, scene: &mut SceneGraph) -> bool {
        for cmd in self.commands.iter_mut().rev() {
            if !cmd.undo(scene) {
                return false;
            }
        }
        true
    }

    fn description(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::command::property::{SetFillCommand, SetOpacityCommand, SetVisibleCommand};
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
    fn group_of_three_commands_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut group = CommandGroup::new("Batch edit");
        group.push(Box::new(SetFillCommand::new(
            id,
            Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        )));
        group.push(Box::new(SetOpacityCommand::new(id, 0.5)));
        group.push(Box::new(SetVisibleCommand::new(id, false)));

        assert!(group.execute(&mut scene));
        assert!(scene.get(id).unwrap().fill.is_some());
        assert_eq!(scene.get(id).unwrap().opacity, 0.5);
        assert!(!scene.get(id).unwrap().visible);

        assert!(group.undo(&mut scene));
        assert!(scene.get(id).unwrap().fill.is_none());
        assert_eq!(scene.get(id).unwrap().opacity, 1.0);
        assert!(scene.get(id).unwrap().visible);
    }

    #[test]
    fn partial_failure_rollback() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut group = CommandGroup::new("Partial fail");
        // First command succeeds.
        group.push(Box::new(SetOpacityCommand::new(id, 0.5)));
        // Second command: set text content on a Frame node (will fail).
        group.push(Box::new(
            crate::command::property::SetTextContentCommand::new(id, "Bad".to_string()),
        ));

        assert!(!group.execute(&mut scene));
        // Opacity should be rolled back to original.
        assert_eq!(scene.get(id).unwrap().opacity, 1.0);
    }

    #[test]
    fn empty_group_no_op() {
        let mut scene = SceneGraph::new();
        let group = CommandGroup::new("Empty");
        assert!(group.is_empty());
        assert_eq!(group.len(), 0);

        let mut group = CommandGroup::new("Empty");
        assert!(group.execute(&mut scene));
        assert!(group.undo(&mut scene));
    }

    #[test]
    fn description_returns_label() {
        let group = CommandGroup::new("My batch operation");
        assert_eq!(group.description(), "My batch operation");
    }
}
