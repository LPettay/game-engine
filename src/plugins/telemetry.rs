use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use std::fs::File;
use std::io::Write;
use crate::plugins::player::Player;
use crate::plugins::planet::PlanetSettings;
use crate::plugins::trajectory::OrbitalStats;

pub struct TelemetryPlugin;

impl Plugin for TelemetryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TelemetryLog { file: None })
           .add_systems(Startup, setup_logging)
           .add_systems(Update, log_telemetry);
    }
}

#[derive(Resource)]
struct TelemetryLog {
    file: Option<File>,
}

fn setup_logging(mut log: ResMut<TelemetryLog>) {
    if let Ok(mut file) = File::create("flight_telemetry.csv") {
        writeln!(file, "Time,Altitude,Velocity,VerticalSpeed,HorizontalSpeed,GravityStrength,Apoapsis,Periapsis").unwrap();
        log.file = Some(file);
    }
}

fn log_telemetry(
    time: Res<Time>,
    mut log: ResMut<TelemetryLog>,
    query: Query<(&Transform, &Velocity, Option<&OrbitalStats>), With<Player>>,
    planet_settings: Res<PlanetSettings>,
    terrain_noise: Option<Res<crate::plugins::terrain::TerrainNoise>>,
    mut elapsed: Local<f32>,
) {
    // Throttle writes to ~1/sec instead of every frame
    *elapsed += time.delta_secs();
    if *elapsed < 1.0 {
        return;
    }
    *elapsed = 0.0;

    if let Some(file) = &mut log.file {
        if let Ok((transform, velocity, stats)) = query.single() {
            let r = transform.translation.length();

            // Calculate altitude relative to actual terrain surface at this location
            let player_pos = transform.translation;
            let direction_from_center = player_pos.normalize();

            // Get terrain elevation at this location
            let elevation_factor = terrain_noise.as_ref()
                .map(|tn| tn.get_elevation(direction_from_center))
                .unwrap_or(0.0);
            
            // Calculate actual terrain surface radius at this location
            let height_mult = 1.0 + elevation_factor * 0.1;
            let actual_terrain_radius = planet_settings.radius * height_mult;
            
            // Altitude is distance from center minus terrain surface radius
            let altitude = r - actual_terrain_radius;
            let vel_mag = velocity.linvel.length();
            
            let up = transform.translation.normalize();
            let vert_speed = velocity.linvel.dot(up);
            let horiz_vel = velocity.linvel - (up * vert_speed);
            let horiz_speed = horiz_vel.length();
            
            // Since we removed ExternalForce, we can't log it directly.
            // But we know gravity is being applied to velocity.
            
            // Re-calculate gravity for verification
            let g_surface = planet_settings.gravity * 5.0;
            let gravity_strength = g_surface * (planet_settings.radius / r).powi(2);

            let apo = if let Some(s) = stats { s.apoapsis } else { 0.0 };
            let peri = if let Some(s) = stats { s.periapsis } else { 0.0 };

            writeln!(
                file, 
                "{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
                time.elapsed_secs(),
                altitude,
                vel_mag,
                vert_speed,
                horiz_speed,
                gravity_strength,
                apo,
                peri
            ).ok();
        }
    }
}
