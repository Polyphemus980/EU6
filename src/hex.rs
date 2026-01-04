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

    /// Matrix to convert world coordinates to axial coordinates.
    /// | √3/3    -1/3 |
    /// |  0      2/3  |
    const WORLD_TO_AXIAL_MATRIX: Mat2 =
        Mat2::from_cols_array(&[Self::SQRT_3 / 3.0, 0.0, -1.0 / 3.0, 2.0 / 3.0]);

    /// The origin hex at (0,0).
    pub(crate) const ZERO: Hex = Hex { q: 0, r: 0 };

    /// Converts axial coordinates to world coordinates. Used for displaying hexes on the map.
    /// This assumes origin point at (0,0) - if the map origin is elsewhere, an offset should be applied.
    pub(crate) fn axial_to_world(&self, size: f32) -> Vec2 {
        Self::AXIAL_TO_WORLD_MATRIX.mul_vec2(Vec2::new(self.q as f32 * size, self.r as f32 * size))
    }

    /// Converts world coordinates to axial coordinates. Used for determining which hex contains a
    /// given point (e.g. mouse click).
    /// This assumes origin point at (0,0) - if the map origin is elsewhere, an offset should be applied.
    pub(crate) fn world_to_axial(world_pos: Vec2, size: f32) -> Hex {
        let axial_unrounded = Self::WORLD_TO_AXIAL_MATRIX.mul_vec2(world_pos) / size;
        Self::axial_round(axial_unrounded.x, axial_unrounded.y)
    }

    /// Rounds fractional axial coordinates to the nearest hex.
    fn axial_round(q: f32, r: f32) -> Hex {
        let rounded_q = q.round();
        let rounded_r = r.round();

        let q_diff = q - rounded_q;
        let r_diff = r - rounded_r;

        if q_diff.abs() > r_diff.abs() {
            Hex {
                q: rounded_q as i32 + (q_diff + 0.5 * r_diff).round() as i32,
                r: rounded_r as i32,
            }
        } else {
            Hex {
                q: rounded_q as i32,
                r: rounded_r as i32 + (r_diff + 0.5 * q_diff).round() as i32,
            }
        }
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

    /// Converts axial coordinates to cube coordinates.
    fn to_cube(&self) -> (i32, i32, i32) {
        let x = self.q;
        let z = self.r;
        let y = -x - z;
        (x, y, z)
    }

    /// Converts cube coordinates to axial coordinates.
    fn from_cube(x: i32, y: i32, z: i32) -> Hex {
        let q = x;
        let r = y;
        Hex { q, r }
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

    /// Computes the distance between two hexes.
    pub(crate) fn distance(&self, other: &Hex) -> i32 {
        let (x1, y1, z1) = self.to_cube();
        let (x2, y2, z2) = other.to_cube();
        ((x1 - x2).abs() + (y1 - y2).abs() + (z1 - z2).abs()) / 2
    }
}
