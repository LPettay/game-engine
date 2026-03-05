// GPU Compute Terrain Generation
// Phase 2: GPU Compute Terrain Generation - Full Integration
// Migrated from Bevy 0.14.2 to Bevy 0.18.1

use bevy::prelude::*;
use bevy::render::{
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_graph::{Node, NodeRunError, RenderGraphContext},
    render_resource::{
        BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
        BindingResource, BindingType, BufferBinding, BufferBindingType,
        BufferDescriptor, BufferUsages, ComputePassDescriptor,
        ComputePipelineDescriptor, Extent3d, TexelCopyBufferInfo, TexelCopyTextureInfo,
        TexelCopyBufferLayout, Origin3d, PipelineCache, ShaderStages,
        StorageTextureAccess, TextureAspect, TextureDescriptor,
        TextureDimension, TextureFormat, TextureUsages,
        TextureViewDescriptor, TextureViewDimension,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    Render, RenderApp, RenderStartup,
};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use crate::plugins::chunked_terrain::ChunkId;
use std::borrow::Cow;
use std::num::NonZeroU64;

pub struct GpuTerrainPlugin;

impl Plugin for GpuTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_gpu_terrain)
            .add_plugins(ExtractResourcePlugin::<GpuTerrainSettings>::default())
            .add_systems(Update, (
                process_gpu_chunk_requests,
                process_gpu_chunk_results,
            ).chain().run_if(in_state(crate::GameState::Playing)));
    }

    fn finish(&self, app: &mut App) {
        // Initialize GPU chunk queue in main world
        app.init_resource::<GpuChunkQueue>();

        // Bevy 0.15+: sub_app_mut always succeeds if RenderApp exists.
        // Use get_sub_app_mut for graceful fallback (e.g., headless mode).
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            // Bevy 0.17+: Use RenderStartup for one-time render resource initialization
            render_app
                .add_systems(RenderStartup, init_terrain_compute_resources)
                .add_systems(Render, create_compute_pipeline);

            // Add compute node to render graph
            // Note: For now, compute dispatch happens in render systems
            // Full render graph integration can be added later if needed
        }
    }
}

#[derive(Resource, Clone, ExtractResource)]
pub struct GpuTerrainSettings {
    pub enabled: bool,
    pub compute_shader_handle: Handle<Shader>,
}

// Uniform buffer struct matching TerrainParams in the shader
// Must follow std140 layout rules:
// - vec3<f32> = 16 bytes (12 data + 4 padding)
// - vec2<f32> = 8 bytes
// - u32/f32 = 4 bytes
// Total size: 96 bytes (must be multiple of 16)
//
// Infinite Scale Rendering Support:
// - observer_scale: meters per pixel at current zoom level
// - min_detail_scale: minimum detail to render at this LOD
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct TerrainParamsUniform {
    pub seed: u32,
    pub _pad_seed: u32, // Padding to align first vec3 to 16 bytes
    pub _pad_seed2: u32,
    pub _pad_seed3: u32,
    pub face_direction: [f32; 3],
    pub _pad_face: f32, // Padding after vec3 (std140 requires vec3 to be 16-byte aligned)
    pub axis_a: [f32; 3],
    pub _pad_axis_a: f32, // Padding after vec3
    pub axis_b: [f32; 3],
    pub _pad_axis_b: f32, // Padding after vec3
    pub chunk_start: [f32; 2], // chunk_start_x, chunk_start_y (vec2 = 8 bytes)
    pub chunk_step: [f32; 2], // chunk_step_x, chunk_step_y (vec2 = 8 bytes)
    pub resolution: u32,
    pub radius: f32,
    pub observer_scale: f32,     // Meters per pixel - enables infinite scale detail
    pub min_detail_scale: f32,   // Minimum detail to render (for LOD optimization)
}

// Chunk GPU generation request
// Supports infinite scale rendering via observer_scale parameter
#[derive(Clone)]
pub struct GpuChunkRequest {
    pub chunk_id: ChunkId,
    pub face_direction: Vec3,
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub chunks_per_face: u32,
    pub resolution: u32,
    pub radius: f32,
    pub seed: u32,
    pub chunk_center: Vec3,
    pub observer_scale: f32,      // Meters per pixel at this LOD level
    pub min_detail_scale: f32,    // Minimum detail to render
}

#[derive(Resource, Default)]
pub struct GpuChunkQueue {
    pub pending: Vec<GpuChunkRequest>,
    pub in_progress: Vec<(GpuChunkRequest, Entity)>, // Request + entity for results
    pub completed: Vec<(GpuChunkRequest, Entity, GpuHeightmapData)>, // Completed with heightmap data
    pub dispatched: Vec<(GpuChunkRequest, Entity, bevy::render::render_resource::Texture, bevy::render::render_resource::Texture)>, // Dispatched with textures for readback
}

// GPU-generated heightmap data
#[derive(Clone)]
pub struct GpuHeightmapData {
    pub heightmap: Vec<f32>, // Height values (resolution x resolution)
    pub colormap: Vec<[u8; 4]>, // Color values (resolution x resolution RGBA)
    pub resolution: u32,
}

fn setup_gpu_terrain(mut commands: Commands, asset_server: Res<AssetServer>) {
    let compute_shader = asset_server.load("shaders/terrain_compute.wgsl");

    commands.insert_resource(GpuTerrainSettings {
        enabled: false, // CPU fallback active — GPU readback not yet implemented
        compute_shader_handle: compute_shader,
    });

    info!("[GPU Terrain] Phase 2 initialized - GPU terrain generation enabled");
    info!("[GPU Terrain] Compute shader loaded: terrain_compute.wgsl");
    info!("[GPU Terrain] Note: Currently using CPU fallback (matches GPU logic)");
    info!("[GPU Terrain] GPU compute dispatch ready when async readback is implemented");
}

/// Build the BindGroupLayoutDescriptor for the terrain compute pipeline.
/// Bevy 0.18: Pipeline descriptors store BindGroupLayoutDescriptor (not BindGroupLayout).
/// The actual BindGroupLayout is created/cached by PipelineCache on demand.
fn terrain_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    let size = std::mem::size_of::<TerrainParamsUniform>() as u64;
    // Verify size is 96 bytes (std140 layout requirement)
    assert_eq!(size, 96, "TerrainParamsUniform must be exactly 96 bytes for std140 layout");
    let min_binding_size = NonZeroU64::new(size);

    let entries = vec![
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size,
            },
            count: None,
        },
        BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::R32Float,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        },
        BindGroupLayoutEntry {
            binding: 2,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::Rgba8Unorm,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        },
        // Binding 3: Detail heightmap for displacement mapping
        BindGroupLayoutEntry {
            binding: 3,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::Rgba8Unorm,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        },
    ];

    BindGroupLayoutDescriptor::new("terrain_compute_bind_group_layout", &entries)
}

// Generate mesh from GPU heightmap data
fn generate_mesh_from_gpu_heightmap(
    request: &GpuChunkRequest,
    heightmap_data: &GpuHeightmapData,
    chunk_center: Vec3,
    radius: f32,
) -> Mesh {
    let resolution = heightmap_data.resolution;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let axis_a = if request.face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = request.face_direction.cross(axis_a);

    let chunk_size = 2.0 / request.chunks_per_face as f32;
    let chunk_start_x = request.chunk_x as f32 * chunk_size - 1.0;
    let chunk_start_y = request.chunk_y as f32 * chunk_size - 1.0;
    let chunk_step_x = chunk_size / (resolution as f32 - 1.0);
    let chunk_step_y = chunk_size / (resolution as f32 - 1.0);

    // Generate vertices from heightmap
    for y in 0..resolution {
        for x in 0..resolution {
            let local_x = chunk_start_x + x as f32 * chunk_step_x;
            let local_y = chunk_start_y + y as f32 * chunk_step_y;

            let point_on_unit_cube = request.face_direction + local_x * axis_a + local_y * axis_b;
            let point_on_unit_sphere = point_on_unit_cube.normalize();

            // Get height from GPU heightmap
            let height_idx = (y * resolution + x) as usize;
            let elevation = heightmap_data.heightmap[height_idx];
            let height = radius * (1.0 + elevation * 0.15);

            // Position relative to chunk center
            let world_pos = point_on_unit_sphere * height;
            let local_pos = world_pos - chunk_center;

            positions.push([local_pos.x, local_pos.y, local_pos.z]);

            // Calculate normal using finite differences (matching CPU version)
            let normal = if x > 0 && x < resolution - 1 && y > 0 && y < resolution - 1 {
                // Calculate positions of neighboring vertices
                let epsilon = 0.001;
                let p_right = (point_on_unit_cube + axis_a * epsilon).normalize();
                let p_up = (point_on_unit_cube + axis_b * epsilon).normalize();

                let h_right_idx = (y * resolution + (x + 1).min(resolution - 1)) as usize;
                let h_up_idx = (((y + 1).min(resolution - 1)) * resolution + x) as usize;
                let h_right = heightmap_data.heightmap[h_right_idx];
                let h_up = heightmap_data.heightmap[h_up_idx];

                let height_right = radius * (1.0 + h_right * 0.15);
                let height_up = radius * (1.0 + h_up * 0.15);

                let pos_right_world = p_right * height_right;
                let pos_up_world = p_up * height_up;

                let pos_right = pos_right_world - chunk_center;
                let pos_up = pos_up_world - chunk_center;
                let pos_current = local_pos;

                // Calculate tangents and normal (outward-facing)
                let tangent_a = pos_right - pos_current;
                let tangent_b = pos_up - pos_current;
                let normal_vec = tangent_a.cross(tangent_b).normalize();

                // Ensure normal points outward (same direction as sphere normal)
                let sphere_normal = point_on_unit_sphere;
                if normal_vec.dot(sphere_normal) < 0.0 {
                    [-normal_vec.x, -normal_vec.y, -normal_vec.z]
                } else {
                    [normal_vec.x, normal_vec.y, normal_vec.z]
                }
            } else {
                // Use sphere normal for boundary vertices (outward-facing)
                [point_on_unit_sphere.x, point_on_unit_sphere.y, point_on_unit_sphere.z]
            };

            normals.push(normal);
            // UV coordinates - use smooth mapping to avoid visible seams
            // Map to 0-1 range with slight overlap to prevent edge artifacts
            let u = x as f32 / (resolution - 1) as f32;
            let v = y as f32 / (resolution - 1) as f32;
            uvs.push([u, v]);

            // Get color from GPU colormap
            let color = heightmap_data.colormap[height_idx];
            colors.push([
                color[0] as f32 / 255.0,
                color[1] as f32 / 255.0,
                color[2] as f32 / 255.0,
                color[3] as f32 / 255.0,
            ]);
        }
    }

    // Generate indices (counter-clockwise winding for outward-facing normals)
    // Matching CPU version winding order
    for y in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let i = y * resolution + x;
            // First triangle: counter-clockwise when viewed from outside
            indices.push(i);
            indices.push(i + resolution + 1);
            indices.push(i + resolution);

            // Second triangle: counter-clockwise when viewed from outside
            indices.push(i);
            indices.push(i + 1);
            indices.push(i + resolution + 1);
        }
    }

    // Bevy 0.18: Mesh::new API unchanged; PrimitiveTopology and RenderAssetUsages
    // are now at bevy::render::mesh::PrimitiveTopology and
    // bevy::render::render_asset::RenderAssetUsages respectively.
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

// Generate heightmap data using CPU (matching GPU shader logic)
// This simulates GPU generation until texture readback is implemented
fn generate_heightmap_cpu(
    request: &GpuChunkRequest,
    terrain_noise: &crate::plugins::terrain::TerrainNoise,
) -> GpuHeightmapData {
    let resolution = request.resolution;
    let mut heightmap = Vec::with_capacity((resolution * resolution) as usize);
    let mut colormap = Vec::with_capacity((resolution * resolution) as usize);

    let axis_a = if request.face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = request.face_direction.cross(axis_a);
    let chunk_size = 2.0 / request.chunks_per_face as f32;
    let chunk_start_x = request.chunk_x as f32 * chunk_size - 1.0;
    let chunk_start_y = request.chunk_y as f32 * chunk_size - 1.0;
    let chunk_step_x = chunk_size / (resolution as f32 - 1.0);
    let chunk_step_y = chunk_size / (resolution as f32 - 1.0);

    for y in 0..resolution {
        for x in 0..resolution {
            let local_x = chunk_start_x + x as f32 * chunk_step_x;
            let local_y = chunk_start_y + y as f32 * chunk_step_y;

            let point_on_unit_cube = request.face_direction + local_x * axis_a + local_y * axis_b;
            let point_on_unit_sphere = point_on_unit_cube.normalize();

            // Get elevation and color using terrain noise (matches GPU shader logic)
            let elevation = terrain_noise.get_elevation(point_on_unit_sphere);
            let (_, color) = terrain_noise.get_data(point_on_unit_sphere);

            heightmap.push(elevation);

            // Convert Color to RGBA bytes
            // Extract LinearRgba from Color enum
            let color_linear = match color {
                bevy::prelude::Color::LinearRgba(linear) => linear,
                bevy::prelude::Color::Srgba(srgb) => bevy::prelude::LinearRgba::from(srgb),
                _ => bevy::prelude::LinearRgba::new(0.5, 0.5, 0.5, 1.0),
            };
            colormap.push([
                (color_linear.red * 255.0) as u8,
                (color_linear.green * 255.0) as u8,
                (color_linear.blue * 255.0) as u8,
                (color_linear.alpha * 255.0) as u8,
            ]);
        }
    }

    GpuHeightmapData {
        heightmap,
        colormap,
        resolution,
    }
}

// System to process GPU chunk requests
// Processes pending chunks and dispatched chunks (GPU-computed)
fn process_gpu_chunk_requests(
    mut gpu_queue: ResMut<GpuChunkQueue>,
    chunk_manager: Res<crate::plugins::chunked_terrain::ChunkManager>,
    mut gpu_chunks_processed: Local<u32>,
) {
    // Process pending chunks - generate on CPU (matches GPU logic)
    let pending = std::mem::take(&mut gpu_queue.pending);

    if !pending.is_empty() {
        let terrain_noise = crate::plugins::terrain::TerrainNoise::new(chunk_manager.terrain_seed);

        *gpu_chunks_processed += pending.len() as u32;
        if *gpu_chunks_processed <= 10 {
            info!("[GPU Terrain] Processing {} GPU chunk requests (CPU fallback, matches GPU logic)", pending.len());
        }

        for request in pending {
            if let Some(pos) = gpu_queue.in_progress.iter().position(|(r, _)| r.chunk_id == request.chunk_id) {
                let (_, entity) = gpu_queue.in_progress.remove(pos);

                // Generate heightmap using CPU (matches GPU shader logic exactly)
                let heightmap_data = generate_heightmap_cpu(&request, &terrain_noise);
                gpu_queue.completed.push((request, entity, heightmap_data));
            }
        }
    }

    // Process dispatched chunks (GPU-computed, waiting for readback)
    // For now, fallback to CPU generation until async buffer mapping is implemented
    let dispatched = std::mem::take(&mut gpu_queue.dispatched);

    if !dispatched.is_empty() {
        let terrain_noise = crate::plugins::terrain::TerrainNoise::new(chunk_manager.terrain_seed);

        for (request, entity, _texture1, _texture2) in dispatched {
            // Generate heightmap using CPU (matches GPU shader logic)
            // Real GPU readback will replace this when async buffer mapping is implemented
            let heightmap_data = generate_heightmap_cpu(&request, &terrain_noise);
            gpu_queue.completed.push((request, entity, heightmap_data));
        }
    }
}

// System to process completed GPU work and generate meshes from heightmaps
fn process_gpu_chunk_results(
    mut gpu_queue: ResMut<GpuChunkQueue>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::plugins::planet::PlanetMaterial>>,
    mut chunk_query: Query<&mut crate::plugins::chunked_terrain::TerrainChunk>,
    chunk_manager: Res<crate::plugins::chunked_terrain::ChunkManager>,
    _planet_settings: Res<crate::plugins::planet::PlanetSettings>,
) {
    // Process completed GPU work
    let completed = std::mem::take(&mut gpu_queue.completed);

    for (request, entity, heightmap_data) in completed {
        // Generate mesh from GPU heightmap
        let mesh = generate_mesh_from_gpu_heightmap(
            &request,
            &heightmap_data,
            request.chunk_center,
            request.radius,
        );

        let mesh_handle = meshes.add(mesh);

        // Create material
        let material_handle = materials.add(crate::plugins::planet::PlanetMaterial {
            scaling: bevy::math::Vec4::new(0.1, 0.0, 0.8, 0.0),
            detail_texture: chunk_manager.detail_texture_handle.clone().unwrap_or_default(),
        });

        // Update chunk component to mark as loaded and not generating
        if let Ok(mut chunk) = chunk_query.get_mut(entity) {
            chunk.is_generating = false;
            chunk.is_loaded = true;
            chunk.mesh_handle = Some(mesh_handle.clone());
        }

        // Add mesh and material to chunk entity
        // Bevy 0.15+: MaterialMeshBundle removed. Use individual components:
        //   Mesh3d(handle) + MeshMaterial3d(handle) + Transform + Visibility
        // Mesh3d has required components for Transform and Visibility via
        // Bevy's required components system, but we set them explicitly for clarity.
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
                Transform::from_translation(request.chunk_center),
                Visibility::Visible,
            ));

            // Chunk is now loaded with mesh and material
        }
    }
}

// Render world resources

#[derive(Resource)]
pub struct TerrainComputePipeline {
    pub pipeline_id: Option<bevy::render::render_resource::CachedComputePipelineId>,
    /// Bevy 0.18: Store the descriptor so we can retrieve the cached BindGroupLayout
    /// from PipelineCache when creating bind groups at runtime.
    pub bind_group_layout_descriptor: BindGroupLayoutDescriptor,
}

impl Default for TerrainComputePipeline {
    fn default() -> Self {
        Self {
            pipeline_id: None,
            bind_group_layout_descriptor: terrain_bind_group_layout_descriptor(),
        }
    }
}

/// Bevy 0.17+: Use RenderStartup for one-time initialization of render world resources.
/// This replaces the old pattern of init_resource + lazy initialization in Render systems.
fn init_terrain_compute_resources(mut commands: Commands) {
    commands.insert_resource(TerrainComputePipeline::default());
    info!("[GPU Terrain] Render resources initialized via RenderStartup");
}

// System to create compute pipeline when shader is ready
fn create_compute_pipeline(
    mut compute_pipeline: ResMut<TerrainComputePipeline>,
    gpu_terrain_settings: Option<Res<GpuTerrainSettings>>,
    pipeline_cache: ResMut<PipelineCache>,
) {
    // Only create pipeline once
    if compute_pipeline.pipeline_id.is_some() {
        return;
    }

    // Queue pipeline creation - Bevy will handle async shader loading
    if let Some(settings) = gpu_terrain_settings {
        // Bevy 0.18: ComputePipelineDescriptor.layout now takes
        // Vec<BindGroupLayoutDescriptor> instead of Vec<BindGroupLayout>.
        // The PipelineCache creates and caches the actual BindGroupLayout objects.
        let pipeline_descriptor = ComputePipelineDescriptor {
            label: Some(Cow::Borrowed("terrain_compute_pipeline")),
            layout: vec![compute_pipeline.bind_group_layout_descriptor.clone()],
            shader: settings.compute_shader_handle.clone(),
            shader_defs: vec![],
            // Bevy 0.18: entry_point is now Option<Cow<'static, str>>
            entry_point: Some(Cow::Borrowed("generate_terrain")),
            push_constant_ranges: vec![],
            // TODO: verify zero_initialize_workgroup_memory field for Bevy 0.18
            ..default()
        };

        // Queue pipeline creation - pipeline cache will handle async shader loading
        let pipeline_id = pipeline_cache.queue_compute_pipeline(pipeline_descriptor);
        compute_pipeline.pipeline_id = Some(pipeline_id);
        info!("[GPU Terrain] Compute pipeline queued for creation");
    }
}

// Custom render node for terrain compute dispatch
pub struct TerrainComputeNode;

impl Node for TerrainComputeNode {
    // Bevy 0.15+: Node::run signature uses lifetime parameter on RenderContext
    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext<'_>,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let compute_pipeline_state = world.resource::<TerrainComputePipeline>();
        let gpu_queue = world.resource::<GpuChunkQueue>();
        let render_device = world.resource::<RenderDevice>();
        let render_queue = world.resource::<RenderQueue>();

        // Check if pipeline is ready
        if let Some(pipeline_id) = compute_pipeline_state.pipeline_id {
            if let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline_id) {
                // Bevy 0.18: Retrieve the cached BindGroupLayout from PipelineCache
                // using the BindGroupLayoutDescriptor stored in our pipeline resource.
                let bind_group_layout = pipeline_cache.get_bind_group_layout(
                    &compute_pipeline_state.bind_group_layout_descriptor,
                );

                // Bevy 0.15+: command_encoder() is an accessor method (field is private)
                let command_encoder = render_context.command_encoder();

                // Process pending chunks (limit per frame)
                // Note: Cannot mutate gpu_queue here - main world system handles that
                // This node just dispatches compute shaders for chunks already in pending
                let pending_count = gpu_queue.pending.len().min(5);

                for request in gpu_queue.pending.iter().take(pending_count) {
                    // Calculate cube-sphere coordinates
                    let axis_a = if request.face_direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
                    let axis_b = request.face_direction.cross(axis_a);
                    let chunk_size = 2.0 / request.chunks_per_face as f32;
                    let chunk_start_x = request.chunk_x as f32 * chunk_size - 1.0;
                    let chunk_start_y = request.chunk_y as f32 * chunk_size - 1.0;
                    let chunk_step_x = chunk_size / (request.resolution as f32 - 1.0);
                    let chunk_step_y = chunk_size / (request.resolution as f32 - 1.0);

                    // Create uniform buffer with cube-sphere parameters
                    // Includes observer_scale for infinite scale rendering
                    let params = TerrainParamsUniform {
                        seed: request.seed,
                        _pad_seed: 0,
                        _pad_seed2: 0,
                        _pad_seed3: 0,
                        face_direction: [request.face_direction.x, request.face_direction.y, request.face_direction.z],
                        _pad_face: 0.0,
                        axis_a: [axis_a.x, axis_a.y, axis_a.z],
                        _pad_axis_a: 0.0,
                        axis_b: [axis_b.x, axis_b.y, axis_b.z],
                        _pad_axis_b: 0.0,
                        chunk_start: [chunk_start_x, chunk_start_y],
                        chunk_step: [chunk_step_x, chunk_step_y],
                        resolution: request.resolution,
                        radius: request.radius,
                        observer_scale: request.observer_scale,
                        min_detail_scale: request.min_detail_scale,
                    };

                    let params_bytes = bytemuck::bytes_of(&params);
                    let uniform_buffer = render_device.create_buffer(&BufferDescriptor {
                        label: Some("terrain_params_uniform"),
                        size: params_bytes.len() as u64,
                        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });

                    render_queue.write_buffer(&uniform_buffer, 0, params_bytes);

                    // Create storage textures for heightmap and colormap
                    let heightmap_texture = render_device.create_texture(
                        &TextureDescriptor {
                            label: Some("terrain_heightmap"),
                            size: Extent3d {
                                width: request.resolution,
                                height: request.resolution,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::R32Float,
                            usage: TextureUsages::STORAGE_BINDING
                                | TextureUsages::COPY_SRC,
                            view_formats: &[],
                        },
                    );

                    let colormap_texture = render_device.create_texture(
                        &TextureDescriptor {
                            label: Some("terrain_colormap"),
                            size: Extent3d {
                                width: request.resolution,
                                height: request.resolution,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8Unorm,
                            usage: TextureUsages::STORAGE_BINDING
                                | TextureUsages::COPY_SRC,
                            view_formats: &[],
                        },
                    );

                    // Detail heightmap for displacement mapping
                    let detail_heightmap_texture = render_device.create_texture(
                        &TextureDescriptor {
                            label: Some("terrain_detail_heightmap"),
                            size: Extent3d {
                                width: request.resolution,
                                height: request.resolution,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8Unorm,
                            usage: TextureUsages::STORAGE_BINDING
                                | TextureUsages::COPY_SRC,
                            view_formats: &[],
                        },
                    );

                    let heightmap_view = heightmap_texture.create_view(
                        &TextureViewDescriptor::default(),
                    );
                    let colormap_view = colormap_texture.create_view(
                        &TextureViewDescriptor::default(),
                    );
                    let detail_heightmap_view = detail_heightmap_texture.create_view(
                        &TextureViewDescriptor::default(),
                    );

                    // Create bind group
                    // Bevy 0.18: Use pipeline_cache.get_bind_group_layout() to get
                    // the cached BindGroupLayout for bind group creation.
                    let bind_group = render_device.create_bind_group(
                        Some("terrain_compute_bind_group"),
                        &bind_group_layout,
                        &[
                            BindGroupEntry {
                                binding: 0,
                                resource: BindingResource::Buffer(BufferBinding {
                                    buffer: &uniform_buffer,
                                    offset: 0,
                                    size: Some(NonZeroU64::new(
                                        std::mem::size_of::<TerrainParamsUniform>() as u64,
                                    ).unwrap()),
                                }),
                            },
                            BindGroupEntry {
                                binding: 1,
                                resource: BindingResource::TextureView(&heightmap_view),
                            },
                            BindGroupEntry {
                                binding: 2,
                                resource: BindingResource::TextureView(&colormap_view),
                            },
                            BindGroupEntry {
                                binding: 3,
                                resource: BindingResource::TextureView(&detail_heightmap_view),
                            },
                        ],
                    );

                    // Dispatch compute shader
                    {
                        let mut compute_pass = command_encoder.begin_compute_pass(
                            &ComputePassDescriptor::default(),
                        );

                        compute_pass.set_pipeline(compute_pipeline);
                        compute_pass.set_bind_group(0, &bind_group, &[]);

                        // Dispatch workgroups (16x16 workgroup size)
                        let workgroups_x = (request.resolution + 15) / 16;
                        let workgroups_y = (request.resolution + 15) / 16;
                        compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
                    }

                    // Copy textures to buffers for readback
                    let heightmap_buffer_size = (request.resolution * request.resolution * 4) as u64; // R32Float = 4 bytes per pixel
                    let colormap_buffer_size = (request.resolution * request.resolution * 4) as u64; // RGBA8Unorm = 4 bytes per pixel

                    let heightmap_readback_buffer = render_device.create_buffer(&BufferDescriptor {
                        label: Some("heightmap_readback"),
                        size: heightmap_buffer_size,
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });

                    let colormap_readback_buffer = render_device.create_buffer(&BufferDescriptor {
                        label: Some("colormap_readback"),
                        size: colormap_buffer_size,
                        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });

                    // Copy texture to buffer
                    // TODO: verify TexelCopyTextureInfo/TexelCopyBufferInfo render API for Bevy 0.18
                    // (these wgpu wrapper types may have been renamed or restructured)
                    command_encoder.copy_texture_to_buffer(
                        TexelCopyTextureInfo {
                            texture: &heightmap_texture,
                            mip_level: 0,
                            origin: Origin3d::ZERO,
                            aspect: TextureAspect::All,
                        },
                        TexelCopyBufferInfo {
                            buffer: &heightmap_readback_buffer,
                            layout: TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(request.resolution * 4),
                                rows_per_image: Some(request.resolution),
                            },
                        },
                        Extent3d {
                            width: request.resolution,
                            height: request.resolution,
                            depth_or_array_layers: 1,
                        },
                    );

                    command_encoder.copy_texture_to_buffer(
                        TexelCopyTextureInfo {
                            texture: &colormap_texture,
                            mip_level: 0,
                            origin: Origin3d::ZERO,
                            aspect: TextureAspect::All,
                        },
                        TexelCopyBufferInfo {
                            buffer: &colormap_readback_buffer,
                            layout: TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(request.resolution * 4),
                                rows_per_image: Some(request.resolution),
                            },
                        },
                        Extent3d {
                            width: request.resolution,
                            height: request.resolution,
                            depth_or_array_layers: 1,
                        },
                    );

                    // Store textures and buffers for async readback
                    // Note: Actual readback happens asynchronously via buffer mapping
                    // For now, we'll use CPU fallback until async readback is fully implemented

                    // Mark chunk as dispatched - will be processed when buffers are ready
                    // For now, fallback to CPU generation
                }
            }
        }

        Ok(())
    }
}

// ChunkId is now public in chunked_terrain, can be used directly
