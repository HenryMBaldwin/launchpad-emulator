//! Addressing of the pads that make up a surface.

/// A position on a surface, with the origin at the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pad {
    /// Column, counting from 0 at the left edge.
    pub x: u8,
    /// Row, counting from 0 at the top edge.
    pub y: u8,
}

impl Pad {
    /// Creates a pad at the given coordinates.
    #[must_use]
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    /// Every pad of a `width` by `height` surface, in row-major order.
    pub fn all(width: u8, height: u8) -> impl Iterator<Item = Self> {
        (0..height).flat_map(move |y| (0..width).map(move |x| Self { x, y }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_walks_the_surface_in_row_major_order() {
        let pads: Vec<Pad> = Pad::all(3, 2).collect();
        assert_eq!(pads.len(), 6);
        assert_eq!(pads[0], Pad::new(0, 0));
        assert_eq!(pads[2], Pad::new(2, 0));
        assert_eq!(pads[3], Pad::new(0, 1));
    }
}
