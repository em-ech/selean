//! Command trait for the undo/redo system.
//!
//! Every mutation that should be undoable implements [`Command`]. The trait is
//! object-safe so commands can be stored as `Box<dyn Command>`.

use crate::scene::SceneGraph;

/// A reversible mutation on the scene graph.
///
/// Commands capture old state on [`execute`](Command::execute) and restore it
/// on [`undo`](Command::undo). Redo is implemented by calling `execute` again,
/// which re-captures the (now restored) old state.
pub trait Command: std::fmt::Debug {
    /// Applies the mutation, capturing old state for undo.
    ///
    /// Returns `true` if the mutation was applied successfully.
    /// Returns `false` if the target node does not exist or the
    /// mutation is not applicable (e.g., wrong node kind).
    fn execute(&mut self, scene: &mut SceneGraph) -> bool;

    /// Reverses the mutation using previously captured state.
    ///
    /// Returns `true` if the undo was applied successfully.
    /// Returns `false` if old state was not captured (execute was
    /// never called) or the scene is in an inconsistent state.
    fn undo(&mut self, scene: &mut SceneGraph) -> bool;

    /// A human-readable description of this command (e.g., "Set Fill").
    #[allow(clippy::unnecessary_literal_bound)]
    fn description(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockCommand {
        execute_count: u32,
        undo_count: u32,
    }

    impl MockCommand {
        fn new() -> Self {
            Self {
                execute_count: 0,
                undo_count: 0,
            }
        }
    }

    impl Command for MockCommand {
        fn execute(&mut self, _scene: &mut SceneGraph) -> bool {
            self.execute_count += 1;
            true
        }

        fn undo(&mut self, _scene: &mut SceneGraph) -> bool {
            self.undo_count += 1;
            true
        }

        fn description(&self) -> &str {
            "Mock command"
        }
    }

    #[test]
    fn mock_command_tracks_calls() {
        let mut cmd = MockCommand::new();
        let mut scene = SceneGraph::new();

        assert!(cmd.execute(&mut scene));
        assert!(cmd.execute(&mut scene));
        assert!(cmd.undo(&mut scene));

        assert_eq!(cmd.execute_count, 2);
        assert_eq!(cmd.undo_count, 1);
    }

    #[test]
    fn debug_format_works() {
        let cmd = MockCommand::new();
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("MockCommand"));
    }
}
