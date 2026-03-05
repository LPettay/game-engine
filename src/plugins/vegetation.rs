use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use noise::NoiseFn;
use crate::plugins::planet::{Planet, PlanetSettings};
use crate::plugins::terrain::TerrainNoise;
use crate::GameState;

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_trees);
    }
}

#[derive(Component)]
pub struct Tree;

fn create_tree_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    
    // Tree trunk (cylinder) - scaled up for visibility
    let trunk_height = 8.0;
    let trunk_radius = 0.4;
    let trunk_segments = 8;
    
    // Trunk base circle
    for i in 0..=trunk_segments {
        let angle = (i as f32 / trunk_segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * trunk_radius;
        let z = angle.sin() * trunk_radius;
        positions.push([x, 0.0, z]);
        normals.push([x / trunk_radius, 0.0, z / trunk_radius]);
        uvs.push([i as f32 / trunk_segments as f32, 0.0]);
    }
    
    // Trunk top circle
    for i in 0..=trunk_segments {
        let angle = (i as f32 / trunk_segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * trunk_radius;
        let z = angle.sin() * trunk_radius;
        positions.push([x, trunk_height, z]);
        normals.push([x / trunk_radius, 0.0, z / trunk_radius]);
        uvs.push([i as f32 / trunk_segments as f32, 1.0]);
    }
    
    // Trunk side faces
    for i in 0..trunk_segments {
        let base = i;
        let next = i + 1;
        indices.extend_from_slice(&[
            base, next, base + trunk_segments + 1,
            next, next + trunk_segments + 1, base + trunk_segments + 1,
        ]);
    }
    
    let base_index = positions.len() as u32;
    
    // Tree foliage (cone/sphere hybrid) - scaled up for visibility
    let foliage_height = 6.0;
    let foliage_radius = 3.0;
    let foliage_segments = 12;
    let foliage_bottom_y = trunk_height;
    let foliage_top_y = trunk_height + foliage_height;
    
    // Foliage bottom circle
    for i in 0..=foliage_segments {
        let angle = (i as f32 / foliage_segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * foliage_radius;
        let z = angle.sin() * foliage_radius;
        positions.push([x, foliage_bottom_y, z]);
        let normal = Vec3::new(x, foliage_height * 0.5, z).normalize();
        normals.push([normal.x, normal.y, normal.z]);
        uvs.push([i as f32 / foliage_segments as f32, 0.0]);
    }
    
    // Foliage top point
    positions.push([0.0, foliage_top_y, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 1.0]);
    
    // Foliage faces
    let tip_index = base_index + foliage_segments + 1;
    for i in 0..foliage_segments {
        let base = base_index + i;
        let next = base_index + ((i + 1) % (foliage_segments + 1));
        indices.extend_from_slice(&[tip_index, base, next]);
    }
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    
    mesh
}

fn should_spawn_tree(elevation: f32, latitude: f32, moisture: f32, is_river: bool) -> bool {
    // Don't spawn trees in water
    if elevation < 0.0 {
        return false;
    }
    
    // Don't spawn trees in rivers
    if is_river {
        return false;
    }
    
    // Don't spawn trees on beaches (too low elevation)
    if elevation < 0.02 {
        return false;
    }
    
    // Don't spawn trees on high mountains (snow)
    if elevation > 0.25 {
        return false;
    }
    
    // Temperature based on latitude
    let temp = 1.0 - latitude; // 1.0 = equator, 0.0 = pole
    let m = (moisture + 1.0) * 0.5; // Normalize moisture to 0..1
    
    // Trees need moderate to high moisture and reasonable temperature
    // Forests appear when: temp > 0.2 and moisture > 0.3
    // Rainforests: temp > 0.7 and moisture > 0.5
    // Taiga: temp 0.2-0.4 and moisture > 0.3
    
    if temp < 0.2 {
        // Too cold (polar) - no trees
        return false;
    }
    
    if temp < 0.4 {
        // Boreal/Taiga - need moisture > 0.3
        return m > 0.3;
    }
    
    if temp < 0.7 {
        // Temperate - need moisture > 0.2 for forests
        return m > 0.2;
    }
    
    // Tropical - need moisture > 0.2
    m > 0.2
}

fn spawn_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    planet_query: Query<Entity, With<Planet>>,
    planet_settings: Res<PlanetSettings>,
) {
    let Ok(planet_entity) = planet_query.single() else {
        println!("WARNING: No planet entity found when spawning trees");
        return;
    };
    
    println!("Spawning trees for planet entity: {:?}", planet_entity);
    
    // Create tree mesh and materials
    let tree_mesh = meshes.add(create_tree_mesh());
    let foliage_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.5, 0.1), // Green foliage
        ..default()
    });
    
    // Sample points across planet surface to place trees
    let terrain_noise = TerrainNoise::new(planet_settings.terrain_seed);
    let samples_per_face = 30; // Reduced for performance: 30x30 = 900 points per face
    
    let mut tree_count = 0;
    
    // Sample each face of the cube sphere
    let directions = [
        Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z,
    ];
    
    for direction in directions {
        let axis_a = if direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let axis_b = direction.cross(axis_a);
        
        for y in 0..samples_per_face {
            for x in 0..samples_per_face {
                let percent = Vec2::new(x as f32, y as f32) / (samples_per_face as f32 - 1.0);
                
                // Cube sphere mapping
                let point_on_unit_cube = direction + (percent.x - 0.5) * 2.0 * axis_a + (percent.y - 0.5) * 2.0 * axis_b;
                let point_on_unit_sphere = point_on_unit_cube.normalize();
                
                // Get terrain data
                let elevation = terrain_noise.get_elevation(point_on_unit_sphere);
                
                // Get biome data for tree placement
                let p = [point_on_unit_sphere.x as f64, point_on_unit_sphere.y as f64, point_on_unit_sphere.z as f64];
                let warp_strength = 0.05;
                let q = [
                    p[0] + terrain_noise.warp_noise.get([p[0], p[1], p[2]]) * warp_strength,
                    p[1] + terrain_noise.warp_noise.get([p[0] + 5.2, p[1] + 1.3, p[2] + 2.8]) * warp_strength,
                    p[2] + terrain_noise.warp_noise.get([p[0] + 1.7, p[1] + 9.2, p[2] + 5.2]) * warp_strength,
                ];
                
                let continent_val = terrain_noise.continent_noise.get(q) as f32;
                let river_base = terrain_noise.river_noise.get(q) as f32;
                let river_val = river_base.abs();
                let is_river = river_val < 0.035 && continent_val > 0.0;
                
                let lat_noise = terrain_noise.moisture_noise.get([p[0] * 2.0, p[1] * 2.0, p[2] * 2.0]) as f32 * 0.1;
                let latitude = (point_on_unit_sphere.y.abs() + lat_noise).clamp(0.0, 1.0);
                let moisture = terrain_noise.moisture_noise.get(q) as f32;
                
                // Check if tree should spawn here
                if should_spawn_tree(elevation, latitude, moisture, is_river) {
                    // Add some randomness to avoid uniform grid
                    let noise_x = (terrain_noise.moisture_noise.get([p[0] * 10.0, p[1] * 10.0, p[2] * 10.0]) as f32 + 1.0) * 0.5;
                    let noise_y = (terrain_noise.moisture_noise.get([p[0] * 10.0 + 100.0, p[1] * 10.0 + 100.0, p[2] * 10.0 + 100.0]) as f32 + 1.0) * 0.5;
                    
                    // Only spawn tree if noise value is above threshold (creates clumps)
                    // Lowered threshold to spawn more trees
                    if noise_x > 0.5 && noise_y > 0.5 {
                        // Calculate tree position on terrain
                        let height_mult = 1.0 + elevation * 0.1;
                        let tree_position = point_on_unit_sphere * planet_settings.radius * height_mult;
                        
                        // Align tree with terrain normal (point upward from planet surface)
                        let up = tree_position.normalize();
                        // Create rotation that makes +Y point in the up direction
                        let tree_rotation = Quat::from_rotation_arc(Vec3::Y, up);
                        
                        // Spawn tree
                        commands.spawn((
                            Mesh3d(tree_mesh.clone()),
                            MeshMaterial3d(foliage_material.clone()),
                            Transform::from_translation(tree_position)
                                .with_rotation(tree_rotation)
                                .with_scale(Vec3::splat(1.0 + (noise_x - 0.5) * 0.4)), // Vary tree size
                            Tree,
                            ChildOf(planet_entity),
                        ));
                        
                        tree_count += 1;
                    }
                }
            }
        }
    }
    
    println!("Spawned {} trees", tree_count);
}

