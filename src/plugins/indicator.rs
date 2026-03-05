use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use crate::plugins::camera::CameraState;
use crate::plugins::player::Player;
use crate::plugins::planet::PlanetSettings;
use crate::GameState;

pub struct IndicatorPlugin;

impl Plugin for IndicatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), setup_indicator)
           .add_systems(Update, update_indicator_position.run_if(in_state(GameState::Playing)))
           .add_systems(Update, toggle_indicator_visibility.run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
struct PlayerIndicator;

fn create_arrow_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    
    // Arrow shaft (cylinder) - scaled up for visibility
    let shaft_height = 1500.0;
    let shaft_radius = 50.0;
    let segments = 16;
    
    // Arrow head (cone) - scaled up
    let head_height = 600.0;
    let head_radius = 200.0;
    let head_segments = 16;
    
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    
    // Create shaft (cylinder)
    let shaft_start_y = -shaft_height / 2.0;
    let shaft_end_y = shaft_height / 2.0;
    
    // Bottom circle
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * shaft_radius;
        let z = angle.sin() * shaft_radius;
        positions.push([x, shaft_start_y, z]);
        normals.push([x / shaft_radius, 0.0, z / shaft_radius]);
        uvs.push([i as f32 / segments as f32, 0.0]);
    }
    
    // Top circle
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * shaft_radius;
        let z = angle.sin() * shaft_radius;
        positions.push([x, shaft_end_y, z]);
        normals.push([x / shaft_radius, 0.0, z / shaft_radius]);
        uvs.push([i as f32 / segments as f32, 1.0]);
    }
    
    // Shaft side faces
    for i in 0..segments {
        let base = i;
        let next = i + 1;
        indices.extend_from_slice(&[
            base, next, base + segments + 1,
            next, next + segments + 1, base + segments + 1,
        ]);
    }
    
    let base_index = positions.len() as u32;
    
    // Create arrow head (cone pointing down)
    let head_top_y = shaft_end_y;
    let head_bottom_y = head_top_y + head_height;
    
    // Top point (tip of arrow)
    positions.push([0.0, head_bottom_y, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 1.0]);
    
    // Base circle of cone
    for i in 0..=head_segments {
        let angle = (i as f32 / head_segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * head_radius;
        let z = angle.sin() * head_radius;
        positions.push([x, head_top_y, z]);
        let normal = Vec3::new(x, head_height, z).normalize();
        normals.push([normal.x, normal.y, normal.z]);
        uvs.push([i as f32 / head_segments as f32, 0.0]);
    }
    
    // Cone faces
    let tip_index = base_index;
    for i in 0..head_segments {
        let base = base_index + 1 + i;
        let next = base_index + 1 + ((i + 1) % (head_segments + 1));
        indices.extend_from_slice(&[tip_index, base, next]);
    }
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    
    mesh
}

fn setup_indicator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create arrow mesh
    let arrow_mesh = meshes.add(create_arrow_mesh());
    
    let white_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0), // White
        emissive: LinearRgba::new(1.0, 1.0, 1.0, 1.0) * 5.0, // Bright glow
        unlit: true, // Make it unlit so it's always visible
        ..default()
    });
    
    // Spawn arrow
    commands.spawn((
        Mesh3d(arrow_mesh),
        MeshMaterial3d(white_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Hidden, // Hidden by default, shown in Orbital view
        PlayerIndicator,
    ));
}

fn update_indicator_position(
    mut queries: ParamSet<(
        Query<&Transform, With<Player>>,
        Query<&mut Transform, With<PlayerIndicator>>,
    )>,
    planet_settings: Res<PlanetSettings>,
) {
    // Get player position first
    let player_pos = {
        let player_query = queries.p0();
        let Ok(player_transform) = player_query.single() else { return };
        player_transform.translation
    };
    
    // Then update indicator position
    {
        let mut indicator_query = queries.p1();
        let Ok(mut indicator_transform) = indicator_query.single_mut() else { return };
        
        let direction = player_pos.normalize();
        // Position arrow so its tip is above the surface
        // Arrow extends: shaft (1500) + head (600) = 2100 units total
        // We want the tip to be at radius + offset, so center must be at radius + offset + arrow_length
        let arrow_length = 1500.0 + 600.0; // Total length from center to tip
        let tip_offset = 500.0; // How far above surface the tip should be
        let arrow_position = direction * (planet_settings.radius + tip_offset + arrow_length);
        indicator_transform.translation = arrow_position;
        
        // Point arrow downward toward planet center
        // The arrow mesh has its tip pointing in +Y direction in local space
        // We want it to point toward the planet center (Vec3::ZERO)
        let planet_center = Vec3::ZERO;
        
        // Use look_at which makes -Z point at target, then rotate to make +Y point at target
        // Rotate 90 degrees around X axis: +Y -> -Z
        let look_at_rotation = Transform::from_translation(arrow_position)
            .looking_at(planet_center, Vec3::Y)
            .rotation;
        
        // Rotate 90 degrees around local X to convert from -Z pointing to +Y pointing
        let x_rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        indicator_transform.rotation = look_at_rotation * x_rotation;
    }
}

fn toggle_indicator_visibility(
    mut indicator_query: Query<&mut Visibility, With<PlayerIndicator>>,
    camera_state: Res<CameraState>,
) {
    if let Ok(mut visibility) = indicator_query.single_mut() {
        // Only show in Orbital view
        if *camera_state == CameraState::Orbital {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

