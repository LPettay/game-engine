// Quadtree Chunk Subdivision System
// Enables infinite scale rendering by subdividing terrain chunks dynamically
// 
// Key concepts:
// - Each cube face starts as a single root chunk
// - Chunks subdivide into 4 children when observer gets close
// - Subdivision depth is unlimited (infinite LOD)
// - Parent features are inherited and refined at each level

use bevy::prelude::*;
use std::collections::HashMap;

/// Quadtree node address - uniquely identifies a chunk at any subdivision level
/// Uses Morton code (Z-order curve) for efficient spatial queries
/// 
/// Address format: [face_index:3][depth:5][morton_path:56]
/// - face_index: 0-5 for cube sphere faces
/// - depth: subdivision level (0 = root, max 31 for 64-bit address)
/// - morton_path: interleaved x,y bits at each level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuadtreeAddress {
    /// Face of the cube sphere (0-5)
    pub face: u8,
    /// Subdivision depth (0 = root covering entire face)
    pub depth: u8,
    /// Morton code encoding the path through the quadtree
    /// Each 2 bits encodes which quadrant (0=SW, 1=SE, 2=NW, 3=NE)
    pub morton: u64,
}

impl QuadtreeAddress {
    /// Create the root node for a face
    pub fn root(face: u8) -> Self {
        Self {
            face,
            depth: 0,
            morton: 0,
        }
    }
    
    /// Create a child at the given quadrant (0-3)
    /// 0 = bottom-left (SW), 1 = bottom-right (SE)
    /// 2 = top-left (NW), 3 = top-right (NE)
    pub fn child(&self, quadrant: u8) -> Self {
        debug_assert!(quadrant < 4, "Quadrant must be 0-3");
        debug_assert!(self.depth < 31, "Maximum subdivision depth reached");
        
        Self {
            face: self.face,
            depth: self.depth + 1,
            morton: (self.morton << 2) | (quadrant as u64),
        }
    }
    
    /// Get all 4 children of this node
    pub fn children(&self) -> [Self; 4] {
        [
            self.child(0),
            self.child(1),
            self.child(2),
            self.child(3),
        ]
    }
    
    /// Get the parent node (returns None for root)
    pub fn parent(&self) -> Option<Self> {
        if self.depth == 0 {
            None
        } else {
            Some(Self {
                face: self.face,
                depth: self.depth - 1,
                morton: self.morton >> 2,
            })
        }
    }
    
    /// Get all ancestors from root to this node (inclusive)
    pub fn ancestors(&self) -> Vec<Self> {
        let mut result = Vec::with_capacity(self.depth as usize + 1);
        let mut current = *self;
        result.push(current);
        while let Some(parent) = current.parent() {
            result.push(parent);
            current = parent;
        }
        result.reverse();
        result
    }
    
    /// Convert Morton code to (x, y) coordinates at this depth
    /// Returns coordinates in range [0, 2^depth)
    pub fn to_xy(&self) -> (u32, u32) {
        let mut x: u32 = 0;
        let mut y: u32 = 0;
        let mut morton = self.morton;
        
        for i in 0..self.depth {
            x |= ((morton & 1) as u32) << i;
            morton >>= 1;
            y |= ((morton & 1) as u32) << i;
            morton >>= 1;
        }
        
        (x, y)
    }
    
    /// Create address from (x, y) coordinates at given depth
    pub fn from_xy(face: u8, depth: u8, x: u32, y: u32) -> Self {
        let mut morton: u64 = 0;
        
        for i in 0..depth {
            morton |= (((x >> i) & 1) as u64) << (2 * i);
            morton |= (((y >> i) & 1) as u64) << (2 * i + 1);
        }
        
        Self { face, depth, morton }
    }
    
    /// Get the size of this chunk relative to the face (0.0 to 1.0)
    pub fn relative_size(&self) -> f32 {
        1.0 / (1u32 << self.depth) as f32
    }
    
    /// Get the center position of this chunk on the unit cube face
    /// Returns (x, y) in range [-1, 1]
    pub fn center_on_face(&self) -> (f32, f32) {
        let (ix, iy) = self.to_xy();
        let size = self.relative_size();
        let grid_size = 1u32 << self.depth;
        
        // Map from grid coordinates to [-1, 1] range
        let x = (ix as f32 + 0.5) / grid_size as f32 * 2.0 - 1.0;
        let y = (iy as f32 + 0.5) / grid_size as f32 * 2.0 - 1.0;
        
        (x, y)
    }
    
    /// Get the corner bounds of this chunk on the unit cube face
    /// Returns ((min_x, min_y), (max_x, max_y)) in range [-1, 1]
    pub fn bounds_on_face(&self) -> ((f32, f32), (f32, f32)) {
        let (ix, iy) = self.to_xy();
        let size = self.relative_size() * 2.0; // Size in [-1, 1] coordinate space
        let grid_size = 1u32 << self.depth;
        
        let min_x = ix as f32 / grid_size as f32 * 2.0 - 1.0;
        let min_y = iy as f32 / grid_size as f32 * 2.0 - 1.0;
        let max_x = min_x + size;
        let max_y = min_y + size;
        
        ((min_x, min_y), (max_x, max_y))
    }
    
    /// Calculate world position of chunk center on a sphere of given radius
    pub fn world_center(&self, face_direction: Vec3, radius: f32) -> Vec3 {
        let (cx, cy) = self.center_on_face();
        
        // Calculate axis vectors for this face
        let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let axis_b = face_direction.cross(axis_a);
        
        // Point on cube face
        let point_on_cube = face_direction + cx * axis_a + cy * axis_b;
        
        // Project to sphere
        point_on_cube.normalize() * radius
    }
    
    /// Check if observer is close enough to warrant subdivision
    pub fn should_subdivide(&self, observer_pos: Vec3, chunk_center: Vec3, radius: f32, subdivision_threshold: f32) -> bool {
        // Don't subdivide beyond maximum practical depth
        if self.depth >= 24 {
            return false;
        }
        
        // Calculate chunk size in world units
        let chunk_world_size = radius * 2.0 * self.relative_size();
        
        // Calculate distance from observer to chunk center
        let distance = (observer_pos - chunk_center).length();
        
        // Subdivide if observer is closer than threshold * chunk_size
        distance < chunk_world_size * subdivision_threshold
    }
    
    /// Check if this chunk should merge back into parent
    pub fn should_merge(&self, observer_pos: Vec3, parent_center: Vec3, radius: f32, merge_threshold: f32) -> bool {
        if self.depth == 0 {
            return false; // Root cannot merge
        }
        
        // Calculate parent chunk size
        let parent = self.parent().unwrap();
        let parent_world_size = radius * 2.0 * parent.relative_size();
        
        // Calculate distance from observer to parent center
        let distance = (observer_pos - parent_center).length();
        
        // Merge if observer is farther than threshold * parent_size
        distance > parent_world_size * merge_threshold
    }
}

/// State of a quadtree chunk in the terrain system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkState {
    /// Chunk is a leaf node and should be rendered
    Leaf,
    /// Chunk has been subdivided, children should be rendered instead
    Subdivided,
    /// Chunk is being generated
    Generating,
}

/// Quadtree node data for terrain chunks
#[derive(Debug, Clone)]
pub struct QuadtreeNode {
    pub address: QuadtreeAddress,
    pub state: ChunkState,
    pub entity: Option<Entity>,
    pub world_center: Vec3,
}

/// Manages the quadtree structure for a single planet
#[derive(Resource)]
pub struct QuadtreeManager {
    /// All nodes in the quadtree, keyed by address
    pub nodes: HashMap<QuadtreeAddress, QuadtreeNode>,
    /// Distance threshold for subdivision (multiplier of chunk size)
    pub subdivision_threshold: f32,
    /// Distance threshold for merging (multiplier of parent chunk size)
    pub merge_threshold: f32,
    /// Maximum depth of subdivision
    pub max_depth: u8,
    /// Planet radius
    pub radius: f32,
}

impl Default for QuadtreeManager {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            subdivision_threshold: 8.0,  // Subdivide when closer than 8x chunk size (very aggressive)
            merge_threshold: 12.0,       // Merge when farther than 12x parent size (hysteresis)
            max_depth: 20,               // Allow very deep subdivision for infinite scale
            radius: 20000.0,
        }
    }
}

impl QuadtreeManager {
    pub fn new(radius: f32) -> Self {
        Self {
            radius,
            ..Default::default()
        }
    }
    
    /// Initialize the quadtree with root nodes for each face
    pub fn initialize(&mut self) {
        let face_directions = [
            Vec3::Y,     // Top (+Y)
            Vec3::NEG_Y, // Bottom (-Y)
            Vec3::X,     // Right (+X)
            Vec3::NEG_X, // Left (-X)
            Vec3::Z,     // Front (+Z)
            Vec3::NEG_Z, // Back (-Z)
        ];
        
        for (face_idx, &face_dir) in face_directions.iter().enumerate() {
            let address = QuadtreeAddress::root(face_idx as u8);
            let world_center = address.world_center(face_dir, self.radius);
            
            self.nodes.insert(address, QuadtreeNode {
                address,
                state: ChunkState::Leaf,
                entity: None,
                world_center,
            });
        }
    }
    
    /// Get the face direction vector for a face index
    pub fn face_direction(face: u8) -> Vec3 {
        match face {
            0 => Vec3::Y,
            1 => Vec3::NEG_Y,
            2 => Vec3::X,
            3 => Vec3::NEG_X,
            4 => Vec3::Z,
            5 => Vec3::NEG_Z,
            _ => panic!("Invalid face index"),
        }
    }
    
    /// Update the quadtree based on observer position
    /// Returns lists of chunks to spawn and despawn, sorted by priority (closest first)
    ///
    /// `max_subdivisions` limits how many nodes can subdivide per call to prevent
    /// frame-long freezes when the camera jumps (e.g. Orbital → Surface).
    /// Each subdivision creates 4 children, so budget 8 = max 32 new nodes.
    /// Pass 0 for unlimited (original behavior).
    ///
    /// Candidates are sorted by distance before budgeting so the closest
    /// chunks always subdivide first — detail loads where the camera is.
    pub fn update(&mut self, observer_pos: Vec3, max_subdivisions: usize) -> (Vec<QuadtreeAddress>, Vec<QuadtreeAddress>) {
        let mut to_spawn: Vec<(QuadtreeAddress, f32)> = Vec::new(); // (address, distance)
        let mut to_despawn = Vec::new();

        // Collect addresses to process (can't mutate while iterating)
        let addresses: Vec<QuadtreeAddress> = self.nodes.keys().cloned().collect();

        // --- Pass 1: find subdivision candidates and process merges ---
        // Subdivision candidates are collected (not executed) so we can
        // sort by distance and spend the budget on the closest nodes.
        let mut subdiv_candidates: Vec<(QuadtreeAddress, f32)> = Vec::new(); // (address, distance)

        for address in addresses {
            let node = match self.nodes.get(&address) {
                Some(n) => n.clone(),
                None => continue,
            };

            let face_dir = Self::face_direction(node.address.face);

            match node.state {
                ChunkState::Leaf | ChunkState::Generating => {
                    // Check if should subdivide - allow subdivision even while generating
                    if node.address.depth < self.max_depth &&
                       node.address.should_subdivide(observer_pos, node.world_center, self.radius, self.subdivision_threshold) {
                        let distance = (observer_pos - node.world_center).length();
                        subdiv_candidates.push((address, distance));
                    }
                }
                ChunkState::Subdivided => {
                    // Merges are not budgeted — always process them
                    if let Some(parent_addr) = node.address.parent() {
                        let parent_center = parent_addr.world_center(face_dir, self.radius);

                        let should_merge = node.address.should_merge(
                            observer_pos,
                            parent_center,
                            self.radius,
                            self.merge_threshold
                        );

                        if should_merge && self.all_children_are_leaves(&node.address) {
                            for child_addr in node.address.children() {
                                if let Some(child_node) = self.nodes.get(&child_addr) {
                                    if child_node.entity.is_some() {
                                        to_despawn.push(child_addr);
                                    }
                                }
                                self.nodes.remove(&child_addr);
                            }

                            if let Some(n) = self.nodes.get_mut(&address) {
                                n.state = ChunkState::Leaf;
                            }
                            let distance = (observer_pos - node.world_center).length();
                            to_spawn.push((address, distance));
                        }
                    }
                }
            }
        }

        // --- Pass 2: sort candidates by distance, apply budget, execute ---
        subdiv_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let limit = if max_subdivisions > 0 {
            max_subdivisions
        } else {
            subdiv_candidates.len() // unlimited
        };

        for (address, _distance) in subdiv_candidates.into_iter().take(limit) {
            let node = match self.nodes.get(&address) {
                Some(n) => n.clone(),
                None => continue,
            };
            let face_dir = Self::face_direction(node.address.face);

            // Mark parent as subdivided (keep as fallback until children load)
            if let Some(n) = self.nodes.get_mut(&address) {
                n.state = ChunkState::Subdivided;
            }

            // Create children with distance for priority sorting
            for child_addr in node.address.children() {
                let child_center = child_addr.world_center(face_dir, self.radius);
                let dist = (observer_pos - child_center).length();
                self.nodes.insert(child_addr, QuadtreeNode {
                    address: child_addr,
                    state: ChunkState::Leaf,
                    entity: None,
                    world_center: child_center,
                });
                to_spawn.push((child_addr, dist));
            }
        }

        // Sort spawns by distance (closest chunks first for priority loading)
        to_spawn.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Extract just the addresses, now sorted by priority
        let sorted_spawn: Vec<QuadtreeAddress> = to_spawn.into_iter().map(|(addr, _)| addr).collect();

        (sorted_spawn, to_despawn)
    }
    
    /// Check if all children of a node are leaf nodes
    fn all_children_are_leaves(&self, address: &QuadtreeAddress) -> bool {
        for child_addr in address.children() {
            match self.nodes.get(&child_addr) {
                Some(node) => {
                    if node.state != ChunkState::Leaf {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
    
    /// Check if all children of a subdivided node have loaded entities (meshes ready)
    pub fn all_children_loaded(&self, address: &QuadtreeAddress) -> bool {
        for child_addr in address.children() {
            match self.nodes.get(&child_addr) {
                Some(node) => {
                    // Child must have an entity (mesh loaded)
                    if node.entity.is_none() {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
    
    /// Get addresses of subdivided nodes that still have entities (parents acting as fallback)
    pub fn get_fallback_parents(&self) -> Vec<QuadtreeAddress> {
        self.nodes.values()
            .filter(|n| n.state == ChunkState::Subdivided && n.entity.is_some())
            .map(|n| n.address)
            .collect()
    }
    
    /// Get all leaf nodes that should be rendered
    pub fn get_visible_chunks(&self) -> Vec<&QuadtreeNode> {
        self.nodes.values()
            .filter(|n| n.state == ChunkState::Leaf)
            .collect()
    }
    
    /// Mark a chunk as having an entity
    pub fn set_entity(&mut self, address: QuadtreeAddress, entity: Entity) {
        if let Some(node) = self.nodes.get_mut(&address) {
            node.entity = Some(entity);
        }
    }
    
    /// Mark a chunk as generating
    pub fn set_generating(&mut self, address: QuadtreeAddress) {
        if let Some(node) = self.nodes.get_mut(&address) {
            node.state = ChunkState::Generating;
        }
    }
    
    /// Mark a chunk as done generating (back to leaf)
    pub fn set_generated(&mut self, address: QuadtreeAddress) {
        if let Some(node) = self.nodes.get_mut(&address) {
            node.state = ChunkState::Leaf;
        }
    }
    
    /// Calculate observer scale (meters per "unit") for a given depth
    pub fn observer_scale_for_depth(&self, depth: u8) -> f32 {
        // At depth 0, one chunk covers the whole face (diameter of planet)
        // At each depth level, scale halves
        let face_size = self.radius * 2.0;
        face_size / (1u32 << depth) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quadtree_address_children() {
        let root = QuadtreeAddress::root(0);
        assert_eq!(root.depth, 0);
        
        let children = root.children();
        assert_eq!(children.len(), 4);
        
        for child in &children {
            assert_eq!(child.depth, 1);
            assert_eq!(child.parent(), Some(root));
        }
    }
    
    #[test]
    fn test_morton_xy_conversion() {
        let addr = QuadtreeAddress::from_xy(0, 3, 5, 3);
        let (x, y) = addr.to_xy();
        assert_eq!(x, 5);
        assert_eq!(y, 3);
    }
    
    #[test]
    fn test_relative_size() {
        let root = QuadtreeAddress::root(0);
        assert_eq!(root.relative_size(), 1.0);

        let child = root.child(0);
        assert_eq!(child.relative_size(), 0.5);

        let grandchild = child.child(0);
        assert_eq!(grandchild.relative_size(), 0.25);
    }

    /// Helper: create a QuadtreeManager, initialize it, and return it
    fn make_manager() -> QuadtreeManager {
        let mut m = QuadtreeManager::new(20000.0);
        m.initialize();
        m
    }

    #[test]
    fn test_subdivision_budget_limits_growth() {
        let mut m = make_manager();
        // Observer right on the surface — triggers aggressive subdivision
        let surface = Vec3::Y * 20001.0;
        let (spawned, _) = m.update(surface, 2);
        // Budget 2: at most 2 subdivisions × 4 children = 8 spawns
        assert!(spawned.len() <= 8, "Budget 2 should produce at most 8 spawns, got {}", spawned.len());
    }

    #[test]
    fn test_unlimited_budget_subdivides_freely() {
        let mut m = make_manager();
        let surface = Vec3::Y * 20001.0;
        let (spawned_unlimited, _) = m.update(surface, 0);
        // With budget 0 (unlimited), many more nodes should subdivide in one pass
        assert!(spawned_unlimited.len() >= 4, "Unlimited should subdivide at least one node");
    }

    #[test]
    fn test_closest_nodes_subdivide_first() {
        let mut m = make_manager();
        // Observer near the +Y face surface
        let surface = Vec3::Y * 20001.0;
        // Budget of 1: only the single closest candidate should subdivide
        let (spawned, _) = m.update(surface, 1);
        assert!(spawned.len() <= 4, "Budget 1 = at most 4 children from 1 subdivision");
        // The spawned children should be from the face closest to the observer (+Y face = face 0)
        for addr in &spawned {
            assert_eq!(addr.face, 0, "Budget 1 near +Y should subdivide face 0 first, got face {}", addr.face);
        }
    }
}

