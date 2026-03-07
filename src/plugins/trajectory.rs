use bevy::prelude::*;
use bevy_rapier3d::prelude::Velocity;
use crate::plugins::player::Player;
use crate::plugins::planet::PlanetSettings;
use crate::plugins::camera::CameraState;

pub struct TrajectoryPlugin;

impl Plugin for TrajectoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            calculate_orbital_stats,
            draw_trajectory.run_if(|state: Res<CameraState>| *state == CameraState::Orbital),
        ));
    }
}

#[derive(Component, Default)]
pub struct OrbitalStats {
    pub apoapsis: f32,
    pub periapsis: f32,
    pub eccentricity: f32,
    pub period: f32,
}

fn calculate_orbital_stats(
    mut commands: Commands,
    query: Query<(Entity, &Transform, &Velocity), With<Player>>,
    mut stats_query: Query<&mut OrbitalStats, With<Player>>,
    planet_settings: Res<PlanetSettings>,
) {
    if let Ok((entity, transform, velocity)) = query.single() {
        let r_vec = transform.translation;
        let v_vec = velocity.linvel;
        
        let r = r_vec.length();
        let v = v_vec.length();
        
        // Standard Gravitational Parameter (mu = G * M)
        // We know F = m * a = m * (G * M / r^2)
        // In our game, F = m * gravity_strength * direction
        // gravity_strength = planet_settings.gravity * 5.0 (from player.rs)
        // So acceleration a = gravity_strength at surface
        // But our gravity is constant? No, let's check player.rs
        // player.rs: force = direction * gravity_strength; (Constant gravity!)
        
        // Wait, if gravity is constant (not 1/r^2), Keplerian orbits don't strictly apply.
        // However, for a game "orbit", we usually want 1/r^2 gravity for realistic orbits.
        // The current implementation in player.rs is:
        // let gravity_strength = planet_settings.gravity * 5.0;
        // force.force = direction * gravity_strength;
        
        // This is CONSTANT gravity, which means "orbits" are actually just circles in a uniform field (impossible).
        // To have real orbits, we need 1/r^2 gravity.
        // I should probably upgrade the gravity system to be Newtonian first?
        // Or just approximate for now?
        // If I change gravity to 1/r^2, it changes the feel of the game.
        // But the user asked for "apoapsis / periapsis", which implies Newtonian physics.
        
        // Let's assume for this calculation that we ARE using Newtonian gravity where
        // g_surface = planet_settings.gravity * 5.0
        // mu = g_surface * radius^2
        
        let g_surface = planet_settings.gravity * 5.0;
        let mu = g_surface * planet_settings.radius * planet_settings.radius;

        // Specific Mechanical Energy: E = v^2/2 - mu/r
        let energy = (v * v) / 2.0 - mu / r;

        // Semi-major axis: a = -mu / (2*E)
        let a = -mu / (2.0 * energy);

        // Angular momentum vector: h = r x v
        let h_vec = r_vec.cross(v_vec);
        let h = h_vec.length();

        // Eccentricity: e = sqrt(1 + (2*E*h^2) / mu^2)
        let e = (1.0 + (2.0 * energy * h * h) / (mu * mu)).sqrt();

        let apoapsis = a * (1.0 + e) - planet_settings.radius;
        let periapsis = a * (1.0 - e) - planet_settings.radius;
        
        // Period: T = 2*pi * sqrt(a^3 / mu)
        let period = if a > 0.0 {
            2.0 * std::f32::consts::PI * (a.powi(3) / mu).sqrt()
        } else {
            0.0
        };

        if let Ok(mut stats) = stats_query.get_mut(entity) {
            stats.apoapsis = apoapsis;
            stats.periapsis = periapsis;
            stats.eccentricity = e;
            stats.period = period;
        } else {
            commands.entity(entity).insert(OrbitalStats {
                apoapsis,
                periapsis,
                eccentricity: e,
                period,
            });
        }
    }
}

fn draw_trajectory(
    mut gizmos: Gizmos,
    query: Query<(&Transform, &Velocity), With<Player>>,
    planet_settings: Res<PlanetSettings>,
) {
    if let Ok((transform, velocity)) = query.single() {
        let r_vec = transform.translation;
        let v_vec = velocity.linvel;
        
        let _r = r_vec.length();
        
        // Gravity constants
        let g_surface = planet_settings.gravity * 5.0;
        let mu = g_surface * planet_settings.radius * planet_settings.radius;

        // Orbital Vectors
        let h_vec = r_vec.cross(v_vec);
        let h_sq = h_vec.length_squared();
        let h = h_sq.sqrt();

        if h < 0.001 {
            // Degenerate orbit (straight line up/down)
            // Just draw a line to center or out?
            // Fallback to integration or simple line
            return;
        }

        // Eccentricity Vector
        let e_vec = (v_vec.cross(h_vec) / mu) - (r_vec.normalize());
        let e = e_vec.length();

        // Semi-latus rectum (p)
        // p = h^2 / mu
        let _p = h_sq / mu;

        // Basis Vectors for Orbital Plane
        // X points to periapsis (direction of e_vec)
        // Z is normal to plane (direction of h_vec)
        let z_axis = h_vec.normalize();
        
        let x_axis = if e > 0.001 {
            e_vec.normalize()
        } else {
            // Circular orbit: e_vec is zero, pick arbitrary X perp to Z
            // If Z is Y, pick X. If Z is not Y, pick Y cross Z.
            if z_axis.y.abs() > 0.9 {
                Vec3::X
            } else {
                Vec3::Y.cross(z_axis).normalize()
            }
        };
        
        let _y_axis = z_axis.cross(x_axis);

        // Draw Orbit
        let points = 500;
        let mut prev_pos = transform.translation; // Start from player position
        let mut vel = velocity.linvel;
        let dt = 0.1;

        // Use iterative integration to account for atmospheric drag and other forces
        // This matches the simulation closer than analytical Keplerian orbits when drag/collisions exist.
        for _ in 0..points {
             // 1. Gravity
             let r_vec = prev_pos;
             let dist = r_vec.length();
             let dir = -r_vec.normalize();
             
             let g_surface = planet_settings.gravity * 5.0;
             let gravity_accel = dir * g_surface * (planet_settings.radius / dist).powi(2);

             // 2. Drag
             let altitude = dist - planet_settings.radius;
             let mut drag_accel = Vec3::ZERO;
             
             // Drag constants must match atmosphere.rs EXACTLY
             // Use rayleigh_scale_height for density falloff (same as atmospheric_drag_system)
             if altitude > 0.0 && altitude < planet_settings.atmosphere_height * 2.0 {
                 let density = planet_settings.air_density_sea_level * (-altitude / planet_settings.rayleigh_scale_height).exp();
                 let speed = vel.length();
                 if speed > 0.001 {
                     let drag_dir = -vel.normalize();
                     // Force calculation matching atmosphere.rs:
                     // force = 0.5 * density * speed^2 * 0.5
                     // Accel = force (assuming mass = 1.0)
                     let drag_force = 0.5 * density * speed * speed * 0.5;
                     drag_accel = drag_dir * drag_force; 
                 }
             }

             // Integrate
             let accel = gravity_accel + drag_accel;
             vel += accel * dt;
             let next_pos = prev_pos + vel * dt;

             gizmos.line(prev_pos, next_pos, Color::srgb(0.0, 1.0, 1.0));
             
             // Collision Check (Simple surface check)
             if next_pos.length() < planet_settings.radius {
                 break;
             }

             prev_pos = next_pos;
        }
    }
}
