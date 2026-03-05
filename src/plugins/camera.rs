use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraState>()
           .add_systems(Startup, setup_camera)
           .add_systems(Update, camera_switching_system)
           .add_systems(PostUpdate, camera_control_system);
    }
}

#[derive(Resource, Default, PartialEq, Eq, Debug)]
pub enum CameraState {
    #[default]
    Orbital,
    Surface, // First Person
    ThirdPerson,
    FreeCam, // Floating freecam on planet surface
}

#[derive(Component)]
struct OrbitalCamera {
    distance: f32,
    pitch: f32,
    yaw: f32,
    // Added for 3rd person zoom
    third_person_distance: f32,
    // FreeCam position (independent of player)
    freecam_position: Option<Vec3>,
}

fn setup_camera(mut commands: Commands) {
    // Orbital camera position
    // Bevy 0.15+: Camera3dBundle removed, spawn Camera3d component directly
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 30000.0).looking_at(Vec3::ZERO, Vec3::Y),
        Projection::Perspective(PerspectiveProjection {
            far: 200000.0, // Significantly increased render distance
            ..default()
        }),
        OrbitalCamera {
            distance: 30000.0,
            pitch: 0.0,
            yaw: 0.0,
            third_person_distance: 20.0,
            freecam_position: None,
        },
    ));
}

use crate::plugins::player::Player;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

fn camera_switching_system(
    mut state: ResMut<CameraState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    // Bevy 0.17+: CursorOptions is a separate component, not part of Window
    mut cursor_options: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut camera_query: Query<&mut OrbitalCamera>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyC) {
        let old_state = std::mem::replace(&mut *state, CameraState::Orbital);
        *state = match old_state {
            CameraState::Orbital => CameraState::Surface,
            CameraState::Surface => CameraState::ThirdPerson,
            CameraState::ThirdPerson => CameraState::FreeCam,
            CameraState::FreeCam => CameraState::Orbital,
        };
        println!("Camera State: {:?}", *state);

        // Reset camera angles and clear stale state on transition
        if let Ok(mut camera) = camera_query.single_mut() {
            // Clear freecam position when leaving FreeCam
            if old_state == CameraState::FreeCam {
                camera.freecam_position = None;
            }

            // Reset yaw/pitch for new mode
            match *state {
                CameraState::Surface => {
                    camera.yaw = 0.0;
                    camera.pitch = 0.0;
                }
                CameraState::ThirdPerson => {
                    camera.yaw = 0.0;
                    camera.pitch = -0.3; // Chase-cam angle
                }
                CameraState::Orbital | CameraState::FreeCam => {
                    camera.yaw = 0.0;
                    camera.pitch = 0.0;
                }
            }
        }

        match *state {
            CameraState::Orbital => {
                cursor_options.grab_mode = CursorGrabMode::None;
                cursor_options.visible = true;
            }
            CameraState::Surface | CameraState::ThirdPerson | CameraState::FreeCam => {
                cursor_options.grab_mode = CursorGrabMode::Confined;
                cursor_options.visible = false;
            }
        }
    }
}

fn camera_control_system(
    state: Res<CameraState>,
    // Bevy 0.18: Use Single<> for guaranteed-single-entity queries
    mut camera: Single<(&mut Transform, &mut OrbitalCamera), Without<Player>>,
    player_query: Query<&Transform, With<Player>>,
    mut mouse_motion_events: MessageReader<MouseMotion>,
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    // Bevy 0.17+: CursorOptions is a separate component
    mut cursor_options: Single<&mut CursorOptions, With<PrimaryWindow>>,
    time: Res<Time>,
    planet_settings: Res<crate::plugins::planet::PlanetSettings>,
) {
    let (ref mut camera_transform, ref mut camera) = *camera;
    let planet_radius_approx = planet_settings.radius;

    // Toggle Cursor Lock
    if keyboard_input.just_pressed(KeyCode::AltLeft) {
        match cursor_options.grab_mode {
            CursorGrabMode::None => {
                 // Lock it back if we are in Surface/ThirdPerson
                 if *state != CameraState::Orbital {
                     cursor_options.grab_mode = CursorGrabMode::Confined;
                     cursor_options.visible = false;
                 }
            }
            _ => {
                // Unlock
                cursor_options.grab_mode = CursorGrabMode::None;
                cursor_options.visible = true;
            }
        }
    }

    let cursor_locked = cursor_options.grab_mode != CursorGrabMode::None;

    match *state {
        CameraState::Orbital => {
            // Zoom
            for event in mouse_wheel_events.read() {
                camera.distance -= event.y * 2000.0; // Increased zoom speed for larger scale
                camera.distance = camera.distance.clamp(planet_radius_approx + 50.0, 150000.0);
            }

            // Rotate
            if mouse_button_input.pressed(MouseButton::Right) {
                for event in mouse_motion_events.read() {
                    camera.yaw -= event.delta.x * 0.005;
                    camera.pitch -= event.delta.y * 0.005;
                    camera.pitch = camera.pitch.clamp(-1.5, 1.5);
                }
            }

            // Update Transform
            let rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
            camera_transform.translation = rotation * Vec3::Z * camera.distance;
            camera_transform.look_at(Vec3::ZERO, Vec3::Y);
        }
        CameraState::Surface => {
            // Bevy 0.16+: Query::single() returns Result
            if let Ok(player_transform) = player_query.single() {
                // Mouse Look (Only if locked)
                if cursor_locked {
                    for event in mouse_motion_events.read() {
                        camera.yaw -= event.delta.x * 0.002;
                        camera.pitch -= event.delta.y * 0.002;
                        camera.pitch = camera.pitch.clamp(-1.5, 1.5);
                    }
                }

                // Calculate Camera Position (Head)
                let up = player_transform.translation.normalize();
                let head_offset = up * 0.8; // Eye level
                let position = player_transform.translation + head_offset;

                // Calculate Rotation
                // Use the player's rotation as the base frame to avoid singularities near poles
                // The player is already aligned to the surface normal by player_alignment_system
                let local_rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
                let global_rotation = player_transform.rotation * local_rotation;

                camera_transform.translation = position;
                camera_transform.rotation = global_rotation;
            }
        }
        CameraState::ThirdPerson => {
            if let Ok(player_transform) = player_query.single() {
                 // Zoom
                 for event in mouse_wheel_events.read() {
                    camera.third_person_distance -= event.y * 2.0;
                    camera.third_person_distance = camera.third_person_distance.clamp(5.0, 100.0);
                }

                // Mouse Look (Only if locked)
                if cursor_locked {
                    for event in mouse_motion_events.read() {
                        camera.yaw -= event.delta.x * 0.002;
                        camera.pitch -= event.delta.y * 0.002;
                        camera.pitch = camera.pitch.clamp(-1.5, 1.5);
                    }
                }

                // Calculate Camera Position
                // 3rd person should be "behind" the direction the player is facing if we wanted a chase cam,
                // OR free orbit around the player?
                // The request says "see my character from 3rd person", usually implies orbit around char OR chase.
                // "zoom in and out" -> likely orbit around player.

                // We'll use the camera's yaw/pitch relative to the PLAYER'S orientation frame
                let local_rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);

                // Align with gravity/player up
                let up = player_transform.translation.normalize();
                let player_rotation = player_transform.rotation;
                // Actually, for 3rd person, we usually want to rotate AROUND the player.
                // So we calculate an offset based on yaw/pitch.

                // Let's make it relative to the player's local frame so it follows them
                let offset = local_rotation * Vec3::Z * camera.third_person_distance;
                let _final_rotation = player_rotation * local_rotation;
                let position = player_transform.translation + (player_rotation * offset);

                camera_transform.translation = position;
                camera_transform.look_at(player_transform.translation, up);
            }
        }
        CameraState::FreeCam => {
            // Bevy 0.15+: delta_seconds() renamed to delta_secs()
            let dt = time.delta_secs();
            let move_speed = 50.0; // Units per second

            // Initialize freecam position from player if not set, or from current camera position
            if camera.freecam_position.is_none() {
                if let Ok(player_transform) = player_query.single() {
                    // Start at player's head position
                    let up = player_transform.translation.normalize();
                    camera.freecam_position = Some(player_transform.translation + up * 2.0);
                } else {
                    // Fallback: use current camera position
                    camera.freecam_position = Some(camera_transform.translation);
                }
            }

            let mut position = camera.freecam_position.unwrap();

            // Mouse Look (Only if cursor locked)
            if cursor_locked {
                for event in mouse_motion_events.read() {
                    camera.yaw -= event.delta.x * 0.002;
                    camera.pitch -= event.delta.y * 0.002;
                    camera.pitch = camera.pitch.clamp(-1.5, 1.5);
                }
            }

            // Calculate orientation based on planet surface
            // "Up" is the direction away from planet center
            let up = position.normalize();

            // Build a rotation frame aligned to the planet surface
            // We need a stable "forward" direction on the surface
            // Use the tangent to the sphere (perpendicular to up)
            let world_forward = if up.y.abs() > 0.99 {
                Vec3::X // Use X if we're near poles
            } else {
                Vec3::Y
            };
            let right = up.cross(world_forward).normalize();
            let forward = right.cross(up).normalize();

            // Build base rotation from surface frame
            let base_rotation = Quat::from_mat3(&Mat3::from_cols(right, up, -forward));

            // Apply local yaw/pitch rotation
            let local_rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
            let final_rotation = base_rotation * local_rotation;

            // Movement input
            let cam_forward = final_rotation * -Vec3::Z;
            let cam_right = final_rotation * Vec3::X;

            let mut move_dir = Vec3::ZERO;

            // WASD for horizontal movement (relative to camera facing, projected onto surface)
            if keyboard_input.pressed(KeyCode::KeyW) {
                move_dir += cam_forward;
            }
            if keyboard_input.pressed(KeyCode::KeyS) {
                move_dir -= cam_forward;
            }
            if keyboard_input.pressed(KeyCode::KeyD) {
                move_dir += cam_right;
            }
            if keyboard_input.pressed(KeyCode::KeyA) {
                move_dir -= cam_right;
            }

            // Space/Control for up/down movement (relative to planet surface normal)
            if keyboard_input.pressed(KeyCode::Space) {
                move_dir += up;
            }
            if keyboard_input.pressed(KeyCode::ControlLeft) || keyboard_input.pressed(KeyCode::ControlRight) {
                move_dir -= up;
            }

            // Apply movement
            if move_dir != Vec3::ZERO {
                position += move_dir.normalize() * move_speed * dt;
            }

            // Clamp minimum height above planet surface
            let min_height = planet_radius_approx + 1.0;
            let current_height = position.length();
            if current_height < min_height {
                position = position.normalize() * min_height;
            }

            // Store and apply position
            camera.freecam_position = Some(position);
            camera_transform.translation = position;
            camera_transform.rotation = final_rotation;
        }
    }
}
