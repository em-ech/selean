//! Draw command list for the hierarchical rendering pipeline.
//!
//! The [`DrawList`] collects [`DrawCommand`]s during Phase 2 (after DFS
//! traversal and UV finalization). Commands are emitted in tree order and
//! then coalesced by [`DrawList::merge_adjacent`] to minimize draw calls.

#![allow(clippy::missing_docs_in_private_items)]

use crate::scene::BlendMode;

/// A single GPU draw or state-change command.
///
/// Commands are iterated sequentially during the render pass. Draw commands
/// reference contiguous ranges in the ordered instance buffers.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub enum DrawCommand {
    /// Set the GPU scissor rectangle (pixel coordinates).
    SetScissor {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    /// Reset the scissor rectangle to the full viewport.
    ResetScissor,
    /// Set the stencil reference value for subsequent draw commands.
    SetStencilRef(u32),
    /// Draw rect instances into the stencil buffer (clip push).
    StencilWrite {
        instance_start: u32,
        instance_count: u32,
    },
    /// Draw rect instances to decrement the stencil buffer (clip pop).
    StencilDecrement {
        instance_start: u32,
        instance_count: u32,
    },
    /// Draw rectangle instances.
    DrawRects {
        instance_start: u32,
        instance_count: u32,
        blend: BlendMode,
        stencil_test: bool,
    },
    /// Draw glyph instances.
    DrawGlyphs {
        instance_start: u32,
        instance_count: u32,
        blend: BlendMode,
        stencil_test: bool,
    },
    /// Draw image quad instances.
    DrawImages {
        instance_start: u32,
        instance_count: u32,
        blend: BlendMode,
        stencil_test: bool,
    },
    /// Draw vector quad instances.
    DrawVectors {
        instance_start: u32,
        instance_count: u32,
        blend: BlendMode,
        stencil_test: bool,
    },
    /// Copy the current framebuffer to the snapshot texture.
    ///
    /// Inserted before rendering a non-native blend mode element to capture
    /// the destination state for the blend formula.
    CopyFramebuffer,
    /// Apply a shader-based blend mode composite.
    ///
    /// Inserted after rendering a non-native blend mode element. Reads the
    /// snapshot (dst) and current framebuffer (src), applies the blend formula,
    /// and writes the result.
    ApplyBlend { mode: BlendMode },
}

/// Ordered list of draw commands for a single frame.
///
/// Built during Phase 2 of the rendering pipeline. Call
/// [`merge_adjacent`](DrawList::merge_adjacent) after all commands are pushed
/// to coalesce contiguous draw ranges.
///
/// [`merge_adjacent`]: DrawList::merge_adjacent
pub struct DrawList {
    commands: Vec<DrawCommand>,
}

impl DrawList {
    /// Creates a new empty draw list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::with_capacity(256),
        }
    }

    /// Resets the draw list for a new frame.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Appends a draw command.
    pub fn push(&mut self, cmd: DrawCommand) {
        self.commands.push(cmd);
    }

    /// Returns the draw commands as a slice.
    #[must_use]
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// Returns the number of commands in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the draw list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Coalesces consecutive draw commands of the same type, blend mode, and
    /// stencil test state when their instance ranges are contiguous.
    ///
    /// State-change commands (`SetScissor`, `ResetScissor`, `SetStencilRef`,
    /// `StencilWrite`, `StencilDecrement`) act as merge barriers.
    pub fn merge_adjacent(&mut self) {
        if self.commands.len() < 2 {
            return;
        }

        let mut write = 0;
        for read in 1..self.commands.len() {
            let can_merge = Self::can_merge(&self.commands[write], &self.commands[read]);
            if can_merge {
                // Extract the count from the read command before mutating.
                let read_count = Self::instance_count(&self.commands[read]);
                Self::add_count(&mut self.commands[write], read_count);
            } else {
                write += 1;
                if write != read {
                    self.commands.swap(write, read);
                }
            }
        }
        self.commands.truncate(write + 1);
    }

    /// Returns `true` if two commands can be merged (same type, blend, stencil,
    /// and contiguous instance ranges).
    #[allow(clippy::similar_names)]
    fn can_merge(first: &DrawCommand, second: &DrawCommand) -> bool {
        let Some((key_a, end_a)) = Self::merge_key(first) else {
            return false;
        };
        let Some((key_b, _)) = Self::merge_key(second) else {
            return false;
        };
        let start_b = Self::start_of(second);
        key_a == key_b && end_a == start_b
    }

    /// Returns a merge key `(type_tag, blend, stencil_test)` and the end index
    /// (start + count) for mergeable draw commands. Non-draw commands return `None`.
    fn merge_key(cmd: &DrawCommand) -> Option<((u8, BlendMode, bool), u32)> {
        match cmd {
            DrawCommand::DrawRects {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => Some(((0, *blend, *stencil_test), instance_start + instance_count)),
            DrawCommand::DrawGlyphs {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => Some(((1, *blend, *stencil_test), instance_start + instance_count)),
            DrawCommand::DrawImages {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => Some(((2, *blend, *stencil_test), instance_start + instance_count)),
            DrawCommand::DrawVectors {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => Some(((3, *blend, *stencil_test), instance_start + instance_count)),
            _ => None,
        }
    }

    /// Returns the `instance_start` of a draw command (0 for non-draw commands).
    fn start_of(cmd: &DrawCommand) -> u32 {
        match cmd {
            DrawCommand::DrawRects { instance_start, .. }
            | DrawCommand::DrawGlyphs { instance_start, .. }
            | DrawCommand::DrawImages { instance_start, .. }
            | DrawCommand::DrawVectors { instance_start, .. } => *instance_start,
            _ => 0,
        }
    }

    /// Extracts the instance count from a draw command (0 for non-draw commands).
    fn instance_count(cmd: &DrawCommand) -> u32 {
        match cmd {
            DrawCommand::DrawRects { instance_count, .. }
            | DrawCommand::DrawGlyphs { instance_count, .. }
            | DrawCommand::DrawImages { instance_count, .. }
            | DrawCommand::DrawVectors { instance_count, .. } => *instance_count,
            _ => 0,
        }
    }

    /// Adds `extra` to the instance count of a draw command.
    fn add_count(cmd: &mut DrawCommand, extra: u32) {
        match cmd {
            DrawCommand::DrawRects { instance_count, .. }
            | DrawCommand::DrawGlyphs { instance_count, .. }
            | DrawCommand::DrawImages { instance_count, .. }
            | DrawCommand::DrawVectors { instance_count, .. } => {
                *instance_count += extra;
            }
            _ => {}
        }
    }
}

impl Default for DrawList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_draw_list() {
        let list = DrawList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.commands().is_empty());
    }

    #[test]
    fn single_command() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 5,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn merge_adjacent_same_type_contiguous() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 4,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 1);
        match &list.commands()[0] {
            DrawCommand::DrawRects {
                instance_start,
                instance_count,
                ..
            } => {
                assert_eq!(*instance_start, 0);
                assert_eq!(*instance_count, 7);
            }
            other => panic!("Expected DrawRects, got {other:?}"),
        }
    }

    #[test]
    fn no_merge_different_types() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawGlyphs {
            instance_start: 0,
            instance_count: 4,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn no_merge_different_blend() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 4,
            blend: BlendMode::Add,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn no_merge_different_stencil_test() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 4,
            blend: BlendMode::Normal,
            stencil_test: true,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn no_merge_non_contiguous_ranges() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 5,
            instance_count: 4,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn no_merge_across_set_scissor() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::SetScissor {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 4,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn no_merge_across_stencil_write() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 2,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::StencilWrite {
            instance_start: 0,
            instance_count: 1,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 2,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn merge_multiple_contiguous_glyphs() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawGlyphs {
            instance_start: 0,
            instance_count: 10,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawGlyphs {
            instance_start: 10,
            instance_count: 5,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawGlyphs {
            instance_start: 15,
            instance_count: 8,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 1);
        match &list.commands()[0] {
            DrawCommand::DrawGlyphs {
                instance_start,
                instance_count,
                ..
            } => {
                assert_eq!(*instance_start, 0);
                assert_eq!(*instance_count, 23);
            }
            other => panic!("Expected DrawGlyphs, got {other:?}"),
        }
    }

    #[test]
    fn complex_mixed_sequence() {
        let mut list = DrawList::new();
        // Mergeable rects
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 2,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 2,
            instance_count: 1,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        // Different type (barrier)
        list.push(DrawCommand::DrawImages {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        // Mergeable images
        list.push(DrawCommand::DrawImages {
            instance_start: 3,
            instance_count: 2,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        // Scissor barrier
        list.push(DrawCommand::ResetScissor);
        // Non-mergeable (after barrier)
        list.push(DrawCommand::DrawImages {
            instance_start: 5,
            instance_count: 1,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        // Expected: DrawRects(0..3), DrawImages(0..5), ResetScissor, DrawImages(5..6)
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn clear_resets() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 5,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.clear();
        assert!(list.is_empty());
    }

    #[test]
    fn merge_adjacent_on_empty_list() {
        let mut list = DrawList::new();
        list.merge_adjacent();
        assert!(list.is_empty());
    }

    #[test]
    fn no_merge_across_copy_framebuffer() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        list.push(DrawCommand::CopyFramebuffer);
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 4,
            blend: BlendMode::Multiply,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn no_merge_across_apply_blend() {
        let mut list = DrawList::new();
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Multiply,
            stencil_test: false,
        });
        list.push(DrawCommand::ApplyBlend {
            mode: BlendMode::Multiply,
        });
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 4,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn copy_and_apply_blend_sequence() {
        let mut list = DrawList::new();
        // Normal rect
        list.push(DrawCommand::DrawRects {
            instance_start: 0,
            instance_count: 2,
            blend: BlendMode::Normal,
            stencil_test: false,
        });
        // Non-native blend sequence
        list.push(DrawCommand::CopyFramebuffer);
        list.push(DrawCommand::DrawRects {
            instance_start: 2,
            instance_count: 1,
            blend: BlendMode::Screen,
            stencil_test: false,
        });
        list.push(DrawCommand::ApplyBlend {
            mode: BlendMode::Screen,
        });
        // More normal rects
        list.push(DrawCommand::DrawRects {
            instance_start: 3,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        });

        list.merge_adjacent();
        // Should be: DrawRects(Normal), CopyFramebuffer, DrawRects(Screen), ApplyBlend, DrawRects(Normal)
        assert_eq!(list.len(), 5);
    }
}
