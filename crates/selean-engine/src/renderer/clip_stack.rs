//! Hierarchical clip state stack for DFS scene traversal.
//!
//! During depth-first rendering, each clip node pushes state onto the stack
//! when entered and pops it when all children have been processed. The stack
//! tracks three independent clip mechanisms:
//! - **Scissor**: axis-aligned GPU scissor rects (intersected on push).
//! - **Stencil**: reference counter incremented/decremented per clip node.
//! - **ShaderRect**: per-instance clip rects (intersected on push).

use crate::scene::ClipRect;
use selean_common::types::NodeId;

/// Snapshot of the clip state at a point during DFS traversal.
///
/// Captured via [`ClipStack::resolve`] and stored alongside render order
/// entries so that Phase 2 can emit the correct draw commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedClipState {
    /// Intersection of all active `ShaderRect` ancestors.
    pub shader_clip_rect: ClipRect,
    /// Intersection of all active `Scissor` ancestors, or `None` if no scissor is active.
    pub scissor_rect: Option<ClipRect>,
    /// Number of active stencil clips (equals stencil reference value).
    pub stencil_ref: u32,
}

/// Stack that tracks nested clip state during hierarchical DFS rendering.
///
/// Each of the three clip modes maintains an independent stack. Push operations
/// intersect with the current top (for scissor and shader rect) or increment
/// a counter (for stencil).
#[allow(clippy::struct_field_names)]
pub struct ClipStack {
    /// Each entry is the intersected scissor rect at that nesting depth.
    scissor_entries: Vec<ClipRect>,
    /// Each entry is the node ID that pushed this stencil level.
    stencil_entries: Vec<NodeId>,
    /// Each entry is the intersected shader clip rect at that nesting depth.
    shader_rect_entries: Vec<ClipRect>,
}

impl ClipStack {
    /// Creates a new empty clip stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scissor_entries: Vec::new(),
            stencil_entries: Vec::new(),
            shader_rect_entries: Vec::new(),
        }
    }

    /// Resets the stack to empty (for reuse across frames).
    pub fn clear(&mut self) {
        self.scissor_entries.clear();
        self.stencil_entries.clear();
        self.shader_rect_entries.clear();
    }

    /// Pushes a scissor clip rect, intersecting with the current top.
    pub fn push_scissor(&mut self, rect: ClipRect) {
        let intersected = match self.scissor_entries.last() {
            Some(current) => current.intersect(&rect),
            None => rect,
        };
        self.scissor_entries.push(intersected);
    }

    /// Pops the most recent scissor clip rect.
    pub fn pop_scissor(&mut self) {
        self.scissor_entries.pop();
    }

    /// Pushes a stencil clip (increments stencil reference).
    pub fn push_stencil(&mut self, node_id: NodeId) {
        self.stencil_entries.push(node_id);
    }

    /// Pops the most recent stencil clip (decrements stencil reference).
    pub fn pop_stencil(&mut self) {
        self.stencil_entries.pop();
    }

    /// Pushes a shader rect clip, intersecting with the current top.
    pub fn push_shader_rect(&mut self, rect: ClipRect) {
        let intersected = match self.shader_rect_entries.last() {
            Some(current) => current.intersect(&rect),
            None => rect,
        };
        self.shader_rect_entries.push(intersected);
    }

    /// Pops the most recent shader rect clip.
    pub fn pop_shader_rect(&mut self) {
        self.shader_rect_entries.pop();
    }

    /// Returns the current scissor clip rect, or `None` if no scissor is active.
    #[must_use]
    pub fn current_scissor(&self) -> Option<&ClipRect> {
        self.scissor_entries.last()
    }

    /// Returns the current shader clip rect (top of stack or `INFINITE`).
    #[must_use]
    pub fn current_shader_rect(&self) -> ClipRect {
        self.shader_rect_entries
            .last()
            .copied()
            .unwrap_or(ClipRect::INFINITE)
    }

    /// Returns the current stencil reference value (number of active stencil clips).
    #[must_use]
    pub fn stencil_ref(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation)]
        let ref_val = self.stencil_entries.len() as u32;
        ref_val
    }

    /// Takes a snapshot of the current clip state for later use.
    #[must_use]
    pub fn resolve(&self) -> ResolvedClipState {
        ResolvedClipState {
            shader_clip_rect: self.current_shader_rect(),
            scissor_rect: self.current_scissor().copied(),
            stencil_ref: self.stencil_ref(),
        }
    }

    /// Returns the effective clip rect for viewport culling.
    ///
    /// This is the intersection of the current scissor rect (if any) and the
    /// current shader rect. Used during DFS to skip off-screen subtrees.
    #[must_use]
    pub fn effective_clip_rect(&self) -> ClipRect {
        let shader = self.current_shader_rect();
        match self.current_scissor() {
            Some(scissor) => scissor.intersect(&shader),
            None => shader,
        }
    }
}

impl Default for ClipStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn empty_stack_defaults() {
        let stack = ClipStack::new();
        assert_eq!(stack.current_scissor(), None);
        assert_eq!(stack.current_shader_rect(), ClipRect::INFINITE);
        assert_eq!(stack.stencil_ref(), 0);
    }

    #[test]
    fn resolve_empty_stack() {
        let stack = ClipStack::new();
        let state = stack.resolve();
        assert_eq!(state.shader_clip_rect, ClipRect::INFINITE);
        assert_eq!(state.scissor_rect, None);
        assert_eq!(state.stencil_ref, 0);
    }

    #[test]
    fn push_pop_scissor() {
        let mut stack = ClipStack::new();
        let rect = ClipRect::new(10.0, 20.0, 300.0, 400.0);

        stack.push_scissor(rect);
        assert_eq!(stack.current_scissor(), Some(&rect));

        stack.pop_scissor();
        assert_eq!(stack.current_scissor(), None);
    }

    #[test]
    fn nested_scissor_intersects() {
        let mut stack = ClipStack::new();
        stack.push_scissor(ClipRect::new(0.0, 0.0, 200.0, 200.0));
        stack.push_scissor(ClipRect::new(50.0, 50.0, 300.0, 300.0));

        let top = stack.current_scissor().unwrap();
        assert_eq!(top.min_x, 50.0);
        assert_eq!(top.min_y, 50.0);
        assert_eq!(top.max_x, 200.0);
        assert_eq!(top.max_y, 200.0);

        stack.pop_scissor();
        let restored = stack.current_scissor().unwrap();
        assert_eq!(restored.min_x, 0.0);
        assert_eq!(restored.max_x, 200.0);
    }

    #[test]
    fn push_pop_stencil() {
        let mut stack = ClipStack::new();
        let id1 = NodeId::new();
        let id2 = NodeId::new();

        assert_eq!(stack.stencil_ref(), 0);

        stack.push_stencil(id1);
        assert_eq!(stack.stencil_ref(), 1);

        stack.push_stencil(id2);
        assert_eq!(stack.stencil_ref(), 2);

        stack.pop_stencil();
        assert_eq!(stack.stencil_ref(), 1);

        stack.pop_stencil();
        assert_eq!(stack.stencil_ref(), 0);
    }

    #[test]
    fn push_pop_shader_rect() {
        let mut stack = ClipStack::new();
        let rect = ClipRect::new(100.0, 100.0, 500.0, 500.0);

        stack.push_shader_rect(rect);
        assert_eq!(stack.current_shader_rect(), rect);

        stack.pop_shader_rect();
        assert_eq!(stack.current_shader_rect(), ClipRect::INFINITE);
    }

    #[test]
    fn nested_shader_rect_intersects() {
        let mut stack = ClipStack::new();
        stack.push_shader_rect(ClipRect::new(0.0, 0.0, 400.0, 400.0));
        stack.push_shader_rect(ClipRect::new(100.0, 100.0, 600.0, 600.0));

        let current = stack.current_shader_rect();
        assert_eq!(current.min_x, 100.0);
        assert_eq!(current.min_y, 100.0);
        assert_eq!(current.max_x, 400.0);
        assert_eq!(current.max_y, 400.0);

        stack.pop_shader_rect();
        let restored = stack.current_shader_rect();
        assert_eq!(restored.min_x, 0.0);
        assert_eq!(restored.max_x, 400.0);
    }

    #[test]
    fn mixed_modes_simultaneously() {
        let mut stack = ClipStack::new();
        let id = NodeId::new();

        stack.push_scissor(ClipRect::new(0.0, 0.0, 800.0, 600.0));
        stack.push_stencil(id);
        stack.push_shader_rect(ClipRect::new(10.0, 10.0, 790.0, 590.0));

        let state = stack.resolve();
        assert_eq!(
            state.scissor_rect,
            Some(ClipRect::new(0.0, 0.0, 800.0, 600.0))
        );
        assert_eq!(state.stencil_ref, 1);
        assert_eq!(
            state.shader_clip_rect,
            ClipRect::new(10.0, 10.0, 790.0, 590.0)
        );

        stack.pop_shader_rect();
        stack.pop_stencil();
        stack.pop_scissor();

        let state = stack.resolve();
        assert_eq!(state.scissor_rect, None);
        assert_eq!(state.stencil_ref, 0);
        assert_eq!(state.shader_clip_rect, ClipRect::INFINITE);
    }

    #[test]
    fn effective_clip_rect_combines_scissor_and_shader() {
        let mut stack = ClipStack::new();
        stack.push_scissor(ClipRect::new(0.0, 0.0, 500.0, 500.0));
        stack.push_shader_rect(ClipRect::new(100.0, 100.0, 600.0, 600.0));

        let effective = stack.effective_clip_rect();
        assert_eq!(effective.min_x, 100.0);
        assert_eq!(effective.min_y, 100.0);
        assert_eq!(effective.max_x, 500.0);
        assert_eq!(effective.max_y, 500.0);
    }

    #[test]
    fn effective_clip_rect_no_scissor() {
        let mut stack = ClipStack::new();
        stack.push_shader_rect(ClipRect::new(50.0, 50.0, 200.0, 200.0));

        let effective = stack.effective_clip_rect();
        assert_eq!(effective, ClipRect::new(50.0, 50.0, 200.0, 200.0));
    }

    #[test]
    fn effective_clip_rect_no_shader() {
        let mut stack = ClipStack::new();
        stack.push_scissor(ClipRect::new(10.0, 10.0, 300.0, 300.0));

        let effective = stack.effective_clip_rect();
        assert_eq!(effective, ClipRect::new(10.0, 10.0, 300.0, 300.0));
    }

    #[test]
    fn effective_clip_rect_empty_stack() {
        let stack = ClipStack::new();
        assert_eq!(stack.effective_clip_rect(), ClipRect::INFINITE);
    }

    #[test]
    fn clear_resets_all_stacks() {
        let mut stack = ClipStack::new();
        stack.push_scissor(ClipRect::new(0.0, 0.0, 100.0, 100.0));
        stack.push_stencil(NodeId::new());
        stack.push_shader_rect(ClipRect::new(0.0, 0.0, 100.0, 100.0));

        stack.clear();
        assert_eq!(stack.current_scissor(), None);
        assert_eq!(stack.stencil_ref(), 0);
        assert_eq!(stack.current_shader_rect(), ClipRect::INFINITE);
    }

    #[test]
    fn triple_nested_stencil() {
        let mut stack = ClipStack::new();
        for _ in 0..3 {
            stack.push_stencil(NodeId::new());
        }
        assert_eq!(stack.stencil_ref(), 3);

        stack.pop_stencil();
        assert_eq!(stack.stencil_ref(), 2);

        stack.pop_stencil();
        stack.pop_stencil();
        assert_eq!(stack.stencil_ref(), 0);
    }

    #[test]
    fn deeply_nested_scissor_shrinks_monotonically() {
        let mut stack = ClipStack::new();
        stack.push_scissor(ClipRect::new(0.0, 0.0, 1000.0, 1000.0));
        stack.push_scissor(ClipRect::new(100.0, 100.0, 900.0, 900.0));
        stack.push_scissor(ClipRect::new(200.0, 200.0, 800.0, 800.0));

        let top = stack.current_scissor().unwrap();
        assert_eq!(top.min_x, 200.0);
        assert_eq!(top.max_x, 800.0);

        stack.pop_scissor();
        let mid = stack.current_scissor().unwrap();
        assert_eq!(mid.min_x, 100.0);
        assert_eq!(mid.max_x, 900.0);

        stack.pop_scissor();
        let base = stack.current_scissor().unwrap();
        assert_eq!(base.min_x, 0.0);
        assert_eq!(base.max_x, 1000.0);
    }
}
