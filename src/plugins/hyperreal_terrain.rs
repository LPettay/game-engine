// Hyperreal Terrain Material System
// Provides high-fidelity terrain rendering with:
// - Vertex displacement from heightmaps
// - Parallax occlusion mapping for micro-detail
// - Multi-scale normal mapping
// - Distance-based LOD for all effects

use bevy::{
    prelude::*,
    render::{
        render_resource::{AsBindGroup, ShaderRef, ShaderType},
        texture::{ImageSampler, ImageSamplerDescriptor, ImageAddressMode, ImageFilterMode},
    },
    pbr::{MaterialPipeline, MaterialPipelineKey},
};

pub struct HyperrealTerrainPlugin;

impl Plugin for HyperrealTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<HyperrealTerrainMaterial>::default())
           .init_resource::<HyperrealTerrainSettings>()
           .add_systems(Startup, setup_default_textures)
           .add_systems(Update, update_terrain_camera_position);
    }
}

/// Global settings for hyperreal terrain rendering
#[derive(Resource)]
pub struct HyperrealTerrainSettings {
    /// Scale of detail texture sampling
    pub detail_scale: f32,
    /// Strength of vertex displacement (in world units)
    pub displacement_strength: f32,
    /// Base roughness of terrain surface
    pub roughness: f32,
    /// Metallic value (usually 0 for terrain)
    pub metallic: f32,
    /// Scale of parallax effect
    pub parallax_scale: f32,
    /// Number of parallax layers (quality vs performance)
    pub parallax_layers: f32,
    /// Strength of normal mapping
    pub normal_strength: f32,
    /// Whether hyperreal features are enabled
    pub enabled: bool,
}

impl Default for HyperrealTerrainSettings {
    fn default() -> Self {
        Self {
            detail_scale: 0.02,
            displacement_strength: 50.0,
            roughness: 0.85,
            metallic: 0.0,
            parallax_scale: 0.05,
            parallax_layers: 16.0,
            normal_strength: 0.8,
            enabled: true,
        }
    }
}

/// Default texture handles for terrain materials
#[derive(Resource)]
pub struct TerrainTextureHandles {
    pub heightmap: Handle<Image>,
    pub detail: Handle<Image>,
    pub normal: Handle<Image>,
}

/// Uniform data sent to the shader
#[derive(Clone, Copy, ShaderType)]
pub struct TerrainMaterialUniform {
    /// x = detail_scale, y = displacement_strength, z = roughness, w = metallic
    pub params: Vec4,
    /// x = parallax_scale, y = parallax_layers, z = normal_strength, w = reserved
    pub parallax_params: Vec4,
    /// Camera position for LOD and parallax calculations
    pub camera_pos: Vec3,
    pub _padding: f32,
}

/// Hyperreal terrain material with all the bells and whistles
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct HyperrealTerrainMaterial {
    #[uniform(0)]
    pub params: Vec4,
    #[uniform(1)]
    pub parallax_params: Vec4,
    #[uniform(2)]
    pub camera_pos: Vec4,
    
    /// Heightmap for vertex displacement and parallax
    #[texture(3)]
    #[sampler(4)]
    pub heightmap: Handle<Image>,
    
    /// Detail/albedo texture for surface color variation
    #[texture(5)]
    #[sampler(6)]
    pub detail_texture: Handle<Image>,
    
    /// Normal map for surface detail
    #[texture(7)]
    #[sampler(8)]
    pub normal_map: Handle<Image>,
}

impl Material for HyperrealTerrainMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terrain_hyperreal.wgsl".into()
    }
    
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_hyperreal.wgsl".into()
    }
    
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Enable vertex colors for both vertex and fragment stages
        descriptor.vertex.shader_defs.push("VERTEX_COLORS".into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("VERTEX_COLORS".into());
        }
        Ok(())
    }
}

impl HyperrealTerrainMaterial {
    /// Create a new hyperreal terrain material with default settings
    pub fn new(
        heightmap: Handle<Image>,
        detail_texture: Handle<Image>,
        normal_map: Handle<Image>,
        settings: &HyperrealTerrainSettings,
    ) -> Self {
        Self {
            params: Vec4::new(
                settings.detail_scale,
                settings.displacement_strength,
                settings.roughness,
                settings.metallic,
            ),
            parallax_params: Vec4::new(
                settings.parallax_scale,
                settings.parallax_layers,
                settings.normal_strength,
                0.0,
            ),
            camera_pos: Vec4::ZERO,
            heightmap,
            detail_texture,
            normal_map,
        }
    }
    
    /// Create a simplified material (for performance or fallback)
    pub fn simple(
        detail_texture: Handle<Image>,
        settings: &HyperrealTerrainSettings,
    ) -> Self {
        Self {
            params: Vec4::new(
                settings.detail_scale,
                0.0, // No displacement
                settings.roughness,
                settings.metallic,
            ),
            parallax_params: Vec4::ZERO, // No parallax
            camera_pos: Vec4::ZERO,
            heightmap: detail_texture.clone(),
            detail_texture: detail_texture.clone(),
            normal_map: detail_texture,
        }
    }
    
    /// Update camera position for LOD calculations
    pub fn set_camera_position(&mut self, pos: Vec3) {
        self.camera_pos = pos.extend(1.0);
    }
}

/// Component to mark entities that need camera position updates
#[derive(Component)]
pub struct HyperrealTerrainChunk {
    pub material_handle: Handle<HyperrealTerrainMaterial>,
}

/// Setup default textures for terrain
fn setup_default_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    // Create default heightmap (flat gray)
    let heightmap = create_default_texture(&mut images, [128, 128, 128, 255], "heightmap");
    
    // Create default detail texture (neutral gray)
    let detail = create_default_texture(&mut images, [180, 180, 180, 255], "detail");
    
    // Create default normal map (flat normal pointing up)
    let normal = create_default_texture(&mut images, [128, 128, 255, 255], "normal");
    
    commands.insert_resource(TerrainTextureHandles {
        heightmap,
        detail,
        normal,
    });
}

fn create_default_texture(
    images: &mut ResMut<Assets<Image>>,
    color: [u8; 4],
    _name: &str,
) -> Handle<Image> {
    let size = 64;
    let data: Vec<u8> = (0..size * size)
        .flat_map(|_| color)
        .collect();
    
    let mut image = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    
    // Set up sampler for tiling
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    
    images.add(image)
}

/// System to update camera position in terrain materials
fn update_terrain_camera_position(
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    chunk_query: Query<&HyperrealTerrainChunk>,
    mut materials: ResMut<Assets<HyperrealTerrainMaterial>>,
) {
    let Ok(camera_transform) = camera_query.get_single() else { return };
    let camera_pos = camera_transform.translation();
    
    // Update all terrain materials with current camera position
    for chunk in chunk_query.iter() {
        if let Some(material) = materials.get_mut(&chunk.material_handle) {
            material.set_camera_position(camera_pos);
        }
    }
}

/// Generate a heightmap texture from terrain noise data
pub fn generate_heightmap_texture(
    heightmap_data: &[f32],
    resolution: u32,
    images: &mut ResMut<Assets<Image>>,
) -> Handle<Image> {
    let mut data = Vec::with_capacity((resolution * resolution * 4) as usize);
    
    for &height in heightmap_data {
        // Normalize height to 0-255 range
        let normalized = ((height.clamp(0.0, 1.0)) * 255.0) as u8;
        data.push(normalized);
        data.push(normalized);
        data.push(normalized);
        data.push(255);
    }
    
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    
    images.add(image)
}

/// Generate a normal map from heightmap data
pub fn generate_normal_map_from_heightmap(
    heightmap_data: &[f32],
    resolution: u32,
    strength: f32,
    images: &mut ResMut<Assets<Image>>,
) -> Handle<Image> {
    let mut data = Vec::with_capacity((resolution * resolution * 4) as usize);
    let res = resolution as i32;
    
    for y in 0..res {
        for x in 0..res {
            let idx = |px: i32, py: i32| -> usize {
                let px = px.clamp(0, res - 1) as usize;
                let py = py.clamp(0, res - 1) as usize;
                py * resolution as usize + px
            };
            
            // Sample neighboring heights
            let h_left = heightmap_data[idx(x - 1, y)];
            let h_right = heightmap_data[idx(x + 1, y)];
            let h_down = heightmap_data[idx(x, y - 1)];
            let h_up = heightmap_data[idx(x, y + 1)];
            
            // Calculate normal from height differences
            let dx = (h_right - h_left) * strength;
            let dy = (h_up - h_down) * strength;
            
            let normal = Vec3::new(-dx, -dy, 1.0).normalize();
            
            // Encode normal to RGB (0-1 range mapped to 0-255)
            let r = ((normal.x * 0.5 + 0.5) * 255.0) as u8;
            let g = ((normal.y * 0.5 + 0.5) * 255.0) as u8;
            let b = ((normal.z * 0.5 + 0.5) * 255.0) as u8;
            
            data.push(r);
            data.push(g);
            data.push(b);
            data.push(255);
        }
    }
    
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    
    images.add(image)
}

