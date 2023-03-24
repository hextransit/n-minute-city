use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use h3o::{CellIndex, LatLng, Resolution};

use super::cell::HexCell;

/// A H3 cell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct H3Cell {
    pub cell: CellIndex,
    pub layer: i16,
}

impl PartialOrd for H3Cell {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cell_hash().cmp(&other.cell_hash()))
    }
}

impl Ord for H3Cell {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cell_hash().cmp(&other.cell_hash())
    }
}

impl H3Cell {
    pub fn from_latlng(
        lat: f64,
        lng: f64,
        resolution: Resolution,
        layer: i16,
    ) -> anyhow::Result<Self> {
        Ok(H3Cell {
            cell: LatLng::new(lat, lng)?.to_cell(resolution),
            layer,
        })
    }

    pub fn cell_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.cell.hash(&mut h);
        0x0.hash(&mut h);
        self.layer.hash(&mut h);
        h.finish()
    }

    /// converts the H3 cell to a normal hexagon cell. The default origin is (0, 0)
    pub fn to_cell(&self, origin: Option<CellIndex>) -> anyhow::Result<HexCell> {
        let origin = match origin {
            Some(origin) => origin,
            None => LatLng::new(0.0, 0.0)?.to_cell(self.cell.resolution()),
        };
        let local_ij = self.cell.to_local_ij(origin)?;

        Ok(HexCell {
            a: local_ij.i() as i16,
            b: local_ij.j() as i16,
            radius: 1,
            layer: self.layer,
        })
    }
}
