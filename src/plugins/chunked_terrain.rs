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
               update_chunk_lods.run_if(quadtree_disabled),
               unload_distant_chunks.run_if(quadtree_disabled),
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
fn quadtree_disabled(chunk_manager: Res<ChunkManager>) -> bool {
    !chunk_manager.use_quadtree
}

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

// Find which chunk the viewport center is looking at
fn find_viewport_center_chunk(
    camera_query: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<TerrainChunk>)>,
    window_query: Query<&Window, With<bevy::window::PrimaryWindow>>,
    planet_query: Query<(Entity, &GlobalTransform), With<Planet>>,
    rapier_context: ReadRapierContext,
    planet_settings: &PlanetSettings,
    chunk_manager: &ChunkManager,
) -> Option<ChunkId> {
    let Ok((camera_transform, camera)) = camera_query.single() else { return None; };
    let Ok(window) = window_query.single() else { return None; };
    let Ok((planet_entity, _)) = planet_query.single() else { return None; };

    // Get viewport center (screen center)
    let screen_center = Vec2::new(window.width() / 2.0, window.height() / 2.0);

    // Convert screen coordinates to world ray
    let Ok(ray) = camera.viewport_to_world(camera_transform, screen_center) else { return None; };

    // Raycast to planet
    let ray_origin = ray.origin;
    let ray_dir = *ray.direction;
    let max_toi = 200000.0;

    let filter = QueryFilter::default();

    let Ok(rapier_ctx) = rapier_context.single() else { return None; };
    if let Some((entity, toi)) = rapier_ctx.cast_ray(
        ray_origin,
        ray_dir,
        max_toi,
        true,
        filter,
    ) {
        if entity == planet_entity {
            // Calculate hit point on planet surface
            let hit_point = ray_origin + ray_dir * toi;
            let direction_from_center = hit_point.normalize();
            
            // Calculate LOD based on camera altitude (same as update_chunk_lods)
            let camera_pos = camera_transform.translation();
            let camera_altitude = camera_pos.length() - planet_settings.radius;
            let min_lod_distance = 0.0;
            let max_lod_distance = planet_settings.radius * 2.0;
            let lod_level = calculate_lod_level(camera_altitude, min_lod_distance, max_lod_distance, chunk_manager.max_lod_levels);
            
            // Find which face this point belongs to
            let directions = [
                Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
            ];
            
            // Find the face with the highest dot product (closest to this direction)
            let mut best_face = 0;
            let mut best_dot = direction_from_center.dot(directions[0]);
            for (i, &dir) in directions.iter().enumerate().skip(1) {
                let dot = direction_from_center.dot(dir);
                if dot > best_dot {
                    best_dot = dot;
                    best_face = i;
                }
            }
            
            let face_direction = directions[best_face];
            
            // Convert direction to cube coordinates
            let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
            let axis_b = face_direction.cross(axis_a);
            
            // Project direction onto face plane
            let point_on_cube = direction_from_center / direction_from_center.dot(face_direction).abs();
            let local_x = point_on_cube.dot(axis_a);
            let local_y = point_on_cube.dot(axis_b);
            
            // Convert to chunk coordinates (assuming fixed chunks_per_face for now)
            let chunks_per_face = 8u32; // Use a reasonable default
            let chunk_size = 2.0 / chunks_per_face as f32;
            let chunk_x = ((local_x + 1.0) / chunk_size).floor() as i32;
            let chunk_y = ((local_y + 1.0) / chunk_size).floor() as i32;
            
            // Clamp to valid range
            let chunk_x = chunk_x.clamp(0, chunks_per_face as i32 - 1);
            let chunk_y = chunk_y.clamp(0, chunks_per_face as i32 - 1);
            
            return Some(ChunkId {
                face_index: best_face as u8,
                lod_level,
                chunk_x,
                chunk_y,
            });
        }
    }
    
    None
}

fn update_chunk_lods(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    camera_query: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<TerrainChunk>)>,
    gpu_settings: Option<Res<GpuTerrainSettings>>,
    mut gpu_queue: Option<ResMut<GpuChunkQueue>>,
    chunk_query: Query<(Entity, &TerrainChunk)>,
    planet_query: Query<(Entity, &GlobalTransform), With<Planet>>,
    planet_settings: Res<PlanetSettings>,
    window_query: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rapier_context: ReadRapierContext,
) {
    let Ok((camera_transform, camera)) = camera_query.single() else { return; };
    let Ok((planet_entity, _planet_transform)) = planet_query.single() else { return; };

    let camera_pos = camera_transform.translation();

    // Find the chunk at viewport center
    let Some(center_chunk_id) = find_viewport_center_chunk(
        camera_query,
        window_query,
        planet_query,
        rapier_context,
        &planet_settings,
        &chunk_manager,
    ) else {
        // Can't find viewport center chunk, skip this frame
        return;
    };
    
    // Calculate LOD based on camera altitude
    let camera_altitude = camera_pos.length() - planet_settings.radius;
    let min_lod_distance = 0.0;
    let max_lod_distance = planet_settings.radius * 2.0;
    let lod_level = calculate_lod_level(camera_altitude, min_lod_distance, max_lod_distance, chunk_manager.max_lod_levels);
    
    // Use fixed chunks_per_face for 3x3 grid (we'll always load 3x3 = 9 chunks)
    let chunks_per_face = 8u32; // Fixed grid size
    
    // Get face direction for the center chunk
    let directions = [
        Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
    ];
    let face_direction = directions[center_chunk_id.face_index as usize];
    
    // Track which chunks should be loaded (circular area centered on viewport center chunk)
    // Store both chunk_id and original world position to avoid recalculation errors
    let mut chunks_to_load: HashMap<ChunkId, Vec3> = HashMap::new();
    
    // Generate circular area around center chunk based on render radius
    // Adapt radius based on LOD level: higher LOD (farther) = smaller radius
    let base_radius = chunk_manager.chunk_render_radius;
    let adaptive_radius = if lod_level > 2 {
        base_radius * 0.7 // Reduce radius for distant chunks
    } else if lod_level > 0 {
        base_radius * 0.85 // Slightly reduce for mid-distance
    } else {
        base_radius // Full radius for close chunks
    };
    let radius = adaptive_radius;
    let radius_int = radius.ceil() as i32;
    
    // Limit max chunks per face to prevent excessive loading
    let max_chunks_per_face = 80;
    
    // Track active faces (will include adjacent faces when circle extends beyond boundaries)
    let mut active_faces_set = HashSet::from([center_chunk_id.face_index as usize]);
    
    // Calculate axis vectors for the center face
    let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = face_direction.cross(axis_a);
    let chunk_size = 2.0 / chunks_per_face as f32;
    
    // Iterate through a square area and check if chunks are within the circle
    for dy in -radius_int..=radius_int {
        for dx in -radius_int..=radius_int {
            // Calculate distance from center chunk (in chunk coordinate space)
            let distance = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt();
            
            // Only include chunks within the radius
            if distance <= radius {
                // Calculate offset chunk coordinates (may go out of bounds)
                // IMPORTANT: dx is horizontal (maps to chunk_y), dy is vertical (maps to chunk_x)
                // For front face (+Z): chunk_x maps to Y (vertical), chunk_y maps to X (horizontal)
                let offset_chunk_x = center_chunk_id.chunk_x as i32 + dy; // dy is vertical -> chunk_x
                let offset_chunk_y = center_chunk_id.chunk_y as i32 + dx; // dx is horizontal -> chunk_y
                
                // Calculate cube coordinates for this offset chunk
                // Use the same logic as calculate_chunk_position, but allow out-of-bounds coordinates
                let offset_chunk_center_x = (offset_chunk_x as f32 + 0.5) * chunk_size - 1.0;
                let offset_chunk_center_y = (offset_chunk_y as f32 + 0.5) * chunk_size - 1.0;
                
                // Calculate cube position using the center face's coordinate system
                // This can go outside [-1, 1] range when crossing face boundaries
                let point_on_cube = face_direction + offset_chunk_center_x * axis_a + offset_chunk_center_y * axis_b;
                
                // Normalize to get sphere position (this correctly maps to adjacent faces)
                let point_on_sphere = point_on_cube.normalize();
                let chunk_world_pos = point_on_sphere * planet_settings.radius;
                
                // Convert world position back to chunk ID (handles cross-face boundaries automatically)
                if let Some(chunk_id) = world_position_to_chunk_id(chunk_world_pos, chunks_per_face, lod_level) {
                    // Check max chunks per face limit
                    let chunks_in_face = chunks_to_load.keys().filter(|id| id.face_index == chunk_id.face_index).count();
                    if chunks_in_face < max_chunks_per_face {
                        // Store the original world position to avoid recalculation errors
                        chunks_to_load.insert(chunk_id, chunk_world_pos);
                        active_faces_set.insert(chunk_id.face_index as usize);
                    }
                }
            }
        }
    }
    
    // Update active faces set (includes all faces that have chunks in the render radius)
    chunk_manager.active_faces = active_faces_set;
    
    // Generate chunks for the circular area
    let mut chunks_generated = 0;
    let mut chunks_skipped = 0;
    
    for (chunk_id, chunk_world_pos) in chunks_to_load.iter() {
        // Check if chunk already exists
        if chunk_manager.chunks.contains_key(chunk_id) {
            chunks_skipped += 1;
            continue;
        }
        
        // Check chunk limit
        if chunk_manager.chunks.len() >= chunk_manager.max_active_chunks {
            continue;
        }
        
        // Use the original world position we calculated, not a recalculated one
        let chunk_id = *chunk_id; // Clone for use below
        let chunk_world_pos = *chunk_world_pos; // Clone for use below
        
        // Get the chunk's actual face direction
        let directions = [
            Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
        ];
        let chunk_face_direction = directions[chunk_id.face_index as usize];
        
        chunks_generated += 1;
        
        // Spawn chunk entity as child of planet (will be populated when generation completes)
        let chunk_entity = commands.spawn((
            TerrainChunk {
                id: chunk_id,
                world_position: chunk_world_pos,
                mesh_handle: None,
                collider_handle: None,
                is_loaded: false,
                is_generating: true,
            },
            Transform::from_translation(chunk_world_pos), // Local to planet
            Visibility::Hidden,
            ChildOf(planet_entity),
        )).id();
        
        chunk_manager.chunks.insert(chunk_id, chunk_entity);
        
        // Check neighbor LODs for geomorphing
        let neighbor_lods = get_neighbor_lods(
            &chunk_id,
            lod_level,
            chunk_id.chunk_x as u32,
            chunk_id.chunk_y as u32,
            chunks_per_face,
            &chunk_manager.chunks,
        );
        
        // Check if GPU generation is enabled
        let use_gpu = gpu_settings.as_ref()
            .map(|s| s.enabled)
            .unwrap_or(false);
        
        if use_gpu {
            // Queue for GPU generation
            if let Some(ref mut gpu_queue) = gpu_queue {
                let resolution = chunk_manager.base_chunk_resolution / (2_u32.pow(lod_level.min(3)));
                let resolution = resolution.max(16);
                
                // Use the original world position as chunk center (more accurate than recalculating)
                let chunk_center_world = chunk_world_pos;
                
                // Calculate observer_scale for infinite scale rendering
                // This determines how much detail to generate at this LOD level
                // observer_scale = approximate meters per pixel at this chunk's LOD
                let chunk_size_meters = (2.0 * planet_settings.radius) / chunks_per_face as f32;
                let observer_scale = chunk_size_meters / resolution as f32;
                let min_detail_scale = 0.001; // 1mm minimum detail
                
                let gpu_request = GpuChunkRequest {
                    chunk_id,
                    face_direction: chunk_face_direction,
                    chunk_x: chunk_id.chunk_x as u32,
                    chunk_y: chunk_id.chunk_y as u32,
                    chunks_per_face,
                    resolution,
                    radius: planet_settings.radius,
                    seed: chunk_manager.terrain_seed,
                    chunk_center: chunk_center_world,
                    observer_scale,
                    min_detail_scale,
                };
                
                let gpu_request_clone = gpu_request.clone();
                gpu_queue.pending.push(gpu_request);
                gpu_queue.in_progress.push((gpu_request_clone, chunk_entity));
                continue; // Skip CPU generation, GPU will handle it
            }
        }
        
        // CPU generation (fallback or when GPU disabled)
        let task = spawn_chunk_generation_task(
            chunk_id,
            lod_level,
            chunk_face_direction,
            chunk_id.chunk_x as u32,
            chunk_id.chunk_y as u32,
            chunks_per_face,
            planet_settings.radius,
            chunk_manager.terrain_seed,
            chunk_manager.base_chunk_resolution,
            neighbor_lods,
        );
        
        commands.entity(chunk_entity).insert(ChunkGenerationTask {
            id: chunk_id,
            task,
        });
    }
    
    // Debug logging
    if chunks_generated > 0 {
        println!("[Circular Grid] Generated {} chunks, skipped {} existing. Center chunk: face={}, x={}, y={}, radius={:.1}", 
            chunks_generated, chunks_skipped, 
            center_chunk_id.face_index, center_chunk_id.chunk_x, center_chunk_id.chunk_y, chunk_manager.chunk_render_radius);
    }
}

fn calculate_lod_level(distance: f32, min_dist: f32, max_dist: f32, max_lods: u32) -> u32 {
    if distance < min_dist {
        return 0; // Highest detail
    }
    if distance > max_dist {
        return max_lods - 1; // Lowest detail
    }
    
    // Logarithmic LOD calculation
    let t = (distance - min_dist) / (max_dist - min_dist);
    let lod = (t * max_lods as f32).floor() as u32;
    lod.min(max_lods - 1)
}

// Convert world position to chunk ID (reuses logic from find_viewport_center_chunk)
fn world_position_to_chunk_id(
    world_pos: Vec3,
    chunks_per_face: u32,
    lod_level: u32,
) -> Option<ChunkId> {
    let direction_from_center = world_pos.normalize();
    
    // Find which face this point belongs to
    let directions = [
        Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
    ];
    
    // Find the face with the highest dot product (closest to this direction)
    let mut best_face = 0;
    let mut best_dot = direction_from_center.dot(directions[0]);
    for (i, &dir) in directions.iter().enumerate().skip(1) {
        let dot = direction_from_center.dot(dir);
        if dot > best_dot {
            best_dot = dot;
            best_face = i;
        }
    }
    
    let face_direction = directions[best_face];
    
    // Convert direction to cube coordinates
    let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = face_direction.cross(axis_a);
    
    // Project direction onto face plane
    let point_on_cube = direction_from_center / direction_from_center.dot(face_direction).abs();
    let local_x = point_on_cube.dot(axis_a);
    let local_y = point_on_cube.dot(axis_b);
    
    // Convert to chunk coordinates
    let chunk_size = 2.0 / chunks_per_face as f32;
    let chunk_x_unclamped = ((local_x + 1.0) / chunk_size).floor() as i32;
    let chunk_y_unclamped = ((local_y + 1.0) / chunk_size).floor() as i32;
    
    // Clamp to valid range - but this loses information for cross-face transitions
    // The real issue is that when coordinates are out of bounds, we've crossed to an adjacent face,
    // but the coordinate system on that face is rotated, so the mapping is wrong.
    // For now, let's clamp normally and see if we can fix the mapping at the call site.
    let chunk_x = chunk_x_unclamped.clamp(0, chunks_per_face as i32 - 1);
    let chunk_y = chunk_y_unclamped.clamp(0, chunks_per_face as i32 - 1);
    
    // Debug: log when clamping occurs (indicates potential coordinate mapping issue)
    static DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);
    if chunk_x_unclamped != chunk_x || chunk_y_unclamped != chunk_y {
        let count = DEBUG_COUNT.fetch_add(1, Ordering::Relaxed);
        if count < 20 {
            println!("[DEBUG WORLD_TO_CHUNK] world_pos=({:.2}, {:.2}, {:.2}) | face={} | local=({:.3}, {:.3}) | unclamped=({}, {}) | clamped=({}, {})",
                world_pos.x, world_pos.y, world_pos.z,
                best_face,
                local_x, local_y,
                chunk_x_unclamped, chunk_y_unclamped,
                chunk_x, chunk_y);
        }
    }
    
    Some(ChunkId {
        face_index: best_face as u8,
        lod_level,
        chunk_x,
        chunk_y,
    })
}

fn calculate_chunk_position(
    face_direction: Vec3,
    chunk_x: u32,
    chunk_y: u32,
    chunks_per_face: u32,
    radius: f32,
) -> Vec3 {
    // Calculate the center of this chunk on the cube sphere face
    let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = face_direction.cross(axis_a);
    
    let chunk_size = 2.0 / chunks_per_face as f32;
    let chunk_center_x = (chunk_x as f32 + 0.5) * chunk_size - 1.0;
    let chunk_center_y = (chunk_y as f32 + 0.5) * chunk_size - 1.0;
    
    let point_on_cube = face_direction + chunk_center_x * axis_a + chunk_center_y * axis_b;
    let point_on_sphere = point_on_cube.normalize();
    
    point_on_sphere * radius
}

fn spawn_chunk_generation_task(
    chunk_id: ChunkId,
    lod_level: u32,
    face_direction: Vec3,
    chunk_x: u32,
    chunk_y: u32,
    chunks_per_face: u32,
    radius: f32,
    seed: u32,
    base_resolution: u32,
    neighbor_lods: (Option<u32>, Option<u32>, Option<u32>, Option<u32>), // (left, right, bottom, top) neighbor LODs
) -> Task<(Mesh, Option<Collider>)> {
    let thread_pool = AsyncComputeTaskPool::get();
    
    thread_pool.spawn(async move {
        // Calculate resolution for this LOD level
        // Higher LOD = lower resolution
        // Optimized resolution curve: 64→32→16→8
        let resolution = base_resolution / (2_u32.pow(lod_level.min(3))); // Cap at 3x reduction
        let resolution = resolution.max(8); // Minimum 8x8 vertices for performance
        
        // Calculate chunk center position (needed to make vertices relative)
        let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let axis_b = face_direction.cross(axis_a);
        let chunk_size = 2.0 / chunks_per_face as f32;
        let chunk_center_x = (chunk_x as f32 + 0.5) * chunk_size - 1.0;
        let chunk_center_y = (chunk_y as f32 + 0.5) * chunk_size - 1.0;
        let point_on_cube = face_direction + chunk_center_x * axis_a + chunk_center_y * axis_b;
        let point_on_sphere = point_on_cube.normalize();
        let chunk_center_world = point_on_sphere * radius;
        
        // Generate terrain noise
        let terrain_noise = TerrainNoise::new(seed);
        
        // Generate chunk mesh with geomorphing (vertices will be relative to chunk center)
        let mesh = generate_chunk_mesh(
            face_direction,
            chunk_x,
            chunk_y,
            chunks_per_face,
            resolution,
            radius,
            chunk_center_world, // Pass chunk center so vertices can be relative
            lod_level,
            neighbor_lods,
            &terrain_noise,
        );
        
        // Temporarily disable chunk colliders - they cause Rapier AABB issues with large world coordinates
        // The main planet already has a collider, so chunk colliders are not strictly necessary
        // TODO: Re-enable with proper coordinate handling or use convex hull instead
        let collider = None;
        
        (mesh, collider)
    })
}

// Helper function to get neighbor LOD levels for geomorphing
fn get_neighbor_lods(
    chunk_id: &ChunkId,
    current_lod: u32,
    chunk_x: u32,
    chunk_y: u32,
    chunks_per_face: u32,
    chunks: &HashMap<ChunkId, Entity>,
) -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
    // Check neighbors: (left, right, bottom, top)
    let mut left_lod = None;
    let mut right_lod = None;
    let mut bottom_lod = None;
    let mut top_lod = None;
    
    // Left neighbor
    if chunk_x > 0 {
        for lod in 0..=6 {
            let check_id = ChunkId {
                face_index: chunk_id.face_index,
                lod_level: lod,
                chunk_x: chunk_id.chunk_x - 1,
                chunk_y: chunk_id.chunk_y,
            };
            if chunks.contains_key(&check_id) {
                left_lod = Some(lod);
                break;
            }
        }
    }
    
    // Right neighbor
    if chunk_x < chunks_per_face - 1 {
        for lod in 0..=6 {
            let check_id = ChunkId {
                face_index: chunk_id.face_index,
                lod_level: lod,
                chunk_x: chunk_id.chunk_x + 1,
                chunk_y: chunk_id.chunk_y,
            };
            if chunks.contains_key(&check_id) {
                right_lod = Some(lod);
                break;
            }
        }
    }
    
    // Bottom neighbor
    if chunk_y > 0 {
        for lod in 0..=6 {
            let check_id = ChunkId {
                face_index: chunk_id.face_index,
                lod_level: lod,
                chunk_x: chunk_id.chunk_x,
                chunk_y: chunk_id.chunk_y - 1,
            };
            if chunks.contains_key(&check_id) {
                bottom_lod = Some(lod);
                break;
            }
        }
    }
    
    // Top neighbor
    if chunk_y < chunks_per_face - 1 {
        for lod in 0..=6 {
            let check_id = ChunkId {
                face_index: chunk_id.face_index,
                lod_level: lod,
                chunk_x: chunk_id.chunk_x,
                chunk_y: chunk_id.chunk_y + 1,
            };
            if chunks.contains_key(&check_id) {
                top_lod = Some(lod);
                break;
            }
        }
    }
    
    (left_lod, right_lod, bottom_lod, top_lod)
}

fn generate_chunk_mesh(
    face_direction: Vec3,
    chunk_x: u32,
    chunk_y: u32,
    chunks_per_face: u32,
    resolution: u32,
    radius: f32,
    chunk_center_world: Vec3,
    lod_level: u32,
    neighbor_lods: (Option<u32>, Option<u32>, Option<u32>, Option<u32>),
    noise: &TerrainNoise,
) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    
    let axis_a = if face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = face_direction.cross(axis_a);
    
    let chunk_size = 2.0 / chunks_per_face as f32;
    let chunk_start_x = chunk_x as f32 * chunk_size - 1.0;
    let chunk_start_y = chunk_y as f32 * chunk_size - 1.0;
    let _chunk_end_x = chunk_start_x + chunk_size;
    let _chunk_end_y = chunk_start_y + chunk_size;
    
    let chunk_step_x = chunk_size / (resolution as f32 - 1.0);
    let chunk_step_y = chunk_size / (resolution as f32 - 1.0);
    
    // Calculate observer scale based on LOD level and chunk size
    // This enables scale-dependent detail in terrain generation
    let chunk_world_size = chunk_size * radius;
    let base_scale = chunk_world_size / resolution as f32;
    let observer_scale = base_scale * (1.0 + lod_level as f32 * 0.5);
    
    // Height multiplier for more dramatic terrain
    let height_mult_factor = 0.15;
    
    let start_index = 0u32;
    
    for y in 0..resolution {
        for x in 0..resolution {
            let local_x = if resolution > 1 {
                chunk_start_x + x as f32 * chunk_step_x
            } else {
                chunk_start_x + chunk_size * 0.5
            };
            let local_y = if resolution > 1 {
                chunk_start_y + y as f32 * chunk_step_y
            } else {
                chunk_start_y + chunk_size * 0.5
            };
            
            let point_on_unit_cube = face_direction + local_x * axis_a + local_y * axis_b;
            let point_on_unit_sphere = point_on_unit_cube.normalize();
            
            // Get terrain data with SCALE-AWARE detail
            let (elevation_factor, color) = noise.get_data_at_scale(point_on_unit_sphere, observer_scale);
            
            // Geomorphing: Adjust boundary vertices to match lower LOD neighbors
            let mut morph_factor = 1.0;
            let is_boundary = x == 0 || x == resolution - 1 || y == 0 || y == resolution - 1;
            
            if is_boundary {
                // Check if we need to morph based on neighbor LODs
                let needs_morph_left = x == 0 && neighbor_lods.0.map_or(false, |lod| lod > lod_level);
                let needs_morph_right = x == resolution - 1 && neighbor_lods.1.map_or(false, |lod| lod > lod_level);
                let needs_morph_bottom = y == 0 && neighbor_lods.2.map_or(false, |lod| lod > lod_level);
                let needs_morph_top = y == resolution - 1 && neighbor_lods.3.map_or(false, |lod| lod > lod_level);
                
                if needs_morph_left || needs_morph_right || needs_morph_bottom || needs_morph_top {
                    // Calculate morph factor based on distance from boundary
                    // Smooth transition over a few vertices
                    let morph_distance = 3.0; // Number of vertices to morph
                    let dist_from_boundary = if x == 0 || x == resolution - 1 {
                        (x.min(resolution - 1 - x) as f32).min(morph_distance) / morph_distance
                    } else if y == 0 || y == resolution - 1 {
                        (y.min(resolution - 1 - y) as f32).min(morph_distance) / morph_distance
                    } else {
                        1.0
                    };
                    
                    // Morph factor: 1.0 at boundary, 0.0 away from boundary
                    morph_factor = 1.0 - dist_from_boundary;
                    
                    // For boundary vertices, sample terrain at the exact position lower LOD would use
                    // This ensures perfect alignment
                    if morph_factor > 0.01 {
                        // Re-sample at lower LOD resolution to get matching position
                        let lower_lod = neighbor_lods.0
                            .or(neighbor_lods.1)
                            .or(neighbor_lods.2)
                            .or(neighbor_lods.3)
                            .unwrap_or(lod_level);
                        
                        if lower_lod > lod_level {
                            // Calculate what the lower LOD neighbor's vertex position would be
                            // This ensures seamless connection
                            let lower_res = resolution / (2_u32.pow((lower_lod - lod_level).min(3)));
                            let lower_step_x = chunk_size / (lower_res as f32 - 1.0);
                            let lower_step_y = chunk_size / (lower_res as f32 - 1.0);
                            
                            // Snap to lower LOD grid
                            let snapped_x = (local_x / lower_step_x).round() * lower_step_x;
                            let snapped_y = (local_y / lower_step_y).round() * lower_step_y;
                            
                            let snapped_cube = face_direction + snapped_x * axis_a + snapped_y * axis_b;
                            let snapped_sphere = snapped_cube.normalize();
                            let snapped_elevation = noise.get_elevation_at_scale(snapped_sphere, observer_scale);
                            let snapped_height = radius * (1.0 + snapped_elevation * height_mult_factor);
                            let snapped_pos_world = snapped_sphere * snapped_height;
                            
                            // Interpolate between high-detail and low-detail positions
                            let high_detail_pos = point_on_unit_sphere * radius * (1.0 + elevation_factor * height_mult_factor);
                            let morphed_pos = high_detail_pos.lerp(snapped_pos_world, morph_factor);
                            let final_position_world = morphed_pos;
                            
                            // Make vertex position relative to chunk center
                            let final_position = final_position_world - chunk_center_world;
                            
                            // Continue with normal calculation using morphed position
                            let epsilon = 0.001;
                            let p_right = (point_on_unit_cube + axis_a * epsilon).normalize();
                            let h_right = noise.get_elevation(p_right);
                            let pos_right_world = p_right * radius * (1.0 + h_right * height_mult_factor);
                            let pos_right = pos_right_world - chunk_center_world;
                            
                            let p_up = (point_on_unit_cube + axis_b * epsilon).normalize();
                            let h_up = noise.get_elevation_at_scale(p_up, observer_scale);
                            let pos_up_world = p_up * radius * (1.0 + h_up * height_mult_factor);
                            let pos_up = pos_up_world - chunk_center_world;
                            
                            let tangent_a = pos_right - final_position;
                            let tangent_b = pos_up - final_position;
                            let normal = tangent_a.cross(tangent_b).normalize();
                            
                            let i = x + y * resolution;
                            positions.push(final_position.to_array());
                            normals.push(normal.to_array());
                            uvs.push([x as f32 / (resolution as f32 - 1.0), y as f32 / (resolution as f32 - 1.0)]);
                            colors.push(color.to_linear().to_f32_array());
                            
                            if x != resolution - 1 && y != resolution - 1 {
                                indices.push(start_index + i);
                                indices.push(start_index + i + resolution + 1);
                                indices.push(start_index + i + resolution);
                                
                                indices.push(start_index + i);
                                indices.push(start_index + i + 1);
                                indices.push(start_index + i + resolution + 1);
                            }
                            continue; // Skip normal path for morphed vertices
                        }
                    }
                }
            }
            
            let height_mult = 1.0 + elevation_factor * height_mult_factor;
            let final_position_world = point_on_unit_sphere * radius * height_mult;
            
            // Make vertex position relative to chunk center (so transform works correctly)
            let final_position = final_position_world - chunk_center_world;
            
            // Calculate normal using finite differences with scale-appropriate sampling
            let epsilon = (chunk_step_x * 0.5).max(0.001);
            let p_right = (point_on_unit_cube + axis_a * epsilon).normalize();
            let h_right = noise.get_elevation_at_scale(p_right, observer_scale);
            let pos_right_world = p_right * radius * (1.0 + h_right * height_mult_factor);
            let pos_right = pos_right_world - chunk_center_world;
            
            let p_up = (point_on_unit_cube + axis_b * epsilon).normalize();
            let h_up = noise.get_elevation_at_scale(p_up, observer_scale);
            let pos_up_world = p_up * radius * (1.0 + h_up * height_mult_factor);
            let pos_up = pos_up_world - chunk_center_world;
            
            let tangent_a = pos_right - final_position;
            let tangent_b = pos_up - final_position;
            let normal = tangent_a.cross(tangent_b).normalize();
            
            let i = x + y * resolution;
            positions.push(final_position.to_array());
            normals.push(normal.to_array());
            uvs.push([x as f32 / (resolution as f32 - 1.0), y as f32 / (resolution as f32 - 1.0)]);
            colors.push(color.to_linear().to_f32_array());
            
            if x != resolution - 1 && y != resolution - 1 {
                indices.push(start_index + i);
                indices.push(start_index + i + resolution + 1);
                indices.push(start_index + i + resolution);
                
                indices.push(start_index + i);
                indices.push(start_index + i + 1);
                indices.push(start_index + i + resolution + 1);
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

fn process_chunk_generation_tasks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_materials: ResMut<Assets<PlanetMaterial>>,
    mut chunk_query: Query<(Entity, &mut TerrainChunk, &mut ChunkGenerationTask)>,
    _planet_settings: Res<PlanetSettings>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut chunks_rendered: Local<usize>,
) {
    // Process a limited number of chunks per frame
    let mut processed = 0;
    let max_per_frame = chunk_manager.max_chunks_per_frame;
    
    // Collect completed tasks first to avoid borrowing issues
    let mut completed_tasks = Vec::new();
    
    for (entity, mut chunk, mut task) in chunk_query.iter_mut() {
        if processed >= max_per_frame {
            break;
        }
        
        // Non-blocking check: use is_finished() instead of block_on
        if task.task.is_finished() {
            // Task is complete, poll it once to get the result (non-blocking)
            if let Some((mesh, collider)) = future::block_on(future::poll_once(&mut task.task)) {
                // Generation complete! Store for later insertion
                completed_tasks.push((entity, mesh, collider, chunk.id));
                chunk.is_loaded = true;
                chunk.is_generating = false;
                processed += 1;
            }
        }
    }
    
    // Now insert bundles for completed tasks
    // Check entity existence before inserting to avoid panics
    let mut chunks_loaded_this_frame = 0;
    for (entity, mesh, collider, _chunk_id) in completed_tasks {
        // Verify entity still exists (might have been despawned by unload_distant_chunks)
        if !chunk_query.contains(entity) {
            continue; // Entity was despawned, skip
        }
        
        let mesh_handle = meshes.add(mesh);
        
        // Use shared material instead of creating per-chunk material
        let material_handle = chunk_manager.shared_material_handle.clone()
            .unwrap_or_else(|| {
                // Fallback: create material if shared one doesn't exist yet
                let detail_texture = chunk_manager.detail_texture_handle.clone().unwrap_or_default();
                planet_materials.add(PlanetMaterial {
                    scaling: Vec4::new(0.02, 0.0, 0.95, 0.05),
                    detail_texture,
                })
            });
        
        // Update chunk mesh handle and mark as loaded
        if let Ok((_, mut chunk, _)) = chunk_query.get_mut(entity) {
            chunk.mesh_handle = Some(mesh_handle.clone());
            chunk.is_loaded = true;
            chunk.is_generating = false;
        }
        
        // Insert bundle - use try_insert to avoid panic if entity was despawned
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            // Get chunk position for debug
            let chunk_pos = if let Ok((_, chunk, _)) = chunk_query.get(entity) {
                chunk.world_position
            } else {
                Vec3::ZERO
            };
            
            // Insert mesh components - mesh, material, transform, and visibility
            entity_commands.insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
                Transform::from_translation(chunk_pos), // Chunk center position
                Visibility::Visible, // Explicitly make visible
            ));
            
            // Explicitly set visibility again to ensure it's applied (sometimes needed after bundle insertion)
            entity_commands.insert(Visibility::Visible);
            
            // Add collider if present
            if let Some(collider) = collider {
                entity_commands.insert((
                    collider,
                    RigidBody::Fixed,
                ));
            }
            
            // Remove task component
            entity_commands.remove::<ChunkGenerationTask>();
            chunks_loaded_this_frame += 1;
            
            // Debug: Print chunk positions (limit to avoid spam)
            *chunks_rendered += 1;
            if *chunks_rendered <= 50 || chunks_loaded_this_frame <= 10 {
                println!("[Chunk Render] Chunk {} rendered at position: {:?} (total rendered: {}, face: {}, lod: {})",
                    chunks_loaded_this_frame, chunk_pos, *chunks_rendered, _chunk_id.face_index, _chunk_id.lod_level);
            }
        } else {
            // Entity was despawned before we could insert the mesh
            println!("[Chunk Render] WARNING: Entity {:?} was despawned before mesh could be inserted", entity);
        }
    }
    
    // Debug output
    if chunks_loaded_this_frame > 0 {
        println!("Loaded {} chunks this frame. Total active chunks: {}", chunks_loaded_this_frame, chunk_manager.chunks.len());
    }
}

fn unload_distant_chunks(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    camera_query: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<TerrainChunk>)>,
    chunk_query: Query<(Entity, &TerrainChunk)>,
    planet_query: Query<(Entity, &GlobalTransform), With<Planet>>,
    planet_settings: Res<PlanetSettings>,
    window_query: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rapier_context: ReadRapierContext,
) {
    // Find the viewport center chunk (same as in update_chunk_lods)
    let Some(center_chunk_id) = find_viewport_center_chunk(
        camera_query,
        window_query,
        planet_query,
        rapier_context,
        &planet_settings,
        &chunk_manager,
    ) else {
        // Can't find viewport center chunk, skip unloading this frame
        return;
    };
    
    // Calculate which chunks should be kept (circular area based on render radius)
    let chunks_per_face = 8u32;
    let mut chunks_to_keep: HashSet<ChunkId> = HashSet::new();
    
    // Generate circular area around center chunk based on render radius
    let radius = chunk_manager.chunk_render_radius;
    let radius_int = radius.ceil() as i32;
    
    // Get the center chunk's face direction
    let directions = [
        Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
    ];
    let center_face_direction = directions[center_chunk_id.face_index as usize];
    
    // Calculate axis vectors for the center face
    let axis_a = if center_face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = center_face_direction.cross(axis_a);
    let chunk_size = 2.0 / chunks_per_face as f32;
    
    // Iterate through a square area and check if chunks are within the circle
    for dy in -radius_int..=radius_int {
        for dx in -radius_int..=radius_int {
            // Calculate distance from center chunk (in chunk coordinate space)
            let distance = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt();
            
            // Only include chunks within the radius
            if distance <= radius {
                // Calculate offset chunk coordinates (may go out of bounds)
                // dx and dy are in chunk coordinate space relative to the center chunk
                let offset_chunk_x = center_chunk_id.chunk_x as i32 + dy; // dy is vertical -> chunk_x
                let offset_chunk_y = center_chunk_id.chunk_y as i32 + dx; // dx is horizontal -> chunk_y
                
                // Calculate cube coordinates for this offset chunk
                // Use the same logic as calculate_chunk_position, but allow out-of-bounds coordinates
                let offset_chunk_center_x = (offset_chunk_x as f32 + 0.5) * chunk_size - 1.0;
                let offset_chunk_center_y = (offset_chunk_y as f32 + 0.5) * chunk_size - 1.0;
                
                // Calculate cube position using the center face's coordinate system
                // This can go outside [-1, 1] range when crossing face boundaries
                let point_on_cube = center_face_direction + offset_chunk_center_x * axis_a + offset_chunk_center_y * axis_b;
                
                // Normalize to get sphere position (this correctly maps to adjacent faces)
                let point_on_sphere = point_on_cube.normalize();
                let chunk_world_pos = point_on_sphere * planet_settings.radius;
                
                // Convert world position back to chunk ID (handles cross-face boundaries automatically)
                if let Some(chunk_id) = world_position_to_chunk_id(chunk_world_pos, chunks_per_face, center_chunk_id.lod_level) {
                    chunks_to_keep.insert(chunk_id);
                }
            }
        }
    }
    
    // Unload all chunks that are NOT in the circular render area
    let mut chunks_to_remove = Vec::new();
    let mut total_chunks = 0;
    let mut generating_chunks = 0;
    
    for (entity, chunk) in chunk_query.iter() {
        total_chunks += 1;
        
        // Never unload chunks that are still generating
        if chunk.is_generating {
            generating_chunks += 1;
            continue;
        }
        
        // Unload if chunk is not in the circular render area
        // Compare by face_index, chunk_x, chunk_y (ignore lod_level since chunks might have different LODs)
        let chunk_in_grid = chunks_to_keep.iter().any(|&keep_id| {
            keep_id.face_index == chunk.id.face_index &&
            keep_id.chunk_x == chunk.id.chunk_x &&
            keep_id.chunk_y == chunk.id.chunk_y
        });
        
        if !chunk_in_grid {
            chunks_to_remove.push((chunk.id, entity));
        }
    }
    
    // Only log if chunks are being removed (to reduce spam)
    if chunks_to_remove.len() > 0 {
        println!("[Circular Unload] Removing {} chunks outside render radius. Center: face={}, x={}, y={}, radius={:.1}", 
            chunks_to_remove.len(),
            center_chunk_id.face_index, center_chunk_id.chunk_x, center_chunk_id.chunk_y, chunk_manager.chunk_render_radius);
    }
    
    // Remove chunks
    for (chunk_id, entity) in chunks_to_remove {
        chunk_manager.chunks.remove(&chunk_id);
        chunk_manager.chunk_unload_cooldown.remove(&chunk_id);
        commands.entity(entity).despawn();
    }
}

// Frustum culling system to hide chunks outside camera view
// Uses distance-based culling + back-face culling for chunks on far side of planet
// Only processes chunks that are loaded (have meshes) to avoid culling chunks without transforms
fn cull_chunks_outside_frustum(
    mut chunk_query: Query<(&mut Visibility, &GlobalTransform, &TerrainChunk)>,
    camera_query: Query<&GlobalTransform, (With<Camera3d>, Without<TerrainChunk>)>,
    chunk_manager: Res<ChunkManager>,
    planet_settings: Res<PlanetSettings>,
    mut cull_frame: Local<u32>,
) {
    let Ok(camera_transform) = camera_query.single() else { return; };
    
    let camera_pos = camera_transform.translation();
    let planet_center = Vec3::ZERO; // Planet is at origin
    
    // Vector from planet center to camera
    let camera_dir_from_planet = (camera_pos - planet_center).normalize();
    
    // Use a render distance that matches the chunk loading system
    // Chunks are loaded in a circular grid with radius chunk_render_radius
    // Estimate chunk size: chunks_per_face = 8, so each chunk covers ~2*radius/8 = radius/4
    let chunks_per_face = 8.0;
    let chunk_size = (2.0 * planet_settings.radius) / chunks_per_face;
    
    // Render distance should cover all loaded chunks plus a margin
    // chunk_render_radius is in chunk units, so multiply by chunk_size
    let max_render_distance = chunk_size * chunk_manager.chunk_render_radius * 2.5; // 2.5x margin for safety
    
    let mut culled_count = 0;
    let mut visible_count = 0;
    let mut backface_culled = 0;
    
    for (mut visibility, transform, chunk) in chunk_query.iter_mut() {
        // Only cull chunks that are loaded (have meshes)
        // Chunks without meshes don't have transforms set yet
        if !chunk.is_loaded {
            continue;
        }
        
        let chunk_pos = transform.translation();
        
        // Skip chunks at origin (likely not initialized yet)
        if chunk_pos.length_squared() < 1000.0 {
            continue;
        }
        
        // Back-face culling: Hide chunks on the far side of the planet
        // The chunk's surface normal (approximately) points from planet center through chunk center
        let chunk_normal = (chunk_pos - planet_center).normalize();
        let facing_camera = chunk_normal.dot(camera_dir_from_planet);
        
        // If chunk is facing away from camera (on far side of planet), cull it
        // Use a small threshold to account for horizon chunks
        if facing_camera < -0.1 {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            backface_culled += 1;
            continue;
        }
        
        let distance_to_camera = (chunk_pos - camera_pos).length();
        
        // Distance-based culling
        if distance_to_camera < max_render_distance {
            if *visibility == Visibility::Hidden {
                *visibility = Visibility::Visible;
            }
            visible_count += 1;
        } else {
            if *visibility == Visibility::Visible {
                *visibility = Visibility::Hidden;
            }
            culled_count += 1;
        }
    }
    
    // Debug logging (limit frequency)
    *cull_frame += 1;
    if *cull_frame % 300 == 0 { // Log every ~5 seconds at 60fps
        println!("[Frustum Culling] Visible: {}, Distance culled: {}, Back-face culled: {}",
            visible_count, culled_count, backface_culled);
    }
}




