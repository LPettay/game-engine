use bevy::{
    prelude::*,
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task},
};
use bevy_rapier3d::prelude::*;
use crate::plugins::terrain::TerrainNoise;
use noise::{NoiseFn, Fbm, Perlin, MultiFractal}; // Added MultiFractal
use futures_lite::future;

pub struct PlanetPlugin;

use crate::plugins::camera::CameraState;
use crate::GameState; // Added

impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlanetSettings>()
           .add_plugins(MaterialPlugin::<PlanetMaterial>::default()) 
           // .add_systems(Startup, setup_planet) // Removed Startup system
           .add_systems(OnEnter(GameState::Loading), start_planet_generation) // Start generation on Loading
           .add_systems(Update, check_planet_generation.run_if(in_state(GameState::Loading))) // Check task
           .add_systems(Update, (planet_rotation_system, update_planet_material).run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
pub struct Planet;

#[derive(Component)]
struct PlanetGenerationTask(Task<(Mesh, Mesh, Image)>); // Task Component: (visual_mesh, collider_mesh, texture)

#[derive(Resource)]
pub struct PlanetSettings {
    pub gravity: f32,
    pub radius: f32,
    pub atmosphere_height: f32,
    pub air_density_sea_level: f32,
    pub rayleigh_scattering: Vec3,
    pub rayleigh_scale_height: f32,
    pub mie_scattering: f32,
    pub mie_scale_height: f32,
    pub mie_asymmetry: f32,
    pub atmosphere_enabled: bool,
    pub soft_lighting: bool,
    pub terrain_seed: u32,
}

impl Default for PlanetSettings {
    fn default() -> Self {
        Self {
            gravity: 9.8,
            radius: 20000.0, 
            atmosphere_height: 10000.0, 
            air_density_sea_level: 0.02, 
            rayleigh_scattering: Vec3::new(0.000055, 0.00013, 0.000224) * 2.0, 
            rayleigh_scale_height: 2500.0, 
            mie_scattering: 0.00021 * 2.0,
            mie_scale_height: 1000.0, 
            mie_asymmetry: 0.76,
            atmosphere_enabled: true,
            soft_lighting: false,
            terrain_seed: 12345,
        }
    }
}

// Custom Material
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PlanetMaterial {
    #[uniform(0)]
    pub scaling: Vec4, // xy = noise scale, z = roughness, w = metallic/lighting_mode
    #[texture(1)]
    #[sampler(2)]
    pub detail_texture: Handle<Image>,
}

impl Material for PlanetMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/planet.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

fn start_planet_generation(mut commands: Commands, planet_settings: Res<PlanetSettings>) {
    let thread_pool = AsyncComputeTaskPool::get();
    
    let radius = planet_settings.radius;
    let seed = planet_settings.terrain_seed;

    // Clone needed data for the thread
    let task = thread_pool.spawn(async move {
        let visual_resolution = 400; // Increased for higher detail
        let collider_resolution = 150; // Increased for better collision accuracy
        let terrain_noise = TerrainNoise::new(seed);
        
        let visual_mesh = generate_planet_mesh(visual_resolution, radius, &terrain_noise);
        let collider_mesh = generate_planet_mesh(collider_resolution, radius, &terrain_noise);
        let texture = generate_noise_texture_data();
        
        (visual_mesh, collider_mesh, texture)
    });

    commands.spawn(PlanetGenerationTask(task));
}

fn check_planet_generation(
    mut commands: Commands,
    mut planet_materials: ResMut<Assets<PlanetMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    planet_settings: Res<PlanetSettings>,
    mut query: Query<(Entity, &mut PlanetGenerationTask)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for (entity, mut task) in query.iter_mut() {
        if let Some((visual_mesh, collider_mesh, image)) = future::block_on(future::poll_once(&mut task.0)) {
            // Task Complete
            
            // Add Assets
            let mesh_handle = meshes.add(visual_mesh);
            let texture_handle = images.add(image);
            
            // Create mesh collider from lower-resolution collision mesh
            // Using TriMesh for accurate collision with terrain features (mountains, valleys, etc.)
            println!("Creating mesh collider from {} vertex mesh...", collider_mesh.attribute(Mesh::ATTRIBUTE_POSITION).map(|a| a.len()).unwrap_or(0));
            let collider = match Collider::from_bevy_mesh(&collider_mesh, &ComputedColliderShape::TriMesh(default())) {
                Some(collider) => {
                    println!("Successfully created mesh collider");
                    collider
                }
                None => {
                    eprintln!("WARNING: Failed to create mesh collider, falling back to sphere collider");
                    Collider::ball(planet_settings.radius)
                }
            };

            // Spawn Planet
            // Use KinematicVelocityBased instead of Fixed to allow manual rotation while maintaining collision
            // Hide the planet mesh by default since we're using chunked terrain
            let planet_entity = commands.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(planet_materials.add(PlanetMaterial {
                    scaling: Vec4::new(0.02, 0.0, 0.95, 0.05), // Reduced noise scale (0.02) for finer detail, soft_lighting=0
                    detail_texture: texture_handle,
                })),
                Visibility::Hidden, // Hide by default - chunked terrain will render instead
                Transform::default(),
                collider,
                RigidBody::KinematicVelocityBased, // Changed from Fixed to allow manual transform updates
                Velocity::default(), // Required for KinematicVelocityBased
                Planet,
            )).id();

            // Ocean Sphere — slightly below terrain surface so land is visible
            let ocean_radius = planet_settings.radius * 0.995;
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(ocean_radius).mesh().ico(6).unwrap())),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(0.0, 0.2, 0.8, 0.6),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.1,
                    metallic: 0.5,
                    reflectance: 0.8,
                    ..default()
                })),
                ChildOf(planet_entity),
            ));

            // Clean up task
            commands.entity(entity).despawn();

            // Transition to Playing
            next_state.set(GameState::Playing);
        }
    }
}

// Modified to return Image, not Handle
fn generate_noise_texture_data() -> Image {
    let size = 2048; // Increased from 1024 to 2048 for higher detail
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    // Fbm needs MultiFractal imported
    let noise = Fbm::<Perlin>::new(0).set_frequency(10.0); 

    for y in 0..size {
        for x in 0..size {
            let nx = x as f64 / size as f64;
            let ny = y as f64 / size as f64;
            
            let val = noise.get([nx, ny, 0.0]);
            let normalized = ((val + 1.0) * 0.5).clamp(0.0, 1.0); 
            
            let v = (normalized * 255.0) as u8;
            pixels.push(v); 
            pixels.push(v); 
            pixels.push(v); 
            pixels.push(255); 
        }
    }

    Image::new_fill(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn planet_rotation_system(
    mut planet_query: Query<&mut Transform, With<Planet>>,
    camera_state: Res<CameraState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    // Planet rotation works in Orbital mode only
    if *camera_state != CameraState::Orbital {
        return;
    }

    // Faster rotation speed for more visible effect
    let rotation_speed = 1.5 * time.delta_secs();
    let mut rotation = 0.0;

    if keyboard_input.pressed(KeyCode::KeyA) {
        rotation += rotation_speed;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        rotation -= rotation_speed;
    }

    if rotation != 0.0 {
        // Rotate planet - collider and child chunks will rotate automatically
        for mut transform in planet_query.iter_mut() {
            transform.rotate_y(rotation);
            // Debug: Log rotation
            println!("[Planet Rotation] Rotating Y by {:.4} rad, total rotation: {:.2}°", 
                rotation, transform.rotation.to_euler(bevy::math::EulerRot::YXZ).0.to_degrees());
        }
    }
}

fn update_planet_material(
    mut materials: ResMut<Assets<PlanetMaterial>>,
    query: Query<&MeshMaterial3d<PlanetMaterial>, With<Planet>>,
    planet_settings: Res<PlanetSettings>,
) {
    if planet_settings.is_changed() {
        if let Ok(handle) = query.single() {
            if let Some(material) = materials.get_mut(&handle.0) {
                // We pack soft_lighting boolean into scaling.y (was unused, actually x is noise scale)
                // Wait, x is used. y is unused.
                // scaling: x=scale, y=soft_light, z=roughness, w=metallic
                let lighting_val = if planet_settings.soft_lighting { 1.0 } else { 0.0 };
                material.scaling.y = lighting_val; 
            }
        }
    }
}

fn generate_planet_mesh(resolution: u32, radius: f32, noise: &TerrainNoise) -> Mesh {
    // ... implementation same as before ...
    // For brevity, I am not repeating the full 100 lines of mesh gen here unless requested.
    // But I MUST include it for the code to compile if I'm overwriting the file.
    // I will include the full body.
    
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let directions = [
        Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
    ];

    for direction in directions {
        generate_face(direction, resolution, radius, noise, &mut positions, &mut normals, &mut uvs, &mut colors, &mut indices);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn generate_face(
    local_up: Vec3, 
    resolution: u32,
    radius: f32,
    noise: &TerrainNoise,
    positions: &mut Vec<[f32; 3]>, 
    normals: &mut Vec<[f32; 3]>, 
    uvs: &mut Vec<[f32; 2]>, 
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>
) {
    let axis_a = if local_up.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let axis_b = local_up.cross(axis_a);
    
    let start_index = positions.len() as u32;

    for y in 0..resolution {
        for x in 0..resolution {
            let i = x + y * resolution;
            let percent = Vec2::new(x as f32, y as f32) / (resolution as f32 - 1.0);
            
            // Cube Sphere mapping
            let point_on_unit_cube = local_up + (percent.x - 0.5) * 2.0 * axis_a + (percent.y - 0.5) * 2.0 * axis_b;
            let point_on_unit_sphere = point_on_unit_cube.normalize(); // Spherify
            
            // Get Terrain Data
            let (elevation_factor, color) = noise.get_data(point_on_unit_sphere);
            
            // Apply elevation - increased multiplier for more dramatic terrain variation
            // With radius=20000, 0.15 multiplier gives up to 3000m elevation variation
            let height_mult = 1.0 + elevation_factor * 0.15;
            let final_position = point_on_unit_sphere * radius * height_mult;

            // Calculate Normals using finite difference
            let epsilon = 0.001; 
            
            // Sample 1: Right (+u)
            let p_right_cube = point_on_unit_cube + axis_a * epsilon;
            let p_right_sphere = p_right_cube.normalize();
            let h_right = noise.get_elevation(p_right_sphere);
            let pos_right = p_right_sphere * radius * (1.0 + h_right * 0.15);
            
            // Sample 2: Up (+v)
            let p_up_cube = point_on_unit_cube + axis_b * epsilon;
            let p_up_sphere = p_up_cube.normalize();
            let h_up = noise.get_elevation(p_up_sphere);
            let pos_up = p_up_sphere * radius * (1.0 + h_up * 0.15);
            
            // Tangents
            let tangent_a = pos_right - final_position;
            let tangent_b = pos_up - final_position;
            
            // Normal is Cross Product
            let normal = tangent_a.cross(tangent_b).normalize();

            positions.push(final_position.to_array());
            normals.push(normal.to_array()); 
            uvs.push(percent.to_array());
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
}
