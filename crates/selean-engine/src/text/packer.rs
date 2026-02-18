//! Shelf-packing algorithm for 2D rectangle allocation.
//!
//! Packs rectangles left-to-right in rows (shelves). When the current row
//! overflows, a new row is started below. When the atlas is full, the caller
//! is responsible for growing the backing store and calling [`ShelfPacker::grow`].
//!
//! This is a pure-geometry module with no GPU dependencies, making it
//! independently testable.

/// Result of a packing attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackResult {
    /// The rectangle was placed at these coordinates.
    Placed {
        /// Top-left X coordinate.
        x: u32,
        /// Top-left Y coordinate.
        y: u32,
    },
    /// The rectangle does not fit; the backing store needs to grow.
    Full,
}

/// A shelf-packing allocator for a 2D rectangular region.
///
/// Packs rectangles left-to-right with configurable padding. Rows have
/// variable height (determined by the tallest rectangle in the row).
#[derive(Debug, Clone)]
pub struct ShelfPacker {
    /// Current atlas width in texels.
    width: u32,
    /// Current atlas height in texels.
    height: u32,
    /// Current cursor X position in the current row.
    cursor_x: u32,
    /// Current cursor Y position (top of the current row).
    cursor_y: u32,
    /// Height of the current row (max padded height seen so far).
    row_height: u32,
    /// Padding between rectangles in texels.
    padding: u32,
}

impl ShelfPacker {
    /// Creates a new shelf packer for the given atlas dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32, padding: u32) -> Self {
        Self {
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            padding,
        }
    }

    /// Attempts to allocate a rectangle of the given dimensions.
    ///
    /// Returns [`PackResult::Placed`] with the top-left coordinates on success,
    /// or [`PackResult::Full`] if the atlas is full and needs to grow.
    pub fn allocate(&mut self, width: u32, height: u32) -> PackResult {
        let padded_w = width + self.padding;
        let padded_h = height + self.padding;

        // Try to fit in the current row.
        if self.cursor_x + padded_w <= self.width && self.cursor_y + padded_h <= self.height {
            let x = self.cursor_x;
            let y = self.cursor_y;
            self.cursor_x += padded_w;
            if padded_h > self.row_height {
                self.row_height = padded_h;
            }
            return PackResult::Placed { x, y };
        }

        // Try starting a new row.
        let new_row_y = self.cursor_y + self.row_height;
        if padded_w <= self.width && new_row_y + padded_h <= self.height {
            self.cursor_x = padded_w;
            self.cursor_y = new_row_y;
            self.row_height = padded_h;
            return PackResult::Placed { x: 0, y: new_row_y };
        }

        PackResult::Full
    }

    /// Notifies the packer that the backing store has been resized.
    ///
    /// Existing packing state (cursor, row) is preserved — the packer simply
    /// gains more room. The caller is responsible for copying existing data
    /// to the new backing store.
    pub fn grow(&mut self, new_width: u32, new_height: u32) {
        self.width = new_width;
        self.height = new_height;
    }

    /// Resets the packer to its initial empty state with new dimensions.
    pub fn reset(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    /// Returns the current atlas dimensions.
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_packer_has_correct_dimensions() {
        let packer = ShelfPacker::new(1024, 1024, 1);
        assert_eq!(packer.dimensions(), (1024, 1024));
    }

    #[test]
    fn single_allocation_placed() {
        let mut packer = ShelfPacker::new(100, 100, 1);
        let result = packer.allocate(48, 48);
        assert_eq!(result, PackResult::Placed { x: 0, y: 0 });
    }

    #[test]
    fn two_allocations_same_row() {
        let mut packer = ShelfPacker::new(200, 100, 1);
        let r1 = packer.allocate(48, 48);
        let r2 = packer.allocate(48, 48);
        assert_eq!(r1, PackResult::Placed { x: 0, y: 0 });
        // Second starts after first width + padding.
        assert_eq!(r2, PackResult::Placed { x: 49, y: 0 });
    }

    #[test]
    fn allocation_wraps_to_new_row() {
        let mut packer = ShelfPacker::new(100, 200, 1);
        // First glyph: 48+1=49 wide, fits in 100.
        let r1 = packer.allocate(48, 48);
        assert_eq!(r1, PackResult::Placed { x: 0, y: 0 });

        // Second glyph: cursor_x=49, needs 49 more = 98, fits in 100.
        let r2 = packer.allocate(48, 48);
        assert_eq!(r2, PackResult::Placed { x: 49, y: 0 });

        // Third glyph: cursor_x=98, needs 49 more = 147 > 100. New row.
        let r3 = packer.allocate(48, 48);
        // New row starts at y = 0 + 49 (row_height) = 49.
        assert_eq!(r3, PackResult::Placed { x: 0, y: 49 });
    }

    #[test]
    fn full_when_no_room() {
        let mut packer = ShelfPacker::new(50, 50, 1);
        // One 48x48 glyph fits (48+1=49 <= 50).
        assert_eq!(packer.allocate(48, 48), PackResult::Placed { x: 0, y: 0 });

        // Second doesn't fit in current row (49+49=98 > 50) or new row (49+49=98 > 50).
        assert_eq!(packer.allocate(48, 48), PackResult::Full);
    }

    #[test]
    fn grow_allows_more_allocations() {
        let mut packer = ShelfPacker::new(50, 50, 1);
        assert_eq!(packer.allocate(48, 48), PackResult::Placed { x: 0, y: 0 });
        assert_eq!(packer.allocate(48, 48), PackResult::Full);

        // Grow the atlas.
        packer.grow(200, 200);

        // Now the second allocation fits in the same row (cursor_x=49, 49+49=98 <= 200).
        assert_eq!(packer.allocate(48, 48), PackResult::Placed { x: 49, y: 0 });
    }

    #[test]
    fn reset_clears_state() {
        let mut packer = ShelfPacker::new(100, 100, 1);
        packer.allocate(48, 48);
        packer.allocate(48, 48);

        packer.reset(200, 200);
        assert_eq!(packer.dimensions(), (200, 200));

        // Should allocate from (0, 0) again.
        assert_eq!(packer.allocate(48, 48), PackResult::Placed { x: 0, y: 0 });
    }

    #[test]
    fn zero_padding_packs_tightly() {
        let mut packer = ShelfPacker::new(100, 100, 0);
        assert_eq!(packer.allocate(50, 50), PackResult::Placed { x: 0, y: 0 });
        assert_eq!(packer.allocate(50, 50), PackResult::Placed { x: 50, y: 0 });
        // Exactly fills the row, next goes to row 2.
        assert_eq!(packer.allocate(50, 50), PackResult::Placed { x: 0, y: 50 });
    }

    #[test]
    fn variable_height_rows() {
        let mut packer = ShelfPacker::new(200, 200, 1);

        // First row: 30px tall glyph, then 50px tall glyph.
        assert_eq!(packer.allocate(40, 30), PackResult::Placed { x: 0, y: 0 });
        assert_eq!(packer.allocate(40, 50), PackResult::Placed { x: 41, y: 0 });

        // Row height is now 51 (50+1 padding). Force new row with a wide glyph.
        // 150+1=151 > remaining (200-82=118), so wraps to new row at y=51.
        assert_eq!(
            packer.allocate(150, 30),
            PackResult::Placed { x: 0, y: 51 }
        );
    }

    #[test]
    fn many_small_glyphs_fill_rows() {
        let mut packer = ShelfPacker::new(100, 100, 1);
        let mut placed = 0;
        for _ in 0..100 {
            if packer.allocate(10, 10) == PackResult::Full {
                break;
            }
            placed += 1;
        }
        // 100 wide / 11 per glyph (10+1) = 9 per row
        // 100 tall / 11 per row = 9 rows
        // 9 * 9 = 81 glyphs
        assert_eq!(placed, 81);
    }
}
