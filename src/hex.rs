use bevy::math::{Mat2, Vec2};

/// Struct representing a hexagonal tile in axial coordinates. Uses pointy top orientation.
/// See: https://www.redblobgames.com/grids/hexagons/#basics
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Hex {
    q: i32,
    r: i32,
}

/// # Size parameter
/// In all methods below, `size` is the circumradius of the hex:
/// the distance from the hex center to any vertex.
impl Hex {
    /// Square root of 3 constant.
    const SQRT_3: f32 = 1.732_050_8;

    /// Directions to neighboring hexes in axial coordinates in (q,r) format. Starts from the right top
    /// neighbor and goes clockwise.
    const NEIGHBOR_DIR: [(i32, i32); 6] = [(1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1)];

    /// Matrix to convert axial coordinates to world coordinates.
    /// | √3   √3/2 |
    /// |  0   3/2  |
    const AXIAL_TO_WORLD_MATRIX: Mat2 =
        Mat2::from_cols_array(&[Self::SQRT_3, 0.0, Self::SQRT_3 / 2.0, 1.5]);

    /// Converts axial coordinates to world coordinates. Used for displaying hexes on the map.
    /// This assumes origin point at (0,0) - if the map origin is elsewhere, an offset should be applied.
    pub(crate) fn axial_to_world(&self, size: f32) -> Vec2 {
        Self::AXIAL_TO_WORLD_MATRIX.mul_vec2(Vec2::new(self.q as f32 * size, self.r as f32 * size))
    }

    /// Creates a new Hex with the given axial coordinates.
    pub(crate) fn new(q: i32, r: i32) -> Self {
        Hex { q, r }
    }

    /// Returns the q axial coordinate.
    pub(crate) fn q(&self) -> i32 {
        self.q
    }

    /// Returns the r axial coordinate.
    pub(crate) fn r(&self) -> i32 {
        self.r
    }

    /// Returns the neighboring hex in the specified direction (0 to 5). See [`Hex::NEIGHBOR_DIR`]
    /// for mapping.
    pub(crate) fn neighbor(&self, direction: usize) -> Hex {
        let (dq, dr) = Self::NEIGHBOR_DIR[direction % 6];
        Hex {
            q: self.q + dq,
            r: self.r + dr,
        }
    }

    pub(crate) fn neighbors(&self) -> Vec<Hex> {
        (0..6).map(|dir| self.neighbor(dir)).collect()
    }
}
