// Physics Foundation Module
// Infinite Universe Engine - Core physics primitives for scientific simulation
//
// This module provides the foundational physics systems that drive
// everything from planetary formation to player-built technology.
// 
// Design principles:
// - Scientifically accurate where practical
// - Simplified models that capture essential behavior
// - Extensible for speculative physics (black holes, multiverse)
// - Performance-optimized for real-time simulation

pub mod constants;
pub mod gravity;
pub mod thermodynamics;
pub mod fluids;
pub mod materials;

use bevy::prelude::*;
use bevy::math::DVec3;

pub struct UniversePhysicsPlugin;

impl Plugin for UniversePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PhysicalConstants>()
           .init_resource::<PhysicsSimulationState>()
           .add_systems(Update, (
               gravity::update_gravitational_forces,
               thermodynamics::update_heat_transfer,
           ).chain());
    }
}

/// Physical constants for this universe
/// These can vary between universes in the multiverse!
#[derive(Resource, Clone, Debug)]
pub struct PhysicalConstants {
    /// Gravitational constant (m³/(kg·s²))
    /// Real value: 6.674e-11
    pub gravitational_constant: f64,
    
    /// Speed of light (m/s)
    /// Real value: 299,792,458
    pub speed_of_light: f64,
    
    /// Planck's constant (J·s)
    /// Real value: 6.626e-34
    pub planck_constant: f64,
    
    /// Boltzmann constant (J/K)
    /// Real value: 1.381e-23
    pub boltzmann_constant: f64,
    
    /// Elementary charge (C)
    /// Real value: 1.602e-19
    pub elementary_charge: f64,
    
    /// Fine structure constant (dimensionless)
    /// Real value: ~1/137 ≈ 0.007297
    /// This affects electromagnetic interactions
    pub fine_structure_constant: f64,
    
    /// Stefan-Boltzmann constant (W/(m²·K⁴))
    /// Real value: 5.670e-8
    pub stefan_boltzmann: f64,
    
    /// Avogadro's number (1/mol)
    /// Real value: 6.022e23
    pub avogadro: f64,
    
    /// Universal gas constant (J/(mol·K))
    /// Real value: 8.314
    pub gas_constant: f64,
}

impl Default for PhysicalConstants {
    fn default() -> Self {
        Self::earth_like()
    }
}

impl PhysicalConstants {
    /// Earth-like universe constants (our reality)
    pub fn earth_like() -> Self {
        Self {
            gravitational_constant: 6.674e-11,
            speed_of_light: 299_792_458.0,
            planck_constant: 6.626e-34,
            boltzmann_constant: 1.381e-23,
            elementary_charge: 1.602e-19,
            fine_structure_constant: 1.0 / 137.036,
            stefan_boltzmann: 5.670e-8,
            avogadro: 6.022e23,
            gas_constant: 8.314,
        }
    }
    
    /// Create a variation for a different universe
    /// Used when traversing black holes into the multiverse
    pub fn varied(seed: u64) -> Self {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Vary constants by ±10% based on seed
        let variation = |base: f64, offset: u64| -> f64 {
            let mut h = DefaultHasher::new();
            (hash + offset).hash(&mut h);
            let v = (h.finish() % 200) as f64 / 1000.0; // 0.0 to 0.2
            base * (0.9 + v)
        };
        
        Self {
            gravitational_constant: variation(6.674e-11, 1),
            speed_of_light: 299_792_458.0, // Keep c constant (fundamental)
            planck_constant: variation(6.626e-34, 2),
            boltzmann_constant: variation(1.381e-23, 3),
            elementary_charge: variation(1.602e-19, 4),
            fine_structure_constant: variation(1.0 / 137.036, 5),
            stefan_boltzmann: variation(5.670e-8, 6),
            avogadro: 6.022e23, // Keep avogadro constant (definitional)
            gas_constant: variation(8.314, 7),
        }
    }
    
    /// Calculate gravitational force between two masses
    pub fn gravitational_force(&self, m1: f64, m2: f64, distance: f64) -> f64 {
        if distance < 1e-10 {
            return 0.0; // Prevent division by near-zero
        }
        self.gravitational_constant * m1 * m2 / (distance * distance)
    }
    
    /// Calculate escape velocity from a body
    pub fn escape_velocity(&self, mass: f64, radius: f64) -> f64 {
        (2.0 * self.gravitational_constant * mass / radius).sqrt()
    }
    
    /// Calculate orbital velocity at a given radius
    pub fn orbital_velocity(&self, central_mass: f64, orbital_radius: f64) -> f64 {
        (self.gravitational_constant * central_mass / orbital_radius).sqrt()
    }
    
    /// Calculate Schwarzschild radius (event horizon) of a black hole
    pub fn schwarzschild_radius(&self, mass: f64) -> f64 {
        2.0 * self.gravitational_constant * mass / (self.speed_of_light * self.speed_of_light)
    }
    
    /// Calculate thermal radiation power (Stefan-Boltzmann law)
    pub fn thermal_radiation_power(&self, area: f64, temperature: f64) -> f64 {
        self.stefan_boltzmann * area * temperature.powi(4)
    }
    
    /// Calculate time dilation factor at given velocity relative to c
    pub fn time_dilation(&self, velocity: f64) -> f64 {
        let beta = velocity / self.speed_of_light;
        if beta >= 1.0 {
            f64::INFINITY
        } else {
            1.0 / (1.0 - beta * beta).sqrt()
        }
    }
    
    /// Calculate gravitational time dilation near a massive body
    pub fn gravitational_time_dilation(&self, mass: f64, distance: f64) -> f64 {
        let rs = self.schwarzschild_radius(mass);
        if distance <= rs {
            f64::INFINITY // Inside event horizon
        } else {
            1.0 / (1.0 - rs / distance).sqrt()
        }
    }
}

/// State of the physics simulation
#[derive(Resource, Default)]
pub struct PhysicsSimulationState {
    /// Total energy in the simulation (for conservation checks)
    pub total_energy: f64,
    /// Number of physics objects being simulated
    pub object_count: usize,
    /// Whether relativistic effects are enabled
    pub relativistic_enabled: bool,
    /// Whether quantum effects are enabled (for advanced players)
    pub quantum_enabled: bool,
}

/// Component for objects affected by gravity
#[derive(Component, Clone, Debug)]
pub struct GravitationalBody {
    /// Mass in kilograms
    pub mass: f64,
    /// Velocity in m/s
    pub velocity: DVec3,
    /// Whether this body exerts gravity on others
    pub is_gravity_source: bool,
    /// Whether this body is affected by gravity
    pub is_affected_by_gravity: bool,
}

impl Default for GravitationalBody {
    fn default() -> Self {
        Self {
            mass: 1.0,
            velocity: DVec3::ZERO,
            is_gravity_source: false,
            is_affected_by_gravity: true,
        }
    }
}

/// Component for thermal simulation
#[derive(Component, Clone, Debug)]
pub struct ThermalBody {
    /// Temperature in Kelvin
    pub temperature: f64,
    /// Specific heat capacity (J/(kg·K))
    pub specific_heat: f64,
    /// Thermal conductivity (W/(m·K))
    pub thermal_conductivity: f64,
    /// Surface area for radiation (m²)
    pub surface_area: f64,
    /// Emissivity (0-1)
    pub emissivity: f64,
}

impl Default for ThermalBody {
    fn default() -> Self {
        Self {
            temperature: 293.0, // ~20°C (room temperature)
            specific_heat: 1000.0, // Generic solid
            thermal_conductivity: 1.0,
            surface_area: 1.0,
            emissivity: 0.5,
        }
    }
}

/// Component for fluid simulation
#[derive(Component, Clone, Debug)]
pub struct FluidBody {
    /// Density (kg/m³)
    pub density: f64,
    /// Viscosity (Pa·s)
    pub viscosity: f64,
    /// Pressure (Pa)
    pub pressure: f64,
    /// Flow velocity (m/s)
    pub flow_velocity: DVec3,
}

impl Default for FluidBody {
    fn default() -> Self {
        Self {
            density: 1000.0, // Water at ~20°C
            viscosity: 0.001, // Water
            pressure: 101325.0, // 1 atm
            flow_velocity: DVec3::ZERO,
        }
    }
}

/// Marker for objects that emit radiation
#[derive(Component)]
pub struct RadiationEmitter {
    /// Power output in Watts
    pub power: f64,
    /// Type of radiation (for gameplay effects)
    pub radiation_type: RadiationType,
}

#[derive(Clone, Debug, Default)]
pub enum RadiationType {
    #[default]
    Thermal,
    Electromagnetic,
    Nuclear,
    Exotic, // For speculative physics
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gravitational_force() {
        let constants = PhysicalConstants::earth_like();
        // Earth-Moon system
        let earth_mass = 5.972e24; // kg
        let moon_mass = 7.342e22; // kg
        let distance = 384_400_000.0; // meters
        
        let force = constants.gravitational_force(earth_mass, moon_mass, distance);
        // Expected: ~1.98e20 N
        assert!((force - 1.98e20).abs() < 0.1e20);
    }
    
    #[test]
    fn test_escape_velocity() {
        let constants = PhysicalConstants::earth_like();
        // Earth
        let earth_mass = 5.972e24;
        let earth_radius = 6.371e6;
        
        let v_escape = constants.escape_velocity(earth_mass, earth_radius);
        // Expected: ~11,186 m/s
        assert!((v_escape - 11186.0).abs() < 100.0);
    }
    
    #[test]
    fn test_time_dilation() {
        let constants = PhysicalConstants::earth_like();
        
        // At 0.5c
        let gamma_50 = constants.time_dilation(0.5 * constants.speed_of_light);
        // Expected: ~1.155
        assert!((gamma_50 - 1.1547).abs() < 0.01);
        
        // At 0.99c
        let gamma_99 = constants.time_dilation(0.99 * constants.speed_of_light);
        // Expected: ~7.09
        assert!((gamma_99 - 7.09).abs() < 0.1);
    }
    
    #[test]
    fn test_schwarzschild_radius() {
        let constants = PhysicalConstants::earth_like();
        
        // Sun's Schwarzschild radius
        let sun_mass = 1.989e30;
        let rs = constants.schwarzschild_radius(sun_mass);
        // Expected: ~2953 m (about 3 km)
        assert!((rs - 2953.0).abs() < 10.0);
    }
}

