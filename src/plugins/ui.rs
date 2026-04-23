use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crate::plugins::planet::PlanetSettings;
use crate::plugins::sun::SunSettings;
use crate::plugins::chunked_terrain::ChunkManager;
use crate::plugins::gpu_terrain::GpuTerrainSettings;
use crate::plugins::simulation::SimulationConfig;
use crate::GameState; // Added

pub struct UiPlugin;

#[derive(Resource)]
pub struct DebugUiVisible(pub bool);

impl Default for DebugUiVisible {
    fn default() -> Self {
        Self(true)
    }
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
           .init_resource::<DebugUiVisible>()
           .add_systems(Update, (debug_ui_system, loading_ui_system, journal_ui_system));
    }
}

fn loading_ui_system(
    mut contexts: EguiContexts,
    state: Res<State<GameState>>,
    mut ready: Local<bool>,
) {
    // Skip first frame — egui context exists but fonts aren't loaded until after first run()
    if !*ready { *ready = true; return; }
    if *state.get() == GameState::Loading {
        // Check if context is ready
        if let Ok(ctx) = contexts.ctx_mut() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.4);
                    ui.heading(egui::RichText::new("Generating Planet...").size(30.0).strong());
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("Calculating high-fidelity terrain mesh...").size(20.0));
                    ui.label(egui::RichText::new("Generating procedural noise textures...").size(20.0));
                    ui.add_space(20.0);
                    ui.spinner();
                });
            });
        }
    }
}

use crate::plugins::trajectory::OrbitalStats;
use crate::plugins::player::Player;
use bevy_rapier3d::prelude::Velocity;

fn debug_ui_system(
    mut contexts: EguiContexts,
    mut planet_settings: ResMut<PlanetSettings>,
    mut sun_settings: ResMut<SunSettings>,
    mut chunk_manager: ResMut<ChunkManager>,
    player_query: Query<(&Transform, &Velocity, Option<&OrbitalStats>), With<Player>>,
    state: Res<State<GameState>>,
    time: Res<Time>,
    chunk_query: Query<&crate::plugins::chunked_terrain::TerrainChunk>,
    gpu_settings: Option<Res<GpuTerrainSettings>>,
    mut sim_config: ResMut<SimulationConfig>,
    terrain_noise: Option<Res<crate::plugins::terrain::TerrainNoise>>,
    debug_visible: Res<DebugUiVisible>,
) {
    if *state.get() == GameState::Loading || !debug_visible.0 {
        return;
    }
    
    // Safely check for context
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::Window::new("Planet Debug").show(ctx, |ui| {
            ui.heading("Sun & Lighting");
            ui.add(egui::Slider::new(&mut sun_settings.illuminance, 0.0..=200000.0).text("Sun Intensity (Lux)"));
            ui.separator();
            ui.heading("Atmosphere & Physics");
        ui.add(egui::Slider::new(&mut planet_settings.gravity, 0.0..=50.0).text("Gravity"));
        ui.add(egui::Slider::new(&mut planet_settings.air_density_sea_level, 0.0..=10.0).text("Air Density (Sea Level)"));
        ui.separator();
        ui.heading("Visuals (Scattering)");
        ui.checkbox(&mut planet_settings.atmosphere_enabled, "Atmosphere Enabled");
        ui.checkbox(&mut planet_settings.soft_lighting, "Basic Soft Lighting");
        ui.add(egui::Slider::new(&mut planet_settings.atmosphere_height, 1000.0..=50000.0).text("Atmosphere Height"));
        
        ui.label("Rayleigh Scattering (Color)");
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut planet_settings.rayleigh_scattering.x, 0.0..=0.01).text("R"));
            ui.add(egui::Slider::new(&mut planet_settings.rayleigh_scattering.y, 0.0..=0.01).text("G"));
            ui.add(egui::Slider::new(&mut planet_settings.rayleigh_scattering.z, 0.0..=0.01).text("B"));
        });
        ui.add(egui::Slider::new(&mut planet_settings.rayleigh_scale_height, 1000.0..=10000.0).text("Rayleigh Scale H"));
        
        ui.separator();
        ui.label("Mie Scattering (Haze)");
        ui.add(egui::Slider::new(&mut planet_settings.mie_scattering, 0.0..=0.01).text("Coeff"));
        ui.add(egui::Slider::new(&mut planet_settings.mie_scale_height, 1000.0..=10000.0).text("Mie Scale H"));
        ui.add(egui::Slider::new(&mut planet_settings.mie_asymmetry, 0.0..=0.99).text("Asymmetry (G)"));

        ui.separator();
        ui.heading("Terrain Chunks");
        ui.add(egui::Slider::new(&mut chunk_manager.chunk_render_radius, 1.0..=10.0).text("Chunk Render Radius"));
        ui.label(format!("Renders chunks within {:.1} chunks of viewport center", chunk_manager.chunk_render_radius));
        
        // Performance metrics
        ui.separator();
        ui.heading("Performance");
        let fps = 1.0 / time.delta_secs();
        let frame_time_ms = time.delta_secs() * 1000.0;
        let loaded_chunks = chunk_query.iter().filter(|c| c.is_loaded).count();
        let generating_chunks = chunk_query.iter().filter(|c| c.is_generating).count();
        
        ui.label(format!("FPS: {:.1}", fps));
        ui.label(format!("Frame Time: {:.2} ms", frame_time_ms));
        ui.label(format!("Quadtree Chunks: {}", chunk_manager.quadtree_chunks.len()));
        ui.label(format!("Quadtree Nodes: {}", chunk_manager.quadtree.nodes.len()));
        ui.label(format!("Legacy Chunks: {}", chunk_manager.chunks.len()));
        ui.label(format!("Loaded Chunks: {}", loaded_chunks));
        ui.label(format!("Generating Chunks: {}", generating_chunks));
        ui.label(format!("Max Active Chunks: {}", chunk_manager.max_active_chunks));
        ui.label(format!("Base Resolution: {}x{}", chunk_manager.base_chunk_resolution, chunk_manager.base_chunk_resolution));
        
        // Performance warnings
        if frame_time_ms > 16.67 {
            ui.colored_label(egui::Color32::RED, format!("⚠ Frame time > 16.67ms (target: 60 FPS)"));
        }
        if chunk_manager.chunks.len() > chunk_manager.max_active_chunks {
            ui.colored_label(egui::Color32::YELLOW, format!("⚠ Exceeded max active chunks"));
        }

        let gpu_status = gpu_settings.as_ref().map_or(false, |s| s.enabled);
        ui.label(format!("GPU Terrain: {}", if gpu_status { "Enabled" } else { "Disabled (CPU fallback)" }));

        ui.separator();
        ui.heading("Simulation");
        if ui.button(if sim_config.paused { "Resume" } else { "Pause" }).clicked() {
            sim_config.paused = !sim_config.paused;
        }
        if let crate::plugins::simulation::SimulationMode::Runtime { ref mut time_compression } = sim_config.mode {
            let mut tc = *time_compression as f32;
            if ui.add(egui::Slider::new(&mut tc, 0.0..=100.0).text("Time Compression")).changed() {
                *time_compression = tc as f64;
            }
        }

        ui.separator();
        ui.heading("Quadtree");
        ui.add(egui::Slider::new(&mut chunk_manager.max_subdivisions_per_frame, 1..=32usize)
            .text("Quadtree Budget"));
        ui.add(egui::Slider::new(&mut chunk_manager.quadtree.subdivision_threshold, 1.0..=20.0)
            .text("Subdivision Threshold"));
        ui.add(egui::Slider::new(&mut chunk_manager.quadtree.merge_threshold, 2.0..=30.0)
            .text("Merge Threshold"));

        ui.separator();
        ui.heading("Telemetry");
        if let Ok((transform, velocity, stats)) = player_query.single() {
            // Calculate altitude relative to actual terrain surface at this location
            let player_pos = transform.translation;
            let distance_from_center = player_pos.length();
            let direction_from_center = player_pos.normalize();
            
            // Get terrain elevation at this location
            let elevation_factor = if let Some(ref tn) = terrain_noise {
                tn.get_elevation(direction_from_center)
            } else {
                0.0
            };
            
            // Calculate actual terrain surface radius at this location
            let height_mult = 1.0 + elevation_factor * 0.1;
            let actual_terrain_radius = planet_settings.radius * height_mult;
            
            // Altitude is distance from center minus terrain surface radius
            let altitude = distance_from_center - actual_terrain_radius;
            
            let speed = velocity.linvel.length();
            let density = if altitude > 0.0 {
                planet_settings.air_density_sea_level * (-altitude / planet_settings.atmosphere_height).exp()
            } else {
                planet_settings.air_density_sea_level
            };
            let drag = 0.5 * density * speed * speed * 0.5;

            ui.label(format!("Altitude: {:.1} m", altitude));
            ui.label(format!("Speed: {:.1} m/s", speed));
            ui.label(format!("Air Density: {:.3} kg/m^3", density));
            ui.label(format!("Est. Drag: {:.2} N", drag));
            
            if let Some(orbital_stats) = stats {
                ui.separator();
                ui.heading("Orbit");
                ui.label(format!("Apoapsis: {:.1} m", orbital_stats.apoapsis));
                ui.label(format!("Periapsis: {:.1} m", orbital_stats.periapsis));
                ui.label(format!("Eccentricity: {:.3}", orbital_stats.eccentricity));
                ui.label(format!("Period: {:.1} s", orbital_stats.period));
            }
        }

        ui.separator();
        ui.label("Controls:");
        ui.label("WASD: Move");
        ui.label("Mouse: Look");
        ui.label("C: Toggle Camera");
        ui.label("J: Toggle Journal");
    });
    }
}

use crate::plugins::observation::ObservationJournal;

fn journal_ui_system(
    mut contexts: EguiContexts,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut journal_query: Query<&mut ObservationJournal, With<Player>>,
    state: Res<State<GameState>>,
) {
    if *state.get() != GameState::Playing {
        return;
    }

    // Toggle journal with J key
    if keyboard_input.just_pressed(KeyCode::KeyJ) {
        if let Ok(mut journal) = journal_query.single_mut() {
            journal.show = !journal.show;
        }
    }

    let Ok(journal) = journal_query.single() else { return };
    if !journal.show || journal.entries.is_empty() {
        return;
    }

    if let Ok(ctx) = contexts.ctx_mut() {
        egui::Window::new("Observation Journal")
            .default_width(350.0)
            .default_height(300.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in journal.entries.iter().rev() {
                        ui.group(|ui| {
                            ui.label(
                                egui::RichText::new(format!("[{:.0}s]", entry.timestamp))
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                            ui.label(&entry.text);
                        });
                        ui.add_space(4.0);
                    }
                });
            });
    }
}
