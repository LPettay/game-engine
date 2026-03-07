use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::camera::visibility::VisibilitySystems;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy_rapier3d::prelude::*;
use bevy_rapier3d::prelude::{ReadRapierContext, QueryFilter};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use futures_lite::future;
use crate::plugins::terrain::TerrainNoise;
use crate::plugins::planet::{PlanetSettings, PlanetMaterial, Planet};
use crate::plugins::gpu_terrain::{GpuTerrainSettings, GpuChunkQueue, GpuChunkRequest};
use crate::plugins::quadtree::{QuadtreeManager, QuadtreeAddress, ChunkState};
use crate::plugins::voxel::{VoxelConfig, density::VoxelChunkData, dual_contouring};
use crate::GameState;

pub struct ChunkedTerrainPlugin;

impl Plugin for ChunkedTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkManager>()
           .add_systems(OnEnter(GameState::Playing), initialize_chunked_terrain)
           .add_systems(Update, (
               sync_detail_texture,
               hide_old_planet_mesh,
               process_chunk_generation_tasks.after(sync_detail_texture),
               process_quadtree_chunk_tasks.after(process_chunk_generation_tasks),
           ).run_if(in_state(GameState::Playing)))
           .add_systems(PostUpdate, (
               // Use quadtree system if enabled, otherwise fall back to old system
               update_quadtree_chunks.run_if(quadtree_enabled),
               manage_parent_chunk_visibility.run_if(quadtree_enabled).after(update_quadtree_chunks),
               // Legacy LOD system removed — quadtree is always active
               // Temporarily disabled: frustum culling is causing chunks to be incorrectly culled
               // The chunk loading/unloading system already handles visibility correctly
               // cull_chunks_outside_frustum,
           ).run_if(in_state(GameState::Playing)).after(VisibilitySystems::CheckVisibility));
    }
}

// Run condition: quadtree is enabled
fn quadtree_enabled(chunk_manager: Res<ChunkManager>) -> bool {
    chunk_manager.use_quadtree
}

// Run condition: quadtree is disabled (use old system)

// System to sync detail texture from planet material to ChunkManager and create shared material
fn sync_detail_texture(
    mut chunk_manager: ResMut<ChunkManager>,
    planet_material_query: Query<&MeshMaterial3d<PlanetMaterial>, With<Planet>>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
) {
    // Only update if we don't have a texture yet
    if chunk_manager.detail_texture_handle.is_none() {
        if let Ok(material_handle) = planet_material_query.single() {
            if let Some(material) = materials.get(&material_handle.0) {
                chunk_manager.detail_texture_handle = Some(material.detail_texture.clone());
            }
        }
    }
    
    // Create shared material if we don't have one yet
    if chunk_manager.shared_material_handle.is_none() {
        let detail_texture = chunk_manager.detail_texture_handle.clone().unwrap_or_default();
        let shared_material = materials.add(PlanetMaterial {
            scaling: Vec4::new(0.02, 0.0, 0.95, 0.05),
            detail_texture,
        });
        chunk_manager.shared_material_handle = Some(shared_material);
    }
}

// System to hide the old planet mesh when chunks are loaded
fn hide_old_planet_mesh(
    mut planet_query: Query<&mut Visibility, (With<Planet>, Without<TerrainChunk>)>,
    _chunk_query: Query<&TerrainChunk>,
) {
    // Always hide the old planet mesh - disable it entirely for now
    // This ensures the old mesh never shows, even when chunks are unloading
    for mut visibility in planet_query.iter_mut() {
        *visibility = Visibility::Hidden;
    }
}

// Chunk identifier: (face_index, lod_level, chunk_x, chunk_y)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId {
    face_index: u8,  // 0-5 for cube sphere faces
    lod_level: u32,   // 0 = highest detail
    chunk_x: i32,
    chunk_y: i32,
}

// Chunk state
#[derive(Component)]
pub struct TerrainChunk {
    pub id: ChunkId,
    pub world_position: Vec3,
    pub mesh_handle: Option<Handle<Mesh>>,
    pub collider_handle: Option<Entity>,
    pub is_loaded: bool,
    pub is_generating: bool,
}

// Generation task for async chunk creation
#[derive(Component)]
struct ChunkGenerationTask {
    #[allow(dead_code)]
    id: ChunkId, // Used for debugging/logging
    task: Task<(Mesh, Option<Collider>)>,
}

#[derive(Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<ChunkId, Entity>,
    max_lod_levels: u32,
    pub base_chunk_resolution: u32,  // Resolution at LOD 0
    planet_radius: f32,
    pub terrain_seed: u32,
    max_chunks_per_frame: usize,
    pub max_active_chunks: usize,
    pub detail_texture_handle: Option<Handle<Image>>, // Store texture handle for chunks
    pub shared_material_handle: Option<Handle<PlanetMaterial>>, // Shared material for all chunks
    current_lod_per_face: [Option<u32>; 6], // Track current LOD per face to prevent flickering
    active_faces: HashSet<usize>, // Track currently active faces for unloading
    chunk_unload_cooldown: HashMap<ChunkId, u32>, // Track frames since chunk became inactive
    pub chunk_render_radius: f32, // Radius (in chunks) for circular render area
    
    // Quadtree system for infinite scale rendering
    pub use_quadtree: bool,          // Enable quadtree-based infinite LOD
    pub quadtree: QuadtreeManager,   // The quadtree itself
    pub quadtree_chunks: HashMap<QuadtreeAddress, Entity>, // Quadtree chunk entities
    pub max_subdivisions_per_frame: usize, // Budget: max quadtree subdivisions per update
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self {
            chunks: HashMap::new(),
            max_lod_levels: 8,  // More LOD levels for smoother transitions
            base_chunk_resolution: 256,  // INCREASED: 256x256 vertices per chunk for hyperreal detail
            planet_radius: 20000.0,
            terrain_seed: 12345,
            max_chunks_per_frame: 16,  // Increased for faster close-range detail loading
            max_active_chunks: 384,    // More chunks for aggressive subdivision
            detail_texture_handle: None,
            shared_material_handle: None,
            current_lod_per_face: [None; 6],
            active_faces: HashSet::new(),
            chunk_unload_cooldown: HashMap::new(),
            chunk_render_radius: 2.5, // Larger radius for better coverage
            
            // Quadtree system - enabled by default for infinite scale
            use_quadtree: true,
            quadtree: QuadtreeManager::default(),
            quadtree_chunks: HashMap::new(),
            max_subdivisions_per_frame: 8, // 8 subdivisions × 4 children = max 32 new nodes/frame
        }
    }
}

fn initialize_chunked_terrain(
    mut commands: Commands,
    planet_settings: Res<PlanetSettings>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    // Cache TerrainNoise as a resource so consumers don't recreate it per-frame
    commands.insert_resource(TerrainNoise::new(planet_settings.terrain_seed));

    chunk_manager.planet_radius = planet_settings.radius;
    chunk_manager.terrain_seed = planet_settings.terrain_seed;
    
    // Debug: Print actual settings being used
    println!("========================================");
    println!("[Terrain] SETTINGS DEBUG:");
    println!("  base_chunk_resolution: {}", chunk_manager.base_chunk_resolution);
    println!("  max_active_chunks: {}", chunk_manager.max_active_chunks);
    println!("  max_chunks_per_frame: {}", chunk_manager.max_chunks_per_frame);
    println!("  use_quadtree: {}", chunk_manager.use_quadtree);
    println!("========================================");
    
    // Initialize quadtree if enabled
    if chunk_manager.use_quadtree {
        chunk_manager.quadtree = QuadtreeManager::new(planet_settings.radius);
        chunk_manager.quadtree.initialize();
        
        println!("[Quadtree] subdivision_threshold: {}", chunk_manager.quadtree.subdivision_threshold);
        println!("[Quadtree] merge_threshold: {}", chunk_manager.quadtree.merge_threshold);
        
        info!("[Quadtree] Initialized infinite scale terrain system with {} root nodes", 
              chunk_manager.quadtree.nodes.len());
    }
    
    // Planet entity should already exist from PlanetPlugin
    // We don't need to spawn a new one - chunks will be parented to existing Planet entity
}

/// Component to identify quadtree-based terrain chunks
#[derive(Component)]
pub struct QuadtreeChunk {
    pub address: QuadtreeAddress,
    pub depth: u8,
    pub observer_scale: f32,
}

/// Task for generating quadtree chunks asynchronously
#[derive(Component)]
struct QuadtreeChunkTask {
    address: QuadtreeAddress,
    task: Task<(Mesh, Option<Collider>)>,
}

/// System to update quadtree chunks based on camera position
/// This enables infinite LOD by subdividing/merging chunks dynamically
fn update_quadtree_chunks(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    camera_query: Query<&GlobalTransform, (With<Camera3d>, Without<QuadtreeChunk>)>,
    planet_query: Query<(Entity, &GlobalTransform), With<Planet>>,
    planet_settings: Res<PlanetSettings>,
    gpu_settings: Option<Res<GpuTerrainSettings>>,
    mut gpu_queue: Option<ResMut<GpuChunkQueue>>,
    mut debug_frame: Local<u32>,
    mut log_frame: Local<u32>,
    mut log_last: Local<u32>,
) {
    let Ok(camera_transform) = camera_query.single() else { return; };
    let Ok((planet_entity, planet_transform)) = planet_query.single() else { return; };
    
    // Transform camera position to planet's local space for quadtree update
    // This ensures correct LOD calculations regardless of planet rotation
    let planet_inverse = planet_transform.affine().inverse();
    let observer_pos_local = planet_inverse.transform_point3(camera_transform.translation());
    
    // Debug: Print camera altitude and chunk info periodically
    *debug_frame += 1;
    {
        if *debug_frame % 180 == 0 { // Every ~3 seconds at 60fps
            let altitude = observer_pos_local.length() - planet_settings.radius;
            let total_chunks = chunk_manager.quadtree.nodes.len();
            
            // Count chunks at each depth
            let mut depth_counts: [u32; 25] = [0; 25];
            let mut max_depth: u8 = 0;
            for node in chunk_manager.quadtree.nodes.values() {
                let d = node.address.depth as usize;
                if d < 25 {
                    depth_counts[d] += 1;
                }
                if node.address.depth > max_depth {
                    max_depth = node.address.depth;
                }
            }
            
            println!("[Quadtree DEBUG] Altitude: {:.1}m, Total nodes: {}, Max depth: {}", 
                altitude, total_chunks, max_depth);
            print!("  Depths: ");
            for d in 0..=max_depth.min(15) {
                if depth_counts[d as usize] > 0 {
                    print!("d{}:{} ", d, depth_counts[d as usize]);
                }
            }
            println!();
        }
    }
    
    // Update quadtree based on observer position in planet-local space
    let budget = chunk_manager.max_subdivisions_per_frame;
    let (to_spawn, to_despawn) = chunk_manager.quadtree.update(observer_pos_local, budget);
    
    // Despawn chunks that are no longer needed
    for address in to_despawn {
        if let Some(entity) = chunk_manager.quadtree_chunks.remove(&address) {
            commands.entity(entity).despawn();
        }
    }
    
    // Limit chunks spawned per frame
    let max_spawn_per_frame = chunk_manager.max_chunks_per_frame;
    
    // Spawn new chunks
    for (i, address) in to_spawn.iter().enumerate() {
        if i >= max_spawn_per_frame {
            break;
        }
        
        // Skip if already has entity
        if chunk_manager.quadtree_chunks.contains_key(address) {
            continue;
        }
        
        let face_dir = QuadtreeManager::face_direction(address.face);
        let world_center = address.world_center(face_dir, planet_settings.radius);
        
        // Calculate observer scale based on depth
        let observer_scale = chunk_manager.quadtree.observer_scale_for_depth(address.depth);
        
        // Calculate chunk parameters
        let ((min_x, min_y), (max_x, max_y)) = address.bounds_on_face();
        let chunk_size = max_x - min_x;
        
        // Resolution scales with depth for consistent detail
        let resolution = chunk_manager.base_chunk_resolution;
        
        // Spawn chunk entity as child of planet (so it rotates with the planet)
        let chunk_entity = commands.spawn((
            QuadtreeChunk {
                address: *address,
                depth: address.depth,
                observer_scale,
            },
            Transform::from_translation(world_center), // Local to planet
            Visibility::Hidden,
            ChildOf(planet_entity),
        )).id();
        
        chunk_manager.quadtree_chunks.insert(*address, chunk_entity);
        chunk_manager.quadtree.set_generating(*address);
        
        // Check if GPU generation is enabled
        let use_gpu = gpu_settings.as_ref()
            .map(|s| s.enabled)
            .unwrap_or(false);
        
        if use_gpu {
            // Queue for GPU generation
            if let Some(ref mut gpu_queue) = gpu_queue {
                // Convert quadtree address to chunk coordinates
                let (chunk_x, chunk_y) = address.to_xy();
                let chunks_per_face = 1u32 << address.depth;
                
                let gpu_request = GpuChunkRequest {
                    chunk_id: ChunkId {
                        face_index: address.face,
                        lod_level: address.depth as u32,
                        chunk_x: chunk_x as i32,
                        chunk_y: chunk_y as i32,
                    },
                    face_direction: face_dir,
                    chunk_x,
                    chunk_y,
                    chunks_per_face,
                    resolution,
                    radius: planet_settings.radius,
                    seed: chunk_manager.terrain_seed,
                    chunk_center: world_center,
                    observer_scale,
                    min_detail_scale: 0.001,
                };
                
                gpu_queue.pending.push(gpu_request.clone());
                gpu_queue.in_progress.push((gpu_request, chunk_entity));
                continue;
            }
        }
        
        // CPU generation — voxel or heightmap path
        let task = spawn_quadtree_chunk_task(
            *address,
            face_dir,
            planet_settings.radius,
            chunk_manager.terrain_seed,
            resolution,
            observer_scale,
        );

        commands.entity(chunk_entity).insert(QuadtreeChunkTask {
            address: *address,
            task,
        });
    }
    
    // Debug logging
    *log_frame += 1;
    {
        if *log_frame - *log_last >= 300 { // Log every ~5 seconds at 60fps
            *log_last = *log_frame;
            let leaf_count = chunk_manager.quadtree.get_visible_chunks().len();
            let total_nodes = chunk_manager.quadtree.nodes.len();
            info!("[Quadtree] {} leaf chunks, {} total nodes, {} entities", 
                  leaf_count, total_nodes, chunk_manager.quadtree_chunks.len());
        }
    }
}

/// System to manage parent chunk visibility
/// Parents stay visible as fallback until all their children have loaded meshes
/// This ensures terrain is always visible during LOD transitions
fn manage_parent_chunk_visibility(
    mut chunk_manager: ResMut<ChunkManager>,
    mut commands: Commands,
) {
    // Get all subdivided parents that still have entities
    let fallback_parents = chunk_manager.quadtree.get_fallback_parents();
    
    let mut parents_to_hide = Vec::new();
    
    for parent_addr in fallback_parents {
        // Check if all children have loaded entities
        if chunk_manager.quadtree.all_children_loaded(&parent_addr) {
            // All children ready - can hide/despawn parent
            parents_to_hide.push(parent_addr);
        }
        // Otherwise, parent stays visible as fallback
    }
    
    // Hide/despawn parents whose children are all loaded
    for parent_addr in parents_to_hide {
        if let Some(entity) = chunk_manager.quadtree_chunks.remove(&parent_addr) {
            commands.entity(entity).despawn();
        }
        // Clear the entity reference in the quadtree node
        if let Some(node) = chunk_manager.quadtree.nodes.get_mut(&parent_addr) {
            node.entity = None;
        }
    }
}

/// Spawn async task to generate a quadtree chunk
fn spawn_quadtree_chunk_task(
    address: QuadtreeAddress,
    face_direction: Vec3,
    radius: f32,
    seed: u32,
    resolution: u32,
    observer_scale: f32,
) -> Task<(Mesh, Option<Collider>)> {
    let thread_pool = AsyncComputeTaskPool::get();

    thread_pool.spawn(async move {
        let terrain_noise = TerrainNoise::new(seed);

        // Get chunk bounds
        let ((min_x, min_y), (max_x, max_y)) = address.bounds_on_face();
        let chunk_size = max_x - min_x;

        // Calculate axis vectors for this face
        let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let axis_b = face_direction.cross(axis_a);

        // Calculate chunk center for relative positioning
        let (cx, cy) = address.center_on_face();
        let point_on_cube = face_direction + cx * axis_a + cy * axis_b;
        let chunk_center = point_on_cube.normalize() * radius;

        // Generate mesh
        let mesh = generate_quadtree_chunk_mesh(
            face_direction,
            min_x, min_y,
            chunk_size,
            resolution,
            radius,
            chunk_center,
            &terrain_noise,
            observer_scale,
        );

        (mesh, None)
    })
}

/// Spawn a voxel-based chunk generation task (dual contouring path)
#[allow(dead_code)]
fn spawn_voxel_chunk_task(
    address: QuadtreeAddress,
    face_direction: Vec3,
    radius: f32,
    seed: u32,
    voxel_size: u32,
    observer_scale: f32,
) -> Task<(Mesh, Option<Collider>)> {
    let thread_pool = AsyncComputeTaskPool::get();

    thread_pool.spawn(async move {
        let terrain_noise = TerrainNoise::new(seed);

        // Calculate chunk center and world size
        let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let axis_b = face_direction.cross(axis_a);
        let (cx, cy) = address.center_on_face();
        let point_on_cube = face_direction + cx * axis_a + cy * axis_b;
        let chunk_center = point_on_cube.normalize() * radius;

        // Chunk world size from face bounds
        let ((min_x, _), (max_x, _)) = address.bounds_on_face();
        let face_size = max_x - min_x;
        let chunk_world_size = face_size * radius;
        let cell_size = chunk_world_size / voxel_size as f32;

        // Generate 3D density field
        let voxel_data = VoxelChunkData::generate(
            &terrain_noise,
            radius,
            chunk_center,
            chunk_world_size,
            voxel_size,
            observer_scale,
        );

        // Extract mesh via dual contouring
        if let Some(mut mesh) = dual_contouring::extract_mesh(&voxel_data) {
            // Offset mesh vertices so they're relative to chunk center (matches heightmap convention)
            // The DC mesh is already in world space relative to voxel_data.world_origin
            // We need positions relative to chunk_center for the Transform::from_translation to work
            let collider = Collider::from_bevy_mesh(&mesh, &ComputedColliderShape::TriMesh(TriMeshFlags::empty()));
            (mesh, collider)
        } else {
            // Empty chunk — return a degenerate mesh
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
            mesh.insert_indices(Indices::U32(Vec::new()));
            (mesh, None)
        }
    })
}

/// Generate mesh for a quadtree chunk with scale-dependent detail
fn generate_quadtree_chunk_mesh(
    face_direction: Vec3,
    start_x: f32,
    start_y: f32,
    size: f32,
    resolution: u32,
    radius: f32,
    chunk_center: Vec3,
    noise: &TerrainNoise,
    observer_scale: f32,
) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    
    let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = face_direction.cross(axis_a);
    
    let step = size / (resolution as f32 - 1.0);
    
    // Calculate actual observer scale based on chunk size and resolution
    // This determines how much detail to add in the terrain noise
    let chunk_world_size = size * radius; // Approximate chunk size in world units
    let effective_scale = (chunk_world_size / resolution as f32).max(observer_scale);
    
    for y in 0..resolution {
        for x in 0..resolution {
            let local_x = start_x + x as f32 * step;
            let local_y = start_y + y as f32 * step;
            
            let point_on_unit_cube = face_direction + local_x * axis_a + local_y * axis_b;
            let point_on_unit_sphere = point_on_unit_cube.normalize();
            
            // Get terrain data with SCALE-AWARE noise (key for infinite detail!)
            let (elevation_factor, color) = noise.get_data_at_scale(point_on_unit_sphere, effective_scale);
            
            // More dramatic height variation (0.15 instead of 0.1)
            let height_mult = 1.0 + elevation_factor * 0.15;
            let final_position_world = point_on_unit_sphere * radius * height_mult;
            let final_position = final_position_world - chunk_center;
            
            // Calculate normal using finite differences with scale-appropriate epsilon
            let epsilon = (step * 0.5).max(0.0001);
            let p_right = (point_on_unit_cube + axis_a * epsilon).normalize();
            let h_right = noise.get_elevation_at_scale(p_right, effective_scale);
            let pos_right_world = p_right * radius * (1.0 + h_right * 0.15);
            let pos_right = pos_right_world - chunk_center;
            
            let p_up = (point_on_unit_cube + axis_b * epsilon).normalize();
            let h_up = noise.get_elevation_at_scale(p_up, effective_scale);
            let pos_up_world = p_up * radius * (1.0 + h_up * 0.15);
            let pos_up = pos_up_world - chunk_center;
            
            let tangent_a = pos_right - final_position;
            let tangent_b = pos_up - final_position;
            let normal = tangent_a.cross(tangent_b).normalize();
            
            positions.push(final_position.to_array());
            normals.push(normal.to_array());
            uvs.push([x as f32 / (resolution as f32 - 1.0), y as f32 / (resolution as f32 - 1.0)]);
            colors.push(color.to_linear().to_f32_array());
            
            if x != resolution - 1 && y != resolution - 1 {
                let i = x + y * resolution;
                indices.push(i);
                indices.push(i + resolution + 1);
                indices.push(i + resolution);
                
                indices.push(i);
                indices.push(i + 1);
                indices.push(i + resolution + 1);
            }
        }
    }
    
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Process completed quadtree chunk generation tasks
fn process_quadtree_chunk_tasks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut chunk_query: Query<(Entity, &QuadtreeChunk, &mut QuadtreeChunkTask)>,
) {
    if !chunk_manager.use_quadtree {
        return;
    }
    
    let mut processed = 0;
    let max_per_frame = chunk_manager.max_chunks_per_frame;
    
    for (entity, chunk, mut task) in chunk_query.iter_mut() {
        if processed >= max_per_frame {
            break;
        }
        
        if task.task.is_finished() {
            if let Some((mesh, _collider)) = future::block_on(future::poll_once(&mut task.task)) {
                let mesh_handle = meshes.add(mesh);
                
                // Use shared material
                let material_handle = chunk_manager.shared_material_handle.clone();
                
                // Get world center from quadtree
                let face_dir = QuadtreeManager::face_direction(chunk.address.face);
                let world_center = chunk.address.world_center(face_dir, chunk_manager.planet_radius);
                
                if let Some(material) = material_handle {
                    commands.entity(entity).insert((
                        Mesh3d(mesh_handle),
                        MeshMaterial3d(material),
                        Transform::from_translation(world_center),
                        Visibility::Visible,
                    ));
                }
                
                // Remove task component
                commands.entity(entity).remove::<QuadtreeChunkTask>();
                
                // Mark as generated in quadtree
                chunk_manager.quadtree.set_generated(task.address);
                chunk_manager.quadtree.set_entity(task.address, entity);

                processed += 1;
            }
        }
    }
}

fn process_chunk_generation_tasks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_materials: ResMut<Assets<PlanetMaterial>>,
    mut chunk_query: Query<(Entity, &mut TerrainChunk, &mut ChunkGenerationTask)>,
    _planet_settings: Res<PlanetSettings>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut chunks_rendered: Local<usize>,
) {
    let mut processed = 0;
    let max_per_frame = chunk_manager.max_chunks_per_frame;
    let mut completed_tasks = Vec::new();

    for (entity, mut chunk, mut task) in chunk_query.iter_mut() {
        if processed >= max_per_frame { break; }
        if task.task.is_finished() {
            if let Some((mesh, collider)) = future::block_on(future::poll_once(&mut task.task)) {
                completed_tasks.push((entity, mesh, collider, chunk.id));
                chunk.is_loaded = true;
                chunk.is_generating = false;
                processed += 1;
            }
        }
    }

    let mut chunks_loaded_this_frame = 0;
    for (entity, mesh, collider, _chunk_id) in completed_tasks {
        if !chunk_query.contains(entity) { continue; }

        let mesh_handle = meshes.add(mesh);
        let material_handle = chunk_manager.shared_material_handle.clone()
            .unwrap_or_else(|| {
                let detail_texture = chunk_manager.detail_texture_handle.clone().unwrap_or_default();
                planet_materials.add(PlanetMaterial {
                    scaling: Vec4::new(0.02, 0.0, 0.95, 0.05),
                    detail_texture,
                })
            });

        if let Ok((_, mut chunk, _)) = chunk_query.get_mut(entity) {
            chunk.mesh_handle = Some(mesh_handle.clone());
            chunk.is_loaded = true;
            chunk.is_generating = false;
        }

        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            let chunk_pos = if let Ok((_, chunk, _)) = chunk_query.get(entity) {
                chunk.world_position
            } else { Vec3::ZERO };

            entity_commands.insert((
                Mesh3d(mesh_handle), MeshMaterial3d(material_handle),
                Transform::from_translation(chunk_pos), Visibility::Visible,
            ));
            entity_commands.insert(Visibility::Visible);

            if let Some(collider) = collider {
                entity_commands.insert((collider, RigidBody::Fixed));
            }

            entity_commands.remove::<ChunkGenerationTask>();
            chunks_loaded_this_frame += 1;
            *chunks_rendered += 1;

            if *chunks_rendered <= 50 || chunks_loaded_this_frame <= 10 {
                println!("[Chunk Render] Chunk {} rendered at position: {:?} (total rendered: {}, face: {}, lod: {})",
                    chunks_loaded_this_frame, chunk_pos, *chunks_rendered, _chunk_id.face_index, _chunk_id.lod_level);
            }
        }
    }

    if chunks_loaded_this_frame > 0 {
        println!("Loaded {} chunks this frame. Total active chunks: {}", chunks_loaded_this_frame, chunk_manager.chunks.len());
    }
}

