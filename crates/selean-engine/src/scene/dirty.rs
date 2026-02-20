//! Dirty flag bitmask for tracking which node properties have changed.
//!
//! Each scene node carries a `DirtyFlags` value. The render loop processes
//! only nodes with non-zero flags, then clears them after rendering.

use std::fmt;

/// Bitmask tracking which properties of a scene node have changed since
/// the last render pass.
///
/// Multiple flags can be combined with bitwise OR. The renderer checks
/// which flags are set to determine what work needs to be done for each node.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DirtyFlags(u8);

impl DirtyFlags {
    /// No properties have changed.
    pub const NONE: Self = Self(0b0000_0000);
    /// Position or size changed — geometry buffers need updating.
    pub const GEOMETRY: Self = Self(0b0000_0001);
    /// Fill, stroke, opacity, or blend mode changed — material uniforms need updating.
    pub const STYLE: Self = Self(0b0000_0010);
    /// Child list changed — render order may need recalculating.
    pub const CHILDREN: Self = Self(0b0000_0100);
    /// Layout constraints changed — triggers layout recalculation.
    pub const LAYOUT: Self = Self(0b0000_1000);
    /// Text content or typography changed — glyph buffers need updating.
    pub const TEXT: Self = Self(0b0001_0000);
    /// Shadow, blur, or other effects changed — compositing pass affected.
    pub const EFFECTS: Self = Self(0b0010_0000);
    /// Local transform changed — world transform needs recomputation.
    /// Propagates *downward* to all descendants (unlike `CHILDREN` which goes up).
    pub const TRANSFORM: Self = Self(0b0100_0000);
    /// Clip mode changed — clipping state needs updating.
    pub const CLIP: Self = Self(0b1000_0000);
    /// All flags set — used when a node is newly created.
    pub const ALL: Self = Self(0b1111_1111);

    /// Returns `true` if no dirty flags are set.
    #[must_use]
    pub const fn is_clean(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if any dirty flags are set.
    #[must_use]
    pub const fn is_dirty(self) -> bool {
        self.0 != 0
    }

    /// Returns `true` if the given flag(s) are set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Sets the given flag(s), returning the updated flags.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Clears the given flag(s), returning the updated flags.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Clears all flags.
    #[must_use]
    pub const fn clear(self) -> Self {
        Self::NONE
    }

    /// Returns the raw bitmask value.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for DirtyFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for DirtyFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for DirtyFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl fmt::Debug for DirtyFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_clean() {
            return write!(f, "DirtyFlags(NONE)");
        }

        write!(f, "DirtyFlags(")?;
        let mut first = true;
        let flags = [
            (Self::GEOMETRY, "GEOMETRY"),
            (Self::STYLE, "STYLE"),
            (Self::CHILDREN, "CHILDREN"),
            (Self::LAYOUT, "LAYOUT"),
            (Self::TEXT, "TEXT"),
            (Self::EFFECTS, "EFFECTS"),
            (Self::TRANSFORM, "TRANSFORM"),
            (Self::CLIP, "CLIP"),
        ];

        for (flag, name) in flags {
            if self.contains(flag) {
                if !first {
                    write!(f, " | ")?;
                }
                write!(f, "{name}")?;
                first = false;
            }
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_clean() {
        let flags = DirtyFlags::default();
        assert!(flags.is_clean());
        assert!(!flags.is_dirty());
    }

    #[test]
    fn none_is_clean() {
        assert!(DirtyFlags::NONE.is_clean());
        assert!(!DirtyFlags::NONE.is_dirty());
    }

    #[test]
    fn single_flag_is_dirty() {
        assert!(DirtyFlags::GEOMETRY.is_dirty());
        assert!(!DirtyFlags::GEOMETRY.is_clean());
    }

    #[test]
    fn contains_checks_specific_flag() {
        let flags = DirtyFlags::GEOMETRY.with(DirtyFlags::STYLE);
        assert!(flags.contains(DirtyFlags::GEOMETRY));
        assert!(flags.contains(DirtyFlags::STYLE));
        assert!(!flags.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn contains_checks_combined_flags() {
        let flags = DirtyFlags::ALL;
        let subset = DirtyFlags::GEOMETRY.with(DirtyFlags::STYLE);
        assert!(flags.contains(subset));
    }

    #[test]
    fn with_combines_flags() {
        let flags = DirtyFlags::GEOMETRY.with(DirtyFlags::STYLE);
        assert_eq!(flags.bits(), 0b0000_0011);
    }

    #[test]
    fn without_clears_flags() {
        let flags = DirtyFlags::ALL.without(DirtyFlags::GEOMETRY);
        assert!(!flags.contains(DirtyFlags::GEOMETRY));
        assert!(flags.contains(DirtyFlags::STYLE));
        assert!(flags.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn clear_removes_all() {
        let flags = DirtyFlags::ALL.clear();
        assert!(flags.is_clean());
    }

    #[test]
    fn bitor_operator() {
        let a = DirtyFlags::GEOMETRY;
        let b = DirtyFlags::STYLE;
        let combined = a | b;
        assert!(combined.contains(DirtyFlags::GEOMETRY));
        assert!(combined.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn bitor_assign_operator() {
        let mut flags = DirtyFlags::GEOMETRY;
        flags |= DirtyFlags::TEXT;
        assert!(flags.contains(DirtyFlags::GEOMETRY));
        assert!(flags.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn bitand_operator() {
        let a = DirtyFlags::GEOMETRY.with(DirtyFlags::STYLE);
        let b = DirtyFlags::STYLE.with(DirtyFlags::TEXT);
        let intersection = a & b;
        assert!(intersection.contains(DirtyFlags::STYLE));
        assert!(!intersection.contains(DirtyFlags::GEOMETRY));
        assert!(!intersection.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn all_flag_contains_every_individual_flag() {
        assert!(DirtyFlags::ALL.contains(DirtyFlags::GEOMETRY));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::STYLE));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::CHILDREN));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::LAYOUT));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::TEXT));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::EFFECTS));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::TRANSFORM));
        assert!(DirtyFlags::ALL.contains(DirtyFlags::CLIP));
    }

    #[test]
    fn clip_flag() {
        let flags = DirtyFlags::CLIP;
        assert!(flags.contains(DirtyFlags::CLIP));
        assert!(!flags.contains(DirtyFlags::GEOMETRY));
        assert_eq!(flags.bits(), 0b1000_0000);
    }

    #[test]
    fn debug_format_clip() {
        assert_eq!(format!("{:?}", DirtyFlags::CLIP), "DirtyFlags(CLIP)");
    }

    #[test]
    fn debug_format_none() {
        assert_eq!(format!("{:?}", DirtyFlags::NONE), "DirtyFlags(NONE)");
    }

    #[test]
    fn debug_format_single() {
        assert_eq!(
            format!("{:?}", DirtyFlags::GEOMETRY),
            "DirtyFlags(GEOMETRY)"
        );
    }

    #[test]
    fn debug_format_combined() {
        let flags = DirtyFlags::GEOMETRY.with(DirtyFlags::TEXT);
        assert_eq!(format!("{flags:?}"), "DirtyFlags(GEOMETRY | TEXT)");
    }

    #[test]
    fn idempotent_with() {
        let flags = DirtyFlags::GEOMETRY.with(DirtyFlags::GEOMETRY);
        assert_eq!(flags.bits(), DirtyFlags::GEOMETRY.bits());
    }

    #[test]
    fn without_flag_not_present_is_noop() {
        let flags = DirtyFlags::GEOMETRY.without(DirtyFlags::TEXT);
        assert_eq!(flags.bits(), DirtyFlags::GEOMETRY.bits());
    }
}
