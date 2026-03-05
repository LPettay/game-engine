// Voxel Terrain with Dual Contouring
// Replaces 2D heightmap mesh generation with 3D density field + dual contouring.
// Enables overhangs, caves, and player terrain modification.
// TOGGLEABLE — old heightmap path preserved via `use_voxels` flag.

pub mod density;
pub mod dual_contouring;
pub mod octree;
pub mod chunk;

use bevy::prelude::*;

pub struct VoxelTerrainPlugin;

impl Plugin for VoxelTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelConfig>()
            .init_resource::<chunk::VoxelModifications>();
    }
}

/// Configuration for voxel terrain system
#[derive(Resource)]
pub struct VoxelConfig {
    /// Voxel grid cells per chunk axis (32 or 64)
    pub chunk_size: u32,
    /// Enable voxel terrain (false = old heightmap path)
    pub use_voxels: bool,
    /// Allow player terrain modification
    pub modification_enabled: bool,
    /// Use GPU compute for density field generation
    pub gpu_density: bool,
}

impl Default for VoxelConfig {
    fn default() -> Self {
        Self {
            chunk_size: 32,
            use_voxels: false, // Off by default — opt-in
            modification_enabled: false,
            gpu_density: false,
        }
    }
}
