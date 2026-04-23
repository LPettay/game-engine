use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::plugins::camera::{CameraState, OrbitalCamera};
use crate::plugins::chunked_terrain::ChunkManager;
use crate::plugins::planet::PlanetSettings;
use crate::plugins::simulation::{SimulationConfig, SimulationMode};
use crate::plugins::terrain::TerrainNoise;
use crate::plugins::ui::DebugUiVisible;
use crate::GameState;

pub struct AgentEyesPlugin;

impl Plugin for AgentEyesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AgentEyesConfig>()
            .add_systems(Startup, setup_capture_directory)
            .add_systems(
                Update,
                (auto_capture_system, manual_capture_system, command_poll_system)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Resource)]
pub struct AgentEyesConfig {
    pub capture_dir: PathBuf,
    pub interval_secs: f32,
    pub frame_counter: u32,
    pub enabled: bool,
    timer: f32,
}

impl Default for AgentEyesConfig {
    fn default() -> Self {
        Self {
            capture_dir: PathBuf::from("captures"),
            interval_secs: 3.0,
            frame_counter: 0,
            enabled: true,
            timer: 0.0,
        }
    }
}

#[derive(Serialize)]
struct FrameSidecar {
    frame_number: u32,
    timestamp_secs: f32,
    camera_position: [f32; 3],
    camera_rotation: [f32; 4],
    camera_mode: String,
    altitude_m: f32,
    distance_from_center: f32,
    fps: f32,
    active_chunks: usize,
    quadtree_max_depth: u8,
    quadtree_total_nodes: usize,
    paused: bool,
    time_compression: f64,
}

fn setup_capture_directory(config: Res<AgentEyesConfig>) {
    if let Err(e) = fs::create_dir_all(&config.capture_dir) {
        warn!("AgentEyes: failed to create capture dir: {e}");
    } else {
        info!(
            "AgentEyes: capture directory ready at {:?}",
            config.capture_dir
        );
    }
}

fn auto_capture_system(
    mut commands: Commands,
    mut config: ResMut<AgentEyesConfig>,
    time: Res<Time>,
    camera_q: Query<&Transform, With<Camera3d>>,
    camera_state: Option<Res<CameraState>>,
    terrain_noise: Option<Res<TerrainNoise>>,
    planet_settings: Option<Res<PlanetSettings>>,
    chunk_manager: Option<Res<ChunkManager>>,
    sim_config: Res<SimulationConfig>,
) {
    if !config.enabled {
        return;
    }

    config.timer += time.delta_secs();
    if config.timer < config.interval_secs {
        return;
    }
    config.timer = 0.0;

    trigger_capture(
        &mut commands,
        &mut config,
        &time,
        &camera_q,
        camera_state.as_deref(),
        terrain_noise.as_deref(),
        planet_settings.as_deref(),
        chunk_manager.as_deref(),
        &sim_config,
    );
}

fn manual_capture_system(
    mut commands: Commands,
    mut config: ResMut<AgentEyesConfig>,
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    camera_q: Query<&Transform, With<Camera3d>>,
    camera_state: Option<Res<CameraState>>,
    terrain_noise: Option<Res<TerrainNoise>>,
    planet_settings: Option<Res<PlanetSettings>>,
    chunk_manager: Option<Res<ChunkManager>>,
    sim_config: Res<SimulationConfig>,
) {
    if !input.just_pressed(KeyCode::F12) {
        return;
    }

    info!("AgentEyes: manual capture triggered (F12)");
    trigger_capture(
        &mut commands,
        &mut config,
        &time,
        &camera_q,
        camera_state.as_deref(),
        terrain_noise.as_deref(),
        planet_settings.as_deref(),
        chunk_manager.as_deref(),
        &sim_config,
    );
}

fn trigger_capture(
    commands: &mut Commands,
    config: &mut AgentEyesConfig,
    time: &Time,
    camera_q: &Query<&Transform, With<Camera3d>>,
    camera_state: Option<&CameraState>,
    terrain_noise: Option<&TerrainNoise>,
    planet_settings: Option<&PlanetSettings>,
    chunk_manager: Option<&ChunkManager>,
    sim_config: &SimulationConfig,
) {
    let frame = config.frame_counter;
    let png_path = config.capture_dir.join(format!("frame_{frame:04}.png"));
    let json_path = config.capture_dir.join(format!("frame_{frame:04}.json"));

    // Trigger async screenshot — no frame stall
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(png_path));

    // Gather camera telemetry
    let (cam_pos, cam_rot, distance) = camera_q
        .iter()
        .next()
        .map(|t| {
            let p = t.translation;
            let r = t.rotation;
            ([p.x, p.y, p.z], [r.x, r.y, r.z, r.w], p.length())
        })
        .unwrap_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0], 0.0));

    let cam_mode = camera_state
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|| "Unknown".into());

    // Altitude above terrain (same formula as telemetry.rs)
    let altitude = if let (Some(tn), Some(ps)) = (terrain_noise, planet_settings) {
        let dir = Vec3::from_array(cam_pos).normalize();
        let elevation = tn.get_elevation(dir);
        let terrain_radius = ps.radius * (1.0 + elevation * 0.1);
        distance - terrain_radius
    } else {
        distance
    };

    // Chunk/quadtree stats
    let (active_chunks, qt_max_depth, qt_total_nodes) = chunk_manager
        .map(|cm| {
            (
                cm.quadtree_chunks.len(),
                cm.quadtree.max_depth,
                cm.quadtree.nodes.len(),
            )
        })
        .unwrap_or((0, 0, 0));

    let fps = if time.delta_secs() > 0.0 {
        1.0 / time.delta_secs()
    } else {
        0.0
    };

    let time_compression = match &sim_config.mode {
        SimulationMode::Runtime { time_compression } => *time_compression,
        SimulationMode::Creative => 1.0,
    };

    let sidecar = FrameSidecar {
        frame_number: frame,
        timestamp_secs: time.elapsed_secs(),
        camera_position: cam_pos,
        camera_rotation: cam_rot,
        camera_mode: cam_mode,
        altitude_m: altitude,
        distance_from_center: distance,
        fps,
        active_chunks,
        quadtree_max_depth: qt_max_depth,
        quadtree_total_nodes: qt_total_nodes,
        paused: sim_config.paused,
        time_compression,
    };

    // Write JSON sidecar (synchronous, <500 bytes, negligible cost)
    match serde_json::to_string_pretty(&sidecar) {
        Ok(json) => {
            if let Err(e) = fs::write(&json_path, json) {
                warn!("AgentEyes: failed to write sidecar: {e}");
            }
        }
        Err(e) => warn!("AgentEyes: failed to serialize sidecar: {e}"),
    }

    info!("AgentEyes: captured frame_{frame:04}");
    config.frame_counter += 1;
}

// --- Agent Command Injection ---

#[derive(Deserialize)]
struct AgentCommand {
    #[serde(default)]
    set_distance: Option<f32>,
    #[serde(default)]
    set_yaw: Option<f32>,
    #[serde(default)]
    set_pitch: Option<f32>,
    #[serde(default)]
    set_mode: Option<String>,
    #[serde(default)]
    capture_now: Option<bool>,
    #[serde(default)]
    set_interval: Option<f32>,
    #[serde(default)]
    set_paused: Option<bool>,
    #[serde(default)]
    set_time_compression: Option<f64>,
    #[serde(default)]
    toggle_debug_ui: Option<bool>,
}

fn command_poll_system(
    mut commands: Commands,
    mut config: ResMut<AgentEyesConfig>,
    time: Res<Time>,
    mut camera_q: Query<&mut OrbitalCamera>,
    camera_transform_q: Query<&Transform, With<Camera3d>>,
    mut camera_state: ResMut<CameraState>,
    terrain_noise: Option<Res<TerrainNoise>>,
    planet_settings: Option<Res<PlanetSettings>>,
    chunk_manager: Option<Res<ChunkManager>>,
    mut sim_config: ResMut<SimulationConfig>,
    mut debug_ui: ResMut<DebugUiVisible>,
) {
    let cmd_path = config.capture_dir.join("command.json");
    let contents = match fs::read_to_string(&cmd_path) {
        Ok(c) => c,
        Err(_) => return, // No command file — normal case
    };

    // Delete immediately to avoid re-processing
    let _ = fs::remove_file(&cmd_path);

    let cmd: AgentCommand = match serde_json::from_str(&contents) {
        Ok(c) => c,
        Err(e) => {
            warn!("AgentEyes: invalid command.json: {e}");
            return;
        }
    };

    info!("AgentEyes: processing command");

    let planet_radius = planet_settings
        .as_ref()
        .map(|ps| ps.radius)
        .unwrap_or(6371.0);

    // Apply camera adjustments
    if let Ok(mut camera) = camera_q.single_mut() {
        if let Some(distance) = cmd.set_distance {
            camera.distance = distance.clamp(planet_radius + 50.0, 150000.0);
            info!("AgentEyes: set distance = {}", camera.distance);
        }
        if let Some(yaw) = cmd.set_yaw {
            camera.yaw = yaw;
            info!("AgentEyes: set yaw = {yaw}");
        }
        if let Some(pitch) = cmd.set_pitch {
            camera.pitch = pitch.clamp(-1.5, 1.5);
            info!("AgentEyes: set pitch = {}", camera.pitch);
        }
    }

    // Apply mode change
    if let Some(ref mode) = cmd.set_mode {
        match mode.as_str() {
            "Orbital" => *camera_state = CameraState::Orbital,
            "Surface" => *camera_state = CameraState::Surface,
            "ThirdPerson" => *camera_state = CameraState::ThirdPerson,
            "FreeCam" => *camera_state = CameraState::FreeCam,
            _ => warn!("AgentEyes: unknown camera mode '{mode}'"),
        }
        info!("AgentEyes: set mode = {mode}");
    }

    // Apply interval change
    if let Some(interval) = cmd.set_interval {
        config.interval_secs = interval.max(0.5); // Minimum 0.5s
        info!("AgentEyes: set interval = {}s", config.interval_secs);
    }

    // Apply simulation pause
    if let Some(paused) = cmd.set_paused {
        sim_config.paused = paused;
        info!("AgentEyes: set paused = {paused}");
    }

    // Toggle debug UI
    if let Some(visible) = cmd.toggle_debug_ui {
        debug_ui.0 = visible;
        info!("AgentEyes: set debug_ui visible = {visible}");
    }

    // Apply time compression
    if let Some(tc) = cmd.set_time_compression {
        let clamped = tc.clamp(1.0, 168.0);
        sim_config.mode = SimulationMode::Runtime { time_compression: clamped };
        info!("AgentEyes: set time_compression = {clamped}");
    }

    // Trigger immediate capture
    if cmd.capture_now == Some(true) {
        info!("AgentEyes: immediate capture triggered by command");
        trigger_capture(
            &mut commands,
            &mut config,
            &time,
            &camera_transform_q,
            Some(&*camera_state),
            terrain_noise.as_deref(),
            planet_settings.as_deref(),
            chunk_manager.as_deref(),
            &sim_config,
        );
    }
}
