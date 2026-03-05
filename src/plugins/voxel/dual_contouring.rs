// Dual Contouring Mesh Extraction
// Two-pass algorithm:
//   1. Find cells with sign changes, solve QEF → one vertex per active cell
//   2. Connect vertices with quads along shared edges → triangle mesh

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use std::collections::HashMap;
use super::density::VoxelChunkData;

/// Edge crossing: interpolated position + normal at a sign-change edge
struct EdgeCrossing {
    position: Vec3,
    normal: Vec3,
}

/// QEF (Quadratic Error Function) solver for finding optimal vertex position.
/// Accumulates plane equations (point + normal) and solves for the point
/// that minimizes the sum of squared distances to all planes.
struct QefSolver {
    // AᵀA matrix (symmetric 3×3, stored as 6 values)
    ata_00: f64, ata_01: f64, ata_02: f64,
    ata_11: f64, ata_12: f64,
    ata_22: f64,
    // Aᵀb vector
    atb: [f64; 3],
    // Mass point (average of all intersection points, used as fallback)
    mass_point: Vec3,
    mass_count: u32,
}

impl QefSolver {
    fn new() -> Self {
        Self {
            ata_00: 0.0, ata_01: 0.0, ata_02: 0.0,
            ata_11: 0.0, ata_12: 0.0,
            ata_22: 0.0,
            atb: [0.0; 3],
            mass_point: Vec3::ZERO,
            mass_count: 0,
        }
    }

    /// Add a plane equation (intersection point + surface normal)
    fn add(&mut self, point: Vec3, normal: Vec3) {
        let n = [normal.x as f64, normal.y as f64, normal.z as f64];
        let d = (point.x * normal.x + point.y * normal.y + point.z * normal.z) as f64;

        // Accumulate AᵀA
        self.ata_00 += n[0] * n[0];
        self.ata_01 += n[0] * n[1];
        self.ata_02 += n[0] * n[2];
        self.ata_11 += n[1] * n[1];
        self.ata_12 += n[1] * n[2];
        self.ata_22 += n[2] * n[2];

        // Accumulate Aᵀb
        self.atb[0] += n[0] * d;
        self.atb[1] += n[1] * d;
        self.atb[2] += n[2] * d;

        // Mass point
        self.mass_point += point;
        self.mass_count += 1;
    }

    /// Solve for optimal vertex position via pseudo-inverse
    fn solve(&self) -> Vec3 {
        if self.mass_count == 0 {
            return Vec3::ZERO;
        }

        let mass_center = self.mass_point / self.mass_count as f32;

        // Try to solve the 3×3 system using Cramer's rule
        let a = self.ata_00;
        let b = self.ata_01;
        let c = self.ata_02;
        let d = self.ata_11;
        let e = self.ata_12;
        let f = self.ata_22;

        let det = a * (d * f - e * e)
                - b * (b * f - c * e)
                + c * (b * e - c * d);

        // If matrix is near-singular, fall back to mass point
        if det.abs() < 1e-10 {
            return mass_center;
        }

        let inv_det = 1.0 / det;

        // Inverse of symmetric 3×3
        let inv_00 = (d * f - e * e) * inv_det;
        let inv_01 = (c * e - b * f) * inv_det;
        let inv_02 = (b * e - c * d) * inv_det;
        let inv_11 = (a * f - c * c) * inv_det;
        let inv_12 = (b * c - a * e) * inv_det;
        let inv_22 = (a * d - b * b) * inv_det;

        let x = inv_00 * self.atb[0] + inv_01 * self.atb[1] + inv_02 * self.atb[2];
        let y = inv_01 * self.atb[0] + inv_11 * self.atb[1] + inv_12 * self.atb[2];
        let z = inv_02 * self.atb[0] + inv_12 * self.atb[1] + inv_22 * self.atb[2];

        let result = Vec3::new(x as f32, y as f32, z as f32);

        // Clamp: if result is too far from mass point, use mass point
        if (result - mass_center).length() > self.mass_point.length() * 0.1 {
            mass_center
        } else {
            result
        }
    }
}

/// Extract a mesh from voxel chunk data using dual contouring
pub fn extract_mesh(chunk: &VoxelChunkData) -> Option<Mesh> {
    let size = chunk.size;
    if size < 2 {
        return None;
    }

    // Pass 1: Find active cells (cells containing a sign change)
    // and solve QEF for each to get one vertex per cell
    let mut cell_vertices: HashMap<(u32, u32, u32), u32> = HashMap::new();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();

    let n = size - 1; // cells are between grid points

    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                // Check all 8 corners of this cell for sign changes
                let corners = [
                    (x, y, z),
                    (x + 1, y, z),
                    (x, y + 1, z),
                    (x + 1, y + 1, z),
                    (x, y, z + 1),
                    (x + 1, y, z + 1),
                    (x, y + 1, z + 1),
                    (x + 1, y + 1, z + 1),
                ];

                let densities: Vec<f32> = corners.iter()
                    .map(|&(cx, cy, cz)| chunk.density(cx, cy, cz))
                    .collect();

                // Check if there's a sign change in this cell
                let has_sign_change = densities.iter().any(|&d| d < 0.0)
                    && densities.iter().any(|&d| d >= 0.0);

                if !has_sign_change {
                    continue;
                }

                // Find edge crossings and accumulate QEF
                let mut qef = QefSolver::new();

                // Check all 12 edges of the cell for sign changes
                let edges: [(usize, usize); 12] = [
                    (0, 1), (2, 3), (4, 5), (6, 7), // X-aligned
                    (0, 2), (1, 3), (4, 6), (5, 7), // Y-aligned
                    (0, 4), (1, 5), (2, 6), (3, 7), // Z-aligned
                ];

                for &(a, b) in &edges {
                    let da = densities[a];
                    let db = densities[b];

                    if (da < 0.0) == (db < 0.0) {
                        continue; // No sign change on this edge
                    }

                    // Interpolate crossing position
                    let t = da / (da - db);
                    let pa = chunk.world_pos(corners[a].0, corners[a].1, corners[a].2);
                    let pb = chunk.world_pos(corners[b].0, corners[b].1, corners[b].2);
                    let crossing_pos = pa + (pb - pa) * t;

                    // Interpolate normal
                    let na = chunk.gradient(corners[a].0, corners[a].1, corners[a].2);
                    let nb = chunk.gradient(corners[b].0, corners[b].1, corners[b].2);
                    let crossing_normal = (na + (nb - na) * t).normalize_or_zero();

                    qef.add(crossing_pos, crossing_normal);
                }

                // Solve for optimal vertex
                let vertex = qef.solve();
                let normal = if qef.mass_count > 0 {
                    // Average normal from all edge crossings
                    let avg_normal = qef.mass_point.normalize_or_zero();
                    // Use the QEF mass point direction as a rough normal,
                    // but prefer the gradient at the vertex position
                    let grad = chunk.gradient(x, y, z);
                    if grad.length() > 0.1 { grad } else { avg_normal }
                } else {
                    Vec3::Y
                };

                let vertex_index = positions.len() as u32;
                positions.push([vertex.x, vertex.y, vertex.z]);
                normals.push([normal.x, normal.y, normal.z]);
                cell_vertices.insert((x, y, z), vertex_index);
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    // Pass 2: Connect vertices with quads along shared edges
    let mut indices: Vec<u32> = Vec::new();

    // For each internal edge, if it has a sign change, connect the 4 cells sharing it
    // X-aligned edges (shared by cells with same x, varying y and z)
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                // Check X-edge between (x,y,z) and (x+1,y,z)
                if y > 0 && z > 0 {
                    let d0 = chunk.density(x, y, z);
                    let d1 = chunk.density(x + 1, y, z);
                    if (d0 < 0.0) != (d1 < 0.0) {
                        // Connect 4 cells sharing this edge
                        if let (Some(&v0), Some(&v1), Some(&v2), Some(&v3)) = (
                            cell_vertices.get(&(x, y - 1, z - 1)),
                            cell_vertices.get(&(x, y, z - 1)),
                            cell_vertices.get(&(x, y, z)),
                            cell_vertices.get(&(x, y - 1, z)),
                        ) {
                            if d0 < 0.0 {
                                indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
                            } else {
                                indices.extend_from_slice(&[v0, v2, v1, v0, v3, v2]);
                            }
                        }
                    }
                }

                // Check Y-edge between (x,y,z) and (x,y+1,z)
                if x > 0 && z > 0 {
                    let d0 = chunk.density(x, y, z);
                    let d1 = chunk.density(x, y + 1, z);
                    if (d0 < 0.0) != (d1 < 0.0) {
                        if let (Some(&v0), Some(&v1), Some(&v2), Some(&v3)) = (
                            cell_vertices.get(&(x - 1, y, z - 1)),
                            cell_vertices.get(&(x, y, z - 1)),
                            cell_vertices.get(&(x, y, z)),
                            cell_vertices.get(&(x - 1, y, z)),
                        ) {
                            if d0 < 0.0 {
                                indices.extend_from_slice(&[v0, v2, v1, v0, v3, v2]);
                            } else {
                                indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
                            }
                        }
                    }
                }

                // Check Z-edge between (x,y,z) and (x,y,z+1)
                if x > 0 && y > 0 {
                    let d0 = chunk.density(x, y, z);
                    let d1 = chunk.density(x, y, z + 1);
                    if (d0 < 0.0) != (d1 < 0.0) {
                        if let (Some(&v0), Some(&v1), Some(&v2), Some(&v3)) = (
                            cell_vertices.get(&(x - 1, y - 1, z)),
                            cell_vertices.get(&(x, y - 1, z)),
                            cell_vertices.get(&(x, y, z)),
                            cell_vertices.get(&(x - 1, y, z)),
                        ) {
                            if d0 < 0.0 {
                                indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
                            } else {
                                indices.extend_from_slice(&[v0, v2, v1, v0, v3, v2]);
                            }
                        }
                    }
                }
            }
        }
    }

    if indices.is_empty() {
        return None;
    }

    // Generate UVs (spherical mapping from world position)
    let uvs: Vec<[f32; 2]> = positions.iter().map(|p| {
        let pos = Vec3::new(p[0], p[1], p[2]).normalize();
        let u = 0.5 + pos.z.atan2(pos.x) / (2.0 * std::f32::consts::PI);
        let v = 0.5 + pos.y.asin() / std::f32::consts::PI;
        [u, v]
    }).collect();

    // Generate vertex colors (from surface normal for basic shading)
    let colors: Vec<[f32; 4]> = normals.iter().map(|n| {
        let brightness = (n[1] * 0.5 + 0.5).clamp(0.2, 1.0);
        [brightness * 0.5, brightness * 0.7, brightness * 0.4, 1.0]
    }).collect();

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qef_solver_single_plane() {
        let mut qef = QefSolver::new();
        // Plane at y=5, normal pointing up
        qef.add(Vec3::new(0.0, 5.0, 0.0), Vec3::Y);
        qef.add(Vec3::new(1.0, 5.0, 0.0), Vec3::Y);
        qef.add(Vec3::new(0.0, 5.0, 1.0), Vec3::Y);

        let result = qef.solve();
        // Y should be close to 5.0
        assert!((result.y - 5.0).abs() < 0.1, "Expected y≈5.0, got {}", result.y);
    }
}
