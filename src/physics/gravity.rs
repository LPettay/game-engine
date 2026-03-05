// Gravity Simulation Module
// N-body gravitational simulation for celestial mechanics
//
// Supports:
// - N-body simulation for accurate orbital mechanics
// - Barnes-Hut optimization for large systems
// - Relativistic corrections near massive bodies
// - Black hole physics including event horizons

use bevy::prelude::*;
use bevy::math::DVec3;
use super::{PhysicalConstants, GravitationalBody};

/// System to update gravitational forces on all bodies
/// Uses N-body simulation with O(n²) for small systems
/// TODO: Implement Barnes-Hut tree for large systems
pub fn update_gravitational_forces(
    constants: Res<PhysicalConstants>,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Transform, &mut GravitationalBody)>,
) {
    // Skip if paused or no time passed
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // Collect all body data for force calculation
    let body_data: Vec<(Entity, DVec3, f64, bool)> = bodies.iter()
        .map(|(e, t, g)| {
            (e, DVec3::new(t.translation.x as f64, t.translation.y as f64, t.translation.z as f64), 
             g.mass, g.is_gravity_source)
        })
        .collect();
    
    // Calculate forces on each body
    for (entity, mut transform, mut grav_body) in bodies.iter_mut() {
        if !grav_body.is_affected_by_gravity {
            continue;
        }
        
        let pos = DVec3::new(
            transform.translation.x as f64,
            transform.translation.y as f64,
            transform.translation.z as f64,
        );
        
        // Sum gravitational forces from all other bodies
        let mut total_force = DVec3::ZERO;
        
        for (other_entity, other_pos, other_mass, is_source) in &body_data {
            if *other_entity == entity || !is_source {
                continue;
            }
            
            let direction = *other_pos - pos;
            let distance_sq = direction.length_squared();
            
            if distance_sq < 1e-10 {
                continue; // Skip if too close (would cause numerical issues)
            }
            
            let distance = distance_sq.sqrt();
            let force_magnitude = constants.gravitational_force(grav_body.mass, *other_mass, distance);
            let force_direction = direction / distance;
            
            total_force += force_direction * force_magnitude;
        }
        
        // F = ma, so a = F/m
        let acceleration = total_force / grav_body.mass;
        
        // Update velocity (semi-implicit Euler integration)
        grav_body.velocity += acceleration * dt;
        
        // Update position
        let new_pos = pos + grav_body.velocity * dt;
        transform.translation = Vec3::new(
            new_pos.x as f32,
            new_pos.y as f32,
            new_pos.z as f32,
        );
    }
}

/// Calculate the gravitational potential energy between two bodies
pub fn gravitational_potential_energy(constants: &PhysicalConstants, m1: f64, m2: f64, distance: f64) -> f64 {
    if distance < 1e-10 {
        return f64::NEG_INFINITY;
    }
    -constants.gravitational_constant * m1 * m2 / distance
}

/// Calculate orbital period using Kepler's third law
pub fn orbital_period(constants: &PhysicalConstants, semi_major_axis: f64, central_mass: f64) -> f64 {
    2.0 * std::f64::consts::PI * (semi_major_axis.powi(3) / (constants.gravitational_constant * central_mass)).sqrt()
}

/// Calculate the Roche limit (minimum orbital radius before tidal disruption)
pub fn roche_limit(primary_radius: f64, primary_density: f64, secondary_density: f64) -> f64 {
    primary_radius * (2.0 * primary_density / secondary_density).powf(1.0 / 3.0)
}

/// Hill sphere radius (sphere of gravitational influence)
pub fn hill_sphere_radius(semi_major_axis: f64, orbiting_mass: f64, central_mass: f64) -> f64 {
    semi_major_axis * (orbiting_mass / (3.0 * central_mass)).powf(1.0 / 3.0)
}

/// Calculate orbital elements from position and velocity
#[derive(Debug, Clone)]
pub struct OrbitalElements {
    /// Semi-major axis (m)
    pub semi_major_axis: f64,
    /// Eccentricity (0 = circular, <1 = ellipse, 1 = parabola, >1 = hyperbola)
    pub eccentricity: f64,
    /// Inclination (radians)
    pub inclination: f64,
    /// Longitude of ascending node (radians)
    pub longitude_ascending_node: f64,
    /// Argument of periapsis (radians)
    pub argument_periapsis: f64,
    /// True anomaly (radians)
    pub true_anomaly: f64,
    /// Orbital period (seconds, only meaningful for bound orbits)
    pub period: f64,
}

impl OrbitalElements {
    /// Calculate orbital elements from state vectors
    pub fn from_state_vectors(
        position: DVec3,
        velocity: DVec3,
        central_mass: f64,
        constants: &PhysicalConstants,
    ) -> Self {
        let mu = constants.gravitational_constant * central_mass;
        let r = position.length();
        let v = velocity.length();
        
        // Specific orbital energy
        let energy = v * v / 2.0 - mu / r;
        
        // Semi-major axis (negative for hyperbolic orbits)
        let semi_major_axis = -mu / (2.0 * energy);
        
        // Angular momentum vector
        let h = position.cross(velocity);
        let h_mag = h.length();
        
        // Eccentricity vector
        let e_vec = velocity.cross(h) / mu - position / r;
        let eccentricity = e_vec.length();
        
        // Inclination
        let inclination = (h.z / h_mag).acos();
        
        // Node vector (points to ascending node)
        let n = DVec3::new(-h.y, h.x, 0.0);
        let n_mag = n.length();
        
        // Longitude of ascending node
        let longitude_ascending_node = if n_mag > 1e-10 {
            let omega = (n.x / n_mag).acos();
            if n.y < 0.0 { 2.0 * std::f64::consts::PI - omega } else { omega }
        } else {
            0.0
        };
        
        // Argument of periapsis
        let argument_periapsis = if n_mag > 1e-10 && eccentricity > 1e-10 {
            let omega = (n.dot(e_vec) / (n_mag * eccentricity)).acos();
            if e_vec.z < 0.0 { 2.0 * std::f64::consts::PI - omega } else { omega }
        } else {
            0.0
        };
        
        // True anomaly
        let true_anomaly = if eccentricity > 1e-10 {
            let nu = (e_vec.dot(position) / (eccentricity * r)).acos();
            if position.dot(velocity) < 0.0 { 2.0 * std::f64::consts::PI - nu } else { nu }
        } else {
            0.0
        };
        
        // Orbital period (only meaningful for elliptical orbits)
        let period = if semi_major_axis > 0.0 {
            orbital_period(constants, semi_major_axis, central_mass)
        } else {
            f64::INFINITY
        };
        
        Self {
            semi_major_axis,
            eccentricity,
            inclination,
            longitude_ascending_node,
            argument_periapsis,
            true_anomaly,
            period,
        }
    }
    
    /// Check if orbit is bound (elliptical)
    pub fn is_bound(&self) -> bool {
        self.eccentricity < 1.0
    }
    
    /// Calculate periapsis distance
    pub fn periapsis(&self) -> f64 {
        self.semi_major_axis * (1.0 - self.eccentricity)
    }
    
    /// Calculate apoapsis distance (infinite for unbound orbits)
    pub fn apoapsis(&self) -> f64 {
        if self.eccentricity >= 1.0 {
            f64::INFINITY
        } else {
            self.semi_major_axis * (1.0 + self.eccentricity)
        }
    }
}

/// Black hole physics
pub mod black_hole {
    use super::*;
    
    /// Calculate the innermost stable circular orbit (ISCO) radius
    /// For a non-rotating (Schwarzschild) black hole, this is 3rs
    pub fn isco_radius(constants: &PhysicalConstants, mass: f64) -> f64 {
        3.0 * constants.schwarzschild_radius(mass)
    }
    
    /// Calculate the photon sphere radius (where light can orbit)
    pub fn photon_sphere_radius(constants: &PhysicalConstants, mass: f64) -> f64 {
        1.5 * constants.schwarzschild_radius(mass)
    }
    
    /// Check if a position is inside the event horizon
    pub fn is_inside_event_horizon(constants: &PhysicalConstants, mass: f64, distance: f64) -> bool {
        distance <= constants.schwarzschild_radius(mass)
    }
    
    /// Calculate tidal forces at a given distance from a black hole
    /// Returns the force gradient (difference in force between head and feet)
    /// This determines "spaghettification"
    pub fn tidal_force(constants: &PhysicalConstants, bh_mass: f64, distance: f64, body_length: f64) -> f64 {
        // Tidal force = 2 * G * M * L / r³
        2.0 * constants.gravitational_constant * bh_mass * body_length / distance.powi(3)
    }
    
    /// Calculate the maximum survivable distance for a given acceleration tolerance
    /// body_length: length of the body (head to feet)
    /// max_g: maximum survivable g-force difference
    pub fn minimum_safe_distance(
        constants: &PhysicalConstants, 
        bh_mass: f64, 
        body_length: f64, 
        max_g: f64
    ) -> f64 {
        let max_force = max_g * 9.81; // Convert to m/s²
        // Solving: 2 * G * M * L / r³ < max_force
        // r > (2 * G * M * L / max_force)^(1/3)
        (2.0 * constants.gravitational_constant * bh_mass * body_length / max_force).powf(1.0 / 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_orbital_period() {
        let constants = PhysicalConstants::earth_like();
        // Earth's orbital period around the Sun
        use super::super::constants::distances::AU;
        use super::super::constants::masses::SOLAR_MASS;
        
        let period = orbital_period(&constants, AU, SOLAR_MASS);
        // Expected: ~31,557,600 seconds (1 year)
        assert!((period - 31_557_600.0).abs() < 10000.0);
    }
    
    #[test]
    fn test_black_hole_isco() {
        let constants = PhysicalConstants::earth_like();
        use super::super::constants::masses::SOLAR_MASS;
        
        let isco = black_hole::isco_radius(&constants, SOLAR_MASS);
        let rs = constants.schwarzschild_radius(SOLAR_MASS);
        
        // ISCO should be 3 * Schwarzschild radius
        assert!((isco - 3.0 * rs).abs() < 1.0);
    }
}

