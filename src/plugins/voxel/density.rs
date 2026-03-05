// SDF Density Field Evaluation
// Converts existing TerrainNoise to a 3D signed distance field.
// Negative = solid, positive = air. Zero-crossing = surface.

use bevy::prelude::*;
use crate::plugins::terrain::TerrainNoise;

/// A 3D voxel chunk data grid
pub struct VoxelChunkData {
    /// Grid dimensions (size × size × size)
    pub size: u32,
    /// Density values (negative = solid, positive = air)
    pub densities: Vec<f32>,
    /// Gradient (normal direction) at each cell
    pub gradients: Vec<Vec3>,
    /// World-space origin of this chunk
    pub world_origin: Vec3,
    /// World-space size of each voxel cell
    pub cell_size: f32,
    /// Observer scale (meters per pixel) for detail level
    pub observer_scale: f32,
}

impl VoxelChunkData {
    /// Create a new chunk and evaluate the density field
    pub fn generate(
        terrain_noise: &TerrainNoise,
        planet_radius: f32,
        world_center: Vec3,
        chunk_world_size: f32,
        size: u32,
        observer_scale: f32,
    ) -> Self {
        let cell_size = chunk_world_size / size as f32;
        let half_extent = chunk_world_size * 0.5;
        let world_origin = world_center - Vec3::splat(half_extent);

        let total = (size * size * size) as usize;
        let mut densities = Vec::with_capacity(total);
        let mut gradients = Vec::with_capacity(total);

        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let local_pos = Vec3::new(
                        x as f32 * cell_size + cell_size * 0.5,
                        y as f32 * cell_size + cell_size * 0.5,
                        z as f32 * cell_size + cell_size * 0.5,
                    );
                    let world_pos = world_origin + local_pos;

                    let d = evaluate_density(
                        world_pos,
                        terrain_noise,
                        planet_radius,
                        observer_scale,
                    );
                    densities.push(d);

                    let grad = evaluate_gradient(
                        world_pos,
                        terrain_noise,
                        planet_radius,
                        observer_scale,
                        cell_size * 0.5,
                    );
                    gradients.push(grad);
                }
            }
        }

        Self {
            size,
            densities,
            gradients,
            world_origin,
            cell_size,
            observer_scale,
        }
    }

    /// Index into the flat arrays
    #[inline]
    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        (z * self.size * self.size + y * self.size + x) as usize
    }

    /// Get density at grid coordinates
    #[inline]
    pub fn density(&self, x: u32, y: u32, z: u32) -> f32 {
        self.densities[self.index(x, y, z)]
    }

    /// Get gradient at grid coordinates
    #[inline]
    pub fn gradient(&self, x: u32, y: u32, z: u32) -> Vec3 {
        self.gradients[self.index(x, y, z)]
    }

    /// World position of a grid cell center
    #[inline]
    pub fn world_pos(&self, x: u32, y: u32, z: u32) -> Vec3 {
        self.world_origin + Vec3::new(
            (x as f32 + 0.5) * self.cell_size,
            (y as f32 + 0.5) * self.cell_size,
            (z as f32 + 0.5) * self.cell_size,
        )
    }
}

/// Evaluate signed distance at a world position.
/// SDF = radial_distance - terrain_surface_radius
/// Negative = inside solid, Positive = air
pub fn evaluate_density(
    world_pos: Vec3,
    terrain_noise: &TerrainNoise,
    planet_radius: f32,
    observer_scale: f32,
) -> f32 {
    let radial_distance = world_pos.length();
    if radial_distance < 0.001 {
        return -planet_radius; // Deep inside planet
    }

    // Unit sphere point for noise lookup
    let unit_point = world_pos / radial_distance;

    // Get terrain elevation at this surface point using scale-dependent detail
    let elevation = terrain_noise.get_elevation_at_scale(unit_point, observer_scale);

    // Surface radius at this point: base radius * (1 + elevation * height_scale)
    let height_scale = 0.15; // matches planet generation
    let surface_radius = planet_radius * (1.0 + elevation * height_scale);

    // SDF: positive above surface (air), negative below (solid)
    radial_distance - surface_radius
}

/// Compute density gradient via central differences (surface normal direction)
pub fn evaluate_gradient(
    world_pos: Vec3,
    terrain_noise: &TerrainNoise,
    planet_radius: f32,
    observer_scale: f32,
    epsilon: f32,
) -> Vec3 {
    let dx = evaluate_density(
        world_pos + Vec3::X * epsilon,
        terrain_noise, planet_radius, observer_scale,
    ) - evaluate_density(
        world_pos - Vec3::X * epsilon,
        terrain_noise, planet_radius, observer_scale,
    );

    let dy = evaluate_density(
        world_pos + Vec3::Y * epsilon,
        terrain_noise, planet_radius, observer_scale,
    ) - evaluate_density(
        world_pos - Vec3::Y * epsilon,
        terrain_noise, planet_radius, observer_scale,
    );

    let dz = evaluate_density(
        world_pos + Vec3::Z * epsilon,
        terrain_noise, planet_radius, observer_scale,
    ) - evaluate_density(
        world_pos - Vec3::Z * epsilon,
        terrain_noise, planet_radius, observer_scale,
    );

    let grad = Vec3::new(dx, dy, dz);
    let len = grad.length();
    if len > 1e-8 {
        grad / len
    } else {
        world_pos.normalize() // fallback: radial direction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_density_above_surface_is_positive() {
        let noise = TerrainNoise::new(42);
        let planet_radius = 20000.0;

        // Point well above surface
        let pos = Vec3::new(0.0, planet_radius + 1000.0, 0.0);
        let d = evaluate_density(pos, &noise, planet_radius, 1000.0);
        assert!(d > 0.0, "Density above surface should be positive, got {}", d);
    }

    #[test]
    fn test_density_below_surface_is_negative() {
        let noise = TerrainNoise::new(42);
        let planet_radius = 20000.0;

        // Point well below surface
        let pos = Vec3::new(0.0, planet_radius - 1000.0, 0.0);
        let d = evaluate_density(pos, &noise, planet_radius, 1000.0);
        assert!(d < 0.0, "Density below surface should be negative, got {}", d);
    }
}
