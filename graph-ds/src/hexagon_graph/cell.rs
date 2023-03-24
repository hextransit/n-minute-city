use serde::{Deserialize, Serialize};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    N,
    NE,
    SE,
    S,
    SW,
    NW,
    UP,
    DOWN,
}

/// A cell on a hexagonal grid.
/// Uses axial coordinates a and b, as described here: https://www.redblobgames.com/grids/hexagons/#coordinates-axial
///
/// The cell at (0, 0) is the center of the grid.
///
///  * a, b: axial coordinates
///  * radius: the size of each hexagon
///  * layer: for multi-layer grids, think of it as levels on buildings (implementation is not yet complete)
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HexCell {
    pub a: i16,
    pub b: i16,
    pub radius: i16,
    pub layer: i16,
}

impl PartialOrd for HexCell {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id().cmp(&other.id()))
    }
}

impl Ord for HexCell {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id().cmp(&other.id())
    }
}

impl HexCell {
    // byte representation [l l r r b b a a]
    pub fn id(&self) -> u64 {
        self.a
            .to_le_bytes()
            .iter()
            .chain(self.b.to_le_bytes().iter())
            .chain(self.radius.to_le_bytes().iter())
            .chain(self.layer.to_le_bytes().iter())
            .fold(0, |acc, &x| (acc << 8) | x as u64)
    }

    // always return the ID with the layer set to 0
    pub fn id_without_layer(&self) -> u64 {
        self.with_layer(0).id()
    }

    pub fn with_layer(mut self, layer: i16) -> Self {
        self.layer = layer;
        self
    }

    pub fn from_id(id: u64) -> HexCell {
        let layer = id.to_le_bytes()[0..2]
            .iter()
            .fold(0, |acc, &x| (acc << 8) | x as i16);
        let radius = id.to_le_bytes()[2..4]
            .iter()
            .fold(0, |acc, &x| (acc << 8) | x as i16);
        let b = id.to_le_bytes()[4..6]
            .iter()
            .fold(0, |acc, &x| (acc << 8) | x as i16);
        let a = id.to_le_bytes()[6..]
            .iter()
            .fold(0, |acc, &x| (acc << 8) | x as i16);
        HexCell {
            a,
            b,
            radius,
            layer,
        }
    }

    pub fn from_carthesian(x: f32, y: f32, r: f32) -> HexCell {
        let (a, b) = (
            (2.0 / 3.0) * x / r,
            (-x / 3.0 + (3.0_f32.sqrt() / 3.0) * y) / r,
        );

        let (a_round, b_round) = (a.round(), b.round());
        let (a, b) = (a - a_round, b - b_round);

        let center = if a.abs() >= b.abs() {
            ((a_round + f32::round(a + 0.5 * b)) as i32, b_round as i32)
        } else {
            (a_round as i32, (b_round + f32::round(b + 0.5 * a)) as i32)
        };

        HexCell {
            a: center.0 as i16,
            b: center.1 as i16,
            radius: r.round() as i16,
            layer: 0,
        }
    }

    pub fn as_carthesian(&self) -> (f32, f32) {
        (
            self.radius as f32 * (3.0 / 2.0) * self.a as f32,
            self.radius as f32
                * ((3.0_f32.sqrt() / 2.0) * self.a as f32 + 3.0_f32.sqrt() * self.b as f32),
        )
    }

    pub fn get_vertices(&self) -> [(f64, f64, f64); 6] {
        let (x, y) = self.as_carthesian();
        let (x, y) = (x as f64, y as f64);
        let mut vertices = [(0.0, 0.0, 0.0); 6];
        for (i, item) in vertices.iter_mut().enumerate() {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / 6.0;
            *item = (
                x + self.radius as f64 * angle.cos(),
                y + self.radius as f64 * angle.sin(),
                self.layer as f64,
            );
        }
        vertices
    }

    pub fn is_neighbor(&self, other: &HexCell) -> bool {
        if self.radius != other.radius {
            false
        } else {
            [
                Direction::N,
                Direction::NE,
                Direction::SE,
                Direction::S,
                Direction::SW,
                Direction::NW,
                Direction::UP,
                Direction::DOWN,
            ]
            .into_iter()
            .any(|dir| self.get_neighbor(dir) == *other)
        }
    }

    pub fn get_all_neighbors(&self) -> Vec<HexCell> {
        [
            Direction::N,
            Direction::NE,
            Direction::SE,
            Direction::S,
            Direction::SW,
            Direction::NW,
            Direction::UP,
            Direction::DOWN,
        ]
        .into_iter()
        .map(|dir| self.get_neighbor(dir))
        .collect()
    }

    pub fn get_neighbor(&self, direction: Direction) -> HexCell {
        match direction {
            Direction::N => HexCell {
                a: self.a,
                b: self.b + 1,
                radius: self.radius,
                layer: self.layer,
            },
            Direction::NE => HexCell {
                a: self.a + 1,
                b: self.b,
                radius: self.radius,
                layer: self.layer,
            },
            Direction::SE => HexCell {
                a: self.a + 1,
                b: self.b - 1,
                radius: self.radius,
                layer: self.layer,
            },
            Direction::S => HexCell {
                a: self.a,
                b: self.b - 1,
                radius: self.radius,
                layer: self.layer,
            },
            Direction::SW => HexCell {
                a: self.a - 1,
                b: self.b,
                radius: self.radius,
                layer: self.layer,
            },
            Direction::NW => HexCell {
                a: self.a - 1,
                b: self.b + 1,
                radius: self.radius,
                layer: self.layer,
            },
            Direction::UP => HexCell {
                a: self.a,
                b: self.b,
                radius: self.radius,
                layer: self.layer + 1,
            },
            Direction::DOWN => HexCell {
                a: self.a,
                b: self.b,
                radius: self.radius,
                layer: self.layer - 1,
            },
        }
    }
}
