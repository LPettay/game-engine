// Thermodynamics Simulation Module
// Heat transfer, phase transitions, and thermal equilibrium
//
// Supports:
// - Conductive heat transfer between adjacent bodies
// - Radiative heat transfer (Stefan-Boltzmann law)
// - Convective heat transfer in fluids
// - Phase transitions (melting, boiling, sublimation)

use bevy::prelude::*;
use super::{PhysicalConstants, ThermalBody, GravitationalBody};

/// Update heat transfer between thermal bodies
pub fn update_heat_transfer(
    constants: Res<PhysicalConstants>,
    time: Res<Time>,
    mut bodies: Query<(&mut ThermalBody, Option<&GravitationalBody>)>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // For now, just handle radiative heat transfer
    // Full conductive/convective transfer requires spatial awareness
    
    // Background temperature (cosmic microwave background in space)
    let background_temp: f64 = 2.725; // K
    
    for (mut thermal, grav_opt) in bodies.iter_mut() {
        // Calculate radiative heat loss
        let mass = grav_opt.map(|g| g.mass).unwrap_or(1.0);
        
        // Power radiated = emissivity * Stefan-Boltzmann * area * T^4
        let power_out = thermal.emissivity * constants.stefan_boltzmann 
            * thermal.surface_area * thermal.temperature.powi(4);
        
        // Power absorbed from background
        let power_in = thermal.emissivity * constants.stefan_boltzmann 
            * thermal.surface_area * background_temp.powi(4);
        
        // Net power loss
        let net_power = power_out - power_in;
        
        // Temperature change: dT = (power * dt) / (mass * specific_heat)
        let heat_capacity = mass * thermal.specific_heat;
        if heat_capacity > 0.0 {
            let dt_temp = (net_power * dt) / heat_capacity;
            thermal.temperature = (thermal.temperature - dt_temp).max(background_temp);
        }
    }
}

/// Calculate conductive heat transfer between two bodies in contact
/// Returns heat flow rate in Watts (positive = from body1 to body2)
pub fn conductive_heat_transfer(
    body1: &ThermalBody,
    body2: &ThermalBody,
    contact_area: f64,
    separation: f64,
) -> f64 {
    if separation <= 0.0 {
        return 0.0;
    }
    
    // Use harmonic mean of conductivities for interface
    let k_effective = 2.0 * body1.thermal_conductivity * body2.thermal_conductivity 
        / (body1.thermal_conductivity + body2.thermal_conductivity);
    
    // Fourier's law: Q = k * A * dT / dx
    k_effective * contact_area * (body1.temperature - body2.temperature) / separation
}

/// Calculate radiative heat transfer between two bodies
/// Returns heat flow rate in Watts (positive = from body1 to body2)
pub fn radiative_heat_transfer(
    constants: &PhysicalConstants,
    body1: &ThermalBody,
    body2: &ThermalBody,
    view_factor: f64, // Fraction of radiation from body1 reaching body2 (0-1)
) -> f64 {
    // Net radiative transfer using Stefan-Boltzmann law
    let effective_emissivity = body1.emissivity * body2.emissivity;
    
    constants.stefan_boltzmann * effective_emissivity * view_factor * body1.surface_area
        * (body1.temperature.powi(4) - body2.temperature.powi(4))
}

/// Phase transition data for a material
#[derive(Clone, Debug)]
pub struct PhaseTransitionData {
    /// Melting point (K)
    pub melting_point: f64,
    /// Boiling point at 1 atm (K)
    pub boiling_point: f64,
    /// Heat of fusion (J/kg)
    pub heat_of_fusion: f64,
    /// Heat of vaporization (J/kg)
    pub heat_of_vaporization: f64,
    /// Heat of sublimation (J/kg) - for materials that sublimate
    pub heat_of_sublimation: f64,
    /// Triple point temperature (K)
    pub triple_point_temp: f64,
    /// Triple point pressure (Pa)
    pub triple_point_pressure: f64,
}

impl PhaseTransitionData {
    /// Water phase transition data
    pub fn water() -> Self {
        Self {
            melting_point: 273.15,
            boiling_point: 373.15,
            heat_of_fusion: 334_000.0,       // 334 kJ/kg
            heat_of_vaporization: 2_260_000.0, // 2.26 MJ/kg
            heat_of_sublimation: 2_594_000.0,  // 2.594 MJ/kg
            triple_point_temp: 273.16,
            triple_point_pressure: 611.657,
        }
    }
    
    /// Iron phase transition data
    pub fn iron() -> Self {
        Self {
            melting_point: 1811.0,
            boiling_point: 3134.0,
            heat_of_fusion: 247_000.0,
            heat_of_vaporization: 6_090_000.0,
            heat_of_sublimation: 6_337_000.0,
            triple_point_temp: 1811.0,  // Approximate
            triple_point_pressure: 0.01, // Very low
        }
    }
    
    /// Determine the phase at given temperature and pressure
    pub fn phase_at(&self, temperature: f64, pressure: f64) -> Phase {
        // Simplified phase determination
        // Real systems use full phase diagrams
        
        if temperature < self.triple_point_temp && pressure < self.triple_point_pressure {
            // Below triple point - solid or gas
            if temperature < self.melting_point * 0.5 {
                Phase::Solid
            } else {
                Phase::Gas
            }
        } else if temperature < self.melting_point {
            Phase::Solid
        } else if temperature < self.boiling_point {
            Phase::Liquid
        } else {
            Phase::Gas
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Solid,
    Liquid,
    Gas,
    Plasma, // For very high temperatures
    Supercritical, // Above critical point
}

/// Calculate the equilibrium temperature of a body at a given distance from a star
/// Uses Stefan-Boltzmann law and geometric dilution
pub fn equilibrium_temperature(
    star_temperature: f64,
    star_radius: f64,
    orbital_radius: f64,
    albedo: f64, // Fraction of light reflected (0-1)
) -> f64 {
    // T_eq = T_star * sqrt(R_star / (2 * d)) * (1 - albedo)^0.25
    // This assumes rapid rotation (uniform temperature)
    
    let geometric_factor = (star_radius / (2.0 * orbital_radius)).sqrt();
    let absorption_factor = (1.0 - albedo).powf(0.25);
    
    star_temperature * geometric_factor * absorption_factor
}

/// Calculate the greenhouse effect temperature increase
/// delta_T = surface_temp * (1 + optical_depth)^0.25 - surface_temp
pub fn greenhouse_effect(
    equilibrium_temp: f64,
    optical_depth: f64, // Atmospheric optical depth for infrared
) -> f64 {
    equilibrium_temp * ((1.0 + optical_depth).powf(0.25) - 1.0)
}

/// Ideal gas law calculations
pub mod ideal_gas {
    use super::*;
    
    /// Calculate pressure from density and temperature
    /// P = ρ * R_specific * T
    /// where R_specific = R / M (gas constant / molar mass)
    pub fn pressure(density: f64, temperature: f64, molar_mass: f64, constants: &PhysicalConstants) -> f64 {
        let r_specific = constants.gas_constant / molar_mass;
        density * r_specific * temperature
    }
    
    /// Calculate density from pressure and temperature
    pub fn density(pressure: f64, temperature: f64, molar_mass: f64, constants: &PhysicalConstants) -> f64 {
        let r_specific = constants.gas_constant / molar_mass;
        pressure / (r_specific * temperature)
    }
    
    /// Calculate temperature from pressure and density
    pub fn temperature(pressure: f64, density: f64, molar_mass: f64, constants: &PhysicalConstants) -> f64 {
        let r_specific = constants.gas_constant / molar_mass;
        pressure / (density * r_specific)
    }
    
    /// Calculate the scale height of an atmosphere
    /// H = k_B * T / (m * g)
    /// where m is the mean molecular mass and g is surface gravity
    pub fn scale_height(temperature: f64, mean_molecular_mass: f64, surface_gravity: f64, constants: &PhysicalConstants) -> f64 {
        constants.boltzmann_constant * temperature / (mean_molecular_mass * surface_gravity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_earth_equilibrium_temp() {
        use super::super::constants::distances::AU;
        use super::super::constants::temperatures::SOLAR_SURFACE;
        
        let solar_radius = 6.96e8;
        let earth_albedo = 0.3; // Earth reflects ~30% of sunlight
        
        let t_eq = equilibrium_temperature(SOLAR_SURFACE, solar_radius, AU, earth_albedo);
        
        // Earth's equilibrium temp should be ~255 K (-18°C)
        // Actual surface is ~288 K (+15°C) due to greenhouse effect
        assert!((t_eq - 255.0).abs() < 10.0);
    }
    
    #[test]
    fn test_water_phases() {
        let water = PhaseTransitionData::water();
        
        assert_eq!(water.phase_at(250.0, 101325.0), Phase::Solid);
        assert_eq!(water.phase_at(300.0, 101325.0), Phase::Liquid);
        assert_eq!(water.phase_at(400.0, 101325.0), Phase::Gas);
    }
}

