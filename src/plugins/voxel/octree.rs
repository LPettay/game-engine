// Octree Address — Thin extension over QuadtreeAddress
// Most chunks sit at radial_index=0 (surface layer).
// Additional layers only where the player digs or subsurface features exist.
// Keeps memory budget manageable — NOT a full 3D octree.

use crate::plugins::quadtree::QuadtreeAddress;

/// Extends QuadtreeAddress with a radial (depth) index for 3D chunks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OctreeAddress {
    /// The surface-level quadtree address (face, depth, morton)
    pub surface_address: QuadtreeAddress,
    /// Radial index: 0 = surface layer, positive = above, negative = below
    /// Most chunks have radial_index = 0
    pub radial_index: i8,
}

impl OctreeAddress {
    /// Create an octree address for the surface layer
    pub fn surface(surface_address: QuadtreeAddress) -> Self {
        Self {
            surface_address,
            radial_index: 0,
        }
    }

    /// Create an address for a subsurface layer
    pub fn subsurface(surface_address: QuadtreeAddress, depth: i8) -> Self {
        Self {
            surface_address,
            radial_index: depth,
        }
    }

    /// Get the world-space center of this 3D chunk
    pub fn world_center_3d(&self, face_direction: bevy::prelude::Vec3, radius: f32) -> bevy::prelude::Vec3 {
        let surface_center = self.surface_address.world_center(face_direction, radius);
        let chunk_size = radius * 2.0 * self.surface_address.relative_size();

        // Offset radially from surface
        let radial_dir = surface_center.normalize();
        surface_center + radial_dir * (self.radial_index as f32 * chunk_size)
    }

    /// Get the world-space size of this chunk (same as surface chunk size)
    pub fn chunk_world_size(&self, radius: f32) -> f32 {
        radius * 2.0 * self.surface_address.relative_size()
    }

    /// Whether this is the surface layer (most common case)
    pub fn is_surface(&self) -> bool {
        self.radial_index == 0
    }

    /// Get adjacent radial layers
    pub fn above(&self) -> Self {
        Self {
            surface_address: self.surface_address,
            radial_index: self.radial_index + 1,
        }
    }

    pub fn below(&self) -> Self {
        Self {
            surface_address: self.surface_address,
            radial_index: self.radial_index - 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_address() {
        let qa = QuadtreeAddress::root(0);
        let oa = OctreeAddress::surface(qa);
        assert!(oa.is_surface());
        assert_eq!(oa.radial_index, 0);
    }

    #[test]
    fn test_subsurface_address() {
        let qa = QuadtreeAddress::root(0);
        let below = OctreeAddress::subsurface(qa, -1);
        assert!(!below.is_surface());
        assert_eq!(below.radial_index, -1);
    }

    #[test]
    fn test_above_below() {
        let qa = QuadtreeAddress::root(0);
        let surface = OctreeAddress::surface(qa);
        let above = surface.above();
        let below = surface.below();
        assert_eq!(above.radial_index, 1);
        assert_eq!(below.radial_index, -1);
    }
}
