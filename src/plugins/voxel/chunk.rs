// Voxel Modification Storage
// Stores sparse density deltas over the procedural base.
// Enables player terrain modification (dig/fill).

use bevy::prelude::*;
use std::collections::HashMap;
use super::octree::OctreeAddress;

/// Sparse voxel modification for a single chunk
#[derive(Clone, Debug)]
pub struct SparseModification {
    /// Density deltas keyed by local (x, y, z) grid coordinates
    /// Positive delta = more solid (fill), negative = more air (dig)
    pub deltas: HashMap<(u32, u32, u32), f32>,
}

impl SparseModification {
    pub fn new() -> Self {
        Self {
            deltas: HashMap::new(),
        }
    }

    /// Apply a density delta at a local grid position
    pub fn modify(&mut self, x: u32, y: u32, z: u32, delta: f32) {
        let entry = self.deltas.entry((x, y, z)).or_insert(0.0);
        *entry += delta;
    }

    /// Get the delta at a local grid position
    pub fn get_delta(&self, x: u32, y: u32, z: u32) -> f32 {
        self.deltas.get(&(x, y, z)).copied().unwrap_or(0.0)
    }

    /// Whether this chunk has any modifications
    pub fn is_empty(&self) -> bool {
        self.deltas.is_empty()
    }
}

/// Resource storing all voxel modifications across the world
#[derive(Resource, Default)]
pub struct VoxelModifications {
    pub chunks: HashMap<OctreeAddress, SparseModification>,
}

impl VoxelModifications {
    /// Dig (increase air) at a world position
    pub fn dig(&mut self, address: OctreeAddress, x: u32, y: u32, z: u32, strength: f32) {
        let mods = self.chunks.entry(address).or_insert_with(SparseModification::new);
        mods.modify(x, y, z, strength); // positive = more air
    }

    /// Fill (increase solid) at a world position
    pub fn fill(&mut self, address: OctreeAddress, x: u32, y: u32, z: u32, strength: f32) {
        let mods = self.chunks.entry(address).or_insert_with(SparseModification::new);
        mods.modify(x, y, z, -strength); // negative = more solid
    }

    /// Get modifications for a chunk
    pub fn get_modifications(&self, address: &OctreeAddress) -> Option<&SparseModification> {
        self.chunks.get(address)
    }

    /// Check if any modifications exist
    pub fn has_modifications(&self) -> bool {
        !self.chunks.is_empty()
    }
}

/// Modify terrain at a world position (converts world pos to chunk + local coords)
pub fn modify_terrain(
    modifications: &mut VoxelModifications,
    world_pos: Vec3,
    radius: f32,
    brush_size: f32,
    strength: f32,
    dig: bool,
) {
    // This is a placeholder for the full implementation.
    // The complete version would:
    // 1. Find which OctreeAddress contains this world position
    // 2. Convert world pos to local grid coordinates
    // 3. Apply brush (spherical area of effect)
    // 4. Mark affected chunks for re-meshing
    let _ = (modifications, world_pos, radius, brush_size, strength, dig);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::quadtree::QuadtreeAddress;

    #[test]
    fn test_sparse_modification() {
        let mut mods = SparseModification::new();
        assert!(mods.is_empty());

        mods.modify(5, 5, 5, 1.0);
        assert!(!mods.is_empty());
        assert_eq!(mods.get_delta(5, 5, 5), 1.0);

        // Accumulates
        mods.modify(5, 5, 5, 0.5);
        assert_eq!(mods.get_delta(5, 5, 5), 1.5);
    }

    #[test]
    fn test_voxel_modifications_resource() {
        let mut vm = VoxelModifications::default();
        let addr = OctreeAddress::surface(QuadtreeAddress::root(0));

        vm.dig(addr, 5, 5, 5, 1.0);
        assert!(vm.has_modifications());

        let mods = vm.get_modifications(&addr).unwrap();
        assert_eq!(mods.get_delta(5, 5, 5), 1.0);
    }
}
