use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_rapier3d::prelude::*;
use crate::plugins::planet::{PlanetSettings, Planet};
use crate::plugins::camera::CameraState;
use crate::plugins::terrain::TerrainNoise;
use crate::plugins::discovery::PlayerKnowledge;
use crate::plugins::observation::{PlayerId, ObservationFocus, ObservationJournal};
use crate::GameState;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), setup_player)
           .configure_sets(Update, PhysicsSet::Forces)
           .add_systems(Update, (
               player_alignment_system.run_if(in_state(GameState::Playing)),
               player_gravity_system.in_set(PhysicsSet::Forces).run_if(in_state(GameState::Playing)),
               player_movement_system.in_set(PhysicsSet::Forces).after(player_gravity_system).run_if(in_state(GameState::Playing)),
               player_teleport_system.run_if(in_state(GameState::Playing)),
               player_surface_adhesion_system.run_if(in_state(GameState::Playing))
            ));
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub(crate) struct TeleportImmunity {
    timer: Timer,
}

fn setup_player(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // Spawn Player — Bevy 0.18: PbrBundle removed, use individual components
    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(0.5, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.2, 0.3))),
        Transform::from_xyz(0.0, 20005.0, 0.0),
        Player,
        PlayerId(0),
        ObservationFocus::default(),
        ObservationJournal::default(),
        PlayerKnowledge::default(),
        RigidBody::Dynamic,
        Collider::capsule_y(0.5, 0.5),
        LockedAxes::ROTATION_LOCKED,
        ExternalImpulse::default(),
        Velocity::default(),
        Ccd::enabled(),
    )).insert((
        Restitution::coefficient(0.0),
        Friction::coefficient(1.0),
        Damping { linear_damping: 0.0, angular_damping: 0.5 },
    ));
}

pub fn player_gravity_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Velocity, &Transform, Option<&mut TeleportImmunity>), With<Player>>,
    planet_settings: Res<PlanetSettings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (entity, mut velocity, transform, teleport_immunity) in query.iter_mut() {
        let planet_center = Vec3::ZERO;
        let distance_vec = planet_center - transform.translation;
        let distance = distance_vec.length();
        let direction = distance_vec.normalize();

        // Newtonian Gravity: g = g_surface * (R / r)^2
        let g_surface = planet_settings.gravity * 5.0;
        let radius = planet_settings.radius;

        let gravity_strength = if distance > 0.1 {
            g_surface * (radius / distance).powi(2)
        } else {
            0.0
        };

        // Handle teleport immunity - completely disable gravity during immunity period
        if let Some(mut immunity) = teleport_immunity {
            immunity.timer.tick(time.delta());
            if immunity.timer.just_finished() {
                commands.entity(entity).remove::<TeleportImmunity>();
            } else {
                // During immunity, don't apply gravity at all
                // The player should be on the surface and collision should keep them there
                continue;
            }
        }

        // Apply normal gravity
        velocity.linvel += direction * gravity_strength * dt;
    }
}


fn player_alignment_system(
    mut query: Query<&mut Transform, With<Player>>,
    camera_state: Res<CameraState>,
) {
    // Only align automatically in Surface/Orbital (Overview) modes.
    // In ThirdPerson (Ship Control), the player controls rotation manually.
    if *camera_state == CameraState::ThirdPerson {
        return;
    }

    for mut transform in query.iter_mut() {
        let up = transform.translation.normalize();
        // Bevy 0.18: transform.up() returns Dir3 (not &Dir3), no deref needed
        let current_up = transform.up();

        // Rotate to align with new up vector
        let rotation_alignment = Quat::from_rotation_arc(current_up.into(), up);
        transform.rotation = rotation_alignment * transform.rotation;
    }
}

fn player_movement_system(
    mut query: Query<(&mut ExternalImpulse, &mut Transform), With<Player>>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<Player>)>,
    camera_state: Res<CameraState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    // Bevy 0.16+: single_mut() returns Result
    let Ok((mut impulse, mut transform)) = query.single_mut() else { return };
    let dt = time.delta_secs();

    // Tuned thrust for new impulse system
    // Force = 65.0. Impulse = Force * dt
    let thrust_force = 65.0;
    let mut move_dir = Vec3::ZERO;

    match *camera_state {
        CameraState::Surface => {
            if let Ok(camera_transform) = camera_query.single() {
                // Bevy 0.18: forward()/right() return Dir3 (not &Dir3), no deref needed
                let forward = camera_transform.forward();
                let right = camera_transform.right();

                if keyboard_input.pressed(KeyCode::KeyW) {
                    move_dir += *forward;
                }
                if keyboard_input.pressed(KeyCode::KeyS) {
                    move_dir -= *forward;
                }
                if keyboard_input.pressed(KeyCode::KeyD) {
                    move_dir += *right;
                }
                if keyboard_input.pressed(KeyCode::KeyA) {
                    move_dir -= *right;
                }
            }
        }
        CameraState::FreeCam => {
            // Player doesn't move in FreeCam mode - camera moves independently
        }
        CameraState::ThirdPerson => {
            // Rocket Controls
            // Space: Thrust (Forward / Up relative to ship)
            if keyboard_input.pressed(KeyCode::Space) {
                 // In our model, the capsule is upright (Y-axis), so "Up" is local Y.
                 // Wait, normally rockets fly "forward" (Z or Y).
                 // Capsule3d is Y-aligned.
                 // Let's assume "Up" (Local Y) is the main thruster.
                 // Bevy 0.18: transform.up() returns Dir3 (not &Dir3), no deref needed
                 move_dir += *transform.up();
            }

            // Torque / Rotation
            let rotation_speed = 2.0 * time.delta_secs();

            // Pitch (W/S) - Rotate around Local X
            if keyboard_input.pressed(KeyCode::KeyW) {
                transform.rotate_local_x(rotation_speed);
            }
            if keyboard_input.pressed(KeyCode::KeyS) {
                transform.rotate_local_x(-rotation_speed);
            }

            // Yaw (A/D) - Rotate around Local Y (Wait, rockets usually Yaw around Z or Y?)
            // If Capsule is Y-up, Yaw is usually Y-axis rotation.
            // BUT, if we are flying "plane style", A/D might be Roll?
            // User asked: "WASD will allow pitching of the vessel"
            // "A" and "D" usually mean Yaw or Roll depending on mode.
            // Let's map A/D to YAW (Local Y) and Q/E to ROLL (Local Z)?
            // Actually, usually:
            // W/S = Pitch
            // A/D = Yaw
            // Q/E = Roll

            if keyboard_input.pressed(KeyCode::KeyA) {
                // Turn Left
                transform.rotate_local_y(rotation_speed);
            }
            if keyboard_input.pressed(KeyCode::KeyD) {
                // Turn Right
                transform.rotate_local_y(-rotation_speed);
            }

            // Roll (Q/E) - Rotate around Local Z (Forward?)
            // If Capsule is Y-up, Forward is -Z.
            // So Roll is around Y? No, Roll is around "Forward" axis.
            // If we fly "Up" (Y), then Roll is around Y? No, that's Yaw.
            // Let's clarify orientation:
            // Capsule stands on Y.
            // "Forward" for movement is Y (Thrust).
            // So "Roll" is rotation around Y.
            // "Pitch" is rotation around X.
            // "Yaw" is rotation around Z.

            // Wait, if "Up" is thrust, then:
            // Roll = Rotate around Y.
            // Pitch = Rotate around X.
            // Yaw = Rotate around Z.

            // Let's remap based on standard rocket controls where "Up" is the nose.

            // Q/E = Roll (Around Y)
            if keyboard_input.pressed(KeyCode::KeyQ) {
                 transform.rotate_local_y(rotation_speed);
            }
            if keyboard_input.pressed(KeyCode::KeyE) {
                 transform.rotate_local_y(-rotation_speed);
            }

        }
        CameraState::Orbital => {}
    }

    if move_dir != Vec3::ZERO {
        impulse.impulse += move_dir.normalize() * thrust_force * dt;
    }
}

fn player_teleport_system(
    mut commands: Commands,
    mut player_query: Query<(Entity, &mut Transform, &mut Velocity), With<Player>>,
    camera_query: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<Player>)>,
    mut planet_queries: ParamSet<(
        Query<Entity, With<Planet>>,
        Query<&GlobalTransform, With<Planet>>,
    )>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_state: Res<CameraState>,
    planet_settings: Res<PlanetSettings>,
    rapier_context: ReadRapierContext,
    terrain_noise: Option<Res<TerrainNoise>>,
) {
    // Only allow teleporting in Orbital view
    if *camera_state != CameraState::Orbital {
        return;
    }

    // Check for left mouse button click
    if !mouse_button_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok((player_entity, mut player_transform, mut player_velocity)) = player_query.single_mut() else { return };
    let Ok((camera_transform, camera)) = camera_query.single() else { return };
    let Ok(window) = window_query.single() else { return };

    // Get planet entity first
    let planet_query_0 = planet_queries.p0();
    let Ok(planet_entity) = planet_query_0.single() else { return };

    // Then get planet transform
    let planet_query_1 = planet_queries.p1();
    let Ok(planet_transform) = planet_query_1.single() else { return };

    // Get mouse position in window coordinates
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Convert screen coordinates to world ray
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else { return };

    // Perform raycast against planet
    let ray_origin = ray.origin;
    let ray_dir = *ray.direction;
    let max_toi = 200000.0; // Max distance

    let filter = QueryFilter::default()
        .exclude_collider(player_entity)
        .exclude_rigid_body(player_entity);

    let Ok(rapier_ctx) = rapier_context.single() else { return };
    if let Some((entity, toi)) = rapier_ctx.cast_ray(
        ray_origin,
        ray_dir,
        max_toi,
        true,
        filter,
    ) {
        if entity == planet_entity {
            // Calculate teleport position on planet surface
            let hit_point = ray_origin + ray_dir * toi;

            // CRITICAL: Transform hit point to planet's local space before calculating terrain
            // The planet may have rotated, so we need to undo that rotation to get the original
            // terrain coordinates that match the noise generation
            // Transform hit point to planet's local space (undo planet rotation)
            // The planet is at origin, so we just need to undo the rotation
            // Extract rotation from GlobalTransform
            let (_scale, rotation, _translation) = planet_transform.to_scale_rotation_translation();
            let inverse_rotation = rotation.inverse();
            let local_hit_point = inverse_rotation * hit_point;
            let direction_from_center_local = local_hit_point.normalize();

            // Use local-space direction for terrain calculation
            let direction_from_center = direction_from_center_local;

            // Get actual terrain elevation at this location using planet-local coordinates
            // The terrain uses a unit sphere point in the planet's original orientation
            let unit_sphere_point = direction_from_center;
            let Some(ref terrain_noise) = terrain_noise else { return; };
            let elevation_factor = terrain_noise.get_elevation(unit_sphere_point);

            // Apply elevation: height_mult = 1.0 + elevation_factor * 0.15 (matches planet generation)
            let height_mult = 1.0 + elevation_factor * 0.15;
            let actual_terrain_radius = planet_settings.radius * height_mult;

            // Position player on terrain surface (capsule bottom touches terrain)
            // Capsule3d::new(0.5, 1.0) means radius=0.5, length=1.0
            // Total height = length + 2*radius = 2.0m
            // Center to bottom = length/2 + radius = 0.5 + 0.5 = 1.0m
            // To place bottom of capsule on terrain surface, center must be at terrain_radius + 1.0
            // Add a small buffer to prevent clipping through terrain
            let player_capsule_center_to_bottom = 1.0; // Distance from capsule center to bottom
            let teleport_height = player_capsule_center_to_bottom + 0.2; // Slightly ABOVE terrain to prevent clipping

            // Calculate position in planet's local space, then transform back to world space
            let local_teleport_position = direction_from_center_local * (actual_terrain_radius + teleport_height);
            let (_scale2, rotation2, _translation2) = planet_transform.to_scale_rotation_translation();
            let teleport_position = rotation2 * local_teleport_position;

            // Teleport player - update Bevy transform, Rapier will sync automatically
            player_transform.translation = teleport_position;

            // Reset velocity to prevent weird physics
            player_velocity.linvel = Vec3::ZERO;
            player_velocity.angvel = Vec3::ZERO;

            // Add teleport immunity - shorter duration since we're placing directly on surface
            // This gives the physics system time to detect collision with terrain
            commands.entity(player_entity).insert(TeleportImmunity {
                timer: Timer::from_seconds(0.2, TimerMode::Once), // Short immunity just to stabilize
            });
        }
    }
}

// System to keep player on terrain surface using collision detection
// This prevents falling through the mesh collider, especially after planet rotation
fn player_surface_adhesion_system(
    mut player_query: Query<(Entity, &mut Transform, &mut Velocity), With<Player>>,
    planet_query: Query<Entity, With<Planet>>,
    planet_settings: Res<PlanetSettings>,
    rapier_context: ReadRapierContext,
    terrain_noise: Option<Res<TerrainNoise>>,
) {
    let Ok((player_entity, mut player_transform, mut player_velocity)) = player_query.single_mut() else { return };
    let Ok(planet_entity) = planet_query.single() else { return };

    let player_pos = player_transform.translation;
    let direction_from_center = player_pos.normalize();

    // Cast a ray downward from player to check distance to terrain surface
    let ray_origin = player_pos + direction_from_center * 2.0; // Start slightly above player
    let ray_dir = -direction_from_center; // Cast toward planet center
    let max_distance = 5.0; // Check up to 5m below player

    let filter = QueryFilter::default()
        .exclude_collider(player_entity)
        .exclude_rigid_body(player_entity);

    let Ok(rapier_ctx) = rapier_context.single() else { return };
    if let Some((entity, toi)) = rapier_ctx.cast_ray(ray_origin, ray_dir, max_distance, true, filter) {
        if entity == planet_entity {
            // Get actual hit point on terrain surface
            let hit_point = ray_origin + ray_dir * toi;

            // Calculate expected terrain height at this location for comparison
            let Some(ref terrain_noise) = terrain_noise else { return; };
            let elevation_factor = terrain_noise.get_elevation(direction_from_center);
            let height_mult = 1.0 + elevation_factor * 0.15;
            let _expected_terrain_radius = planet_settings.radius * height_mult;

            // Calculate distance from player to actual hit point
            let player_capsule_center_to_bottom = 1.0; // Capsule3d::new(0.5, 1.0) -> length/2 + radius = 1.0
            let distance_to_hit = (player_pos - hit_point).length();
            let expected_distance = player_capsule_center_to_bottom; // Should be ~1m (center to bottom)

            // If player is significantly further from surface than expected, they're falling through
            // Push them back to the correct position
            if distance_to_hit > expected_distance + 0.5 {
                let correction_distance = distance_to_hit - expected_distance;
                let correction = direction_from_center * correction_distance;
                player_transform.translation -= correction;

                // Stop downward velocity completely
                let downward_vel = player_velocity.linvel.dot(-direction_from_center);
                if downward_vel > 0.0 {
                    player_velocity.linvel += direction_from_center * downward_vel;
                }
            }
        }
    }
}

// Add explicit System Set ordering for physics updates
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum PhysicsSet {
    Forces,
}
