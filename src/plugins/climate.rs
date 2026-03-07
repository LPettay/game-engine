// Climate Simulation Module
// Infinite Universe Engine - Atmosphere and weather simulation
//
// Simulates:
// - Atmospheric composition and pressure
// - Global circulation patterns (Hadley, Ferrel, Polar cells)
// - Ocean currents (thermohaline, wind-driven)
// - Weather patterns and precipitation
// - Seasonal variations from axial tilt

use bevy::prelude::*;

pub struct ClimatePlugin;

impl Plugin for ClimatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClimateState>()
           .init_resource::<AtmosphereConfig>()
           .add_systems(Update, update_climate.run_if(in_state(crate::GameState::Playing)));
    }
}

/// Global climate state
#[derive(Resource)]
pub struct ClimateState {
    /// Atmospheric grid
    pub atmosphere: AtmosphereGrid,
    /// Ocean grid
    pub ocean: OceanGrid,
    /// Current season (0.0-1.0, 0=spring equinox)
    pub season: f64,
    /// Day of year (0.0-1.0)
    pub day_of_year: f64,
    /// Climate simulation settings
    pub settings: ClimateSettings,
}

impl Default for ClimateState {
    fn default() -> Self {
        Self {
            atmosphere: AtmosphereGrid::new(72, 36, 10), // 5-degree resolution, 10 altitude levels
            ocean: OceanGrid::new(72, 36, 5),            // 5-degree resolution, 5 depth levels
            season: 0.0,
            day_of_year: 0.0,
            settings: ClimateSettings::default(),
        }
    }
}

/// Atmosphere configuration for a planet
#[derive(Resource, Clone, Debug)]
pub struct AtmosphereConfig {
    /// Surface pressure (Pa)
    pub surface_pressure: f64,
    /// Composition (fraction by mass)
    pub composition: AtmosphereComposition,
    /// Scale height (m)
    pub scale_height: f64,
    /// Average temperature at surface (K)
    pub surface_temperature: f64,
    /// Lapse rate (K/m)
    pub lapse_rate: f64,
    /// Axial tilt (radians)
    pub axial_tilt: f64,
    /// Rotation period (seconds)
    pub rotation_period: f64,
}

impl Default for AtmosphereConfig {
    fn default() -> Self {
        Self::earth_like()
    }
}

impl AtmosphereConfig {
    pub fn earth_like() -> Self {
        Self {
            surface_pressure: 101325.0,
            composition: AtmosphereComposition::earth_like(),
            scale_height: 8500.0,
            surface_temperature: 288.0, // 15°C
            lapse_rate: 0.0065,          // 6.5 K/km
            axial_tilt: 0.4091,          // 23.4 degrees
            rotation_period: 86400.0,    // 24 hours
        }
    }
    
    pub fn mars_like() -> Self {
        Self {
            surface_pressure: 636.0, // ~0.6% of Earth
            composition: AtmosphereComposition::mars_like(),
            scale_height: 11100.0,
            surface_temperature: 210.0, // -63°C
            lapse_rate: 0.0025,
            axial_tilt: 0.4396, // 25.2 degrees
            rotation_period: 88620.0, // 24h 37m
        }
    }
}

/// Atmospheric composition
#[derive(Clone, Debug)]
pub struct AtmosphereComposition {
    pub nitrogen: f64,
    pub oxygen: f64,
    pub carbon_dioxide: f64,
    pub water_vapor: f64,
    pub argon: f64,
    pub other: f64,
}

impl AtmosphereComposition {
    pub fn earth_like() -> Self {
        Self {
            nitrogen: 0.7808,
            oxygen: 0.2095,
            carbon_dioxide: 0.0004,
            water_vapor: 0.01, // Variable
            argon: 0.0093,
            other: 0.0,
        }
    }
    
    pub fn mars_like() -> Self {
        Self {
            nitrogen: 0.027,
            oxygen: 0.0013,
            carbon_dioxide: 0.9532,
            water_vapor: 0.0003,
            argon: 0.016,
            other: 0.002,
        }
    }
    
    /// Mean molecular weight (g/mol)
    pub fn mean_molecular_weight(&self) -> f64 {
        self.nitrogen * 28.0 +
        self.oxygen * 32.0 +
        self.carbon_dioxide * 44.0 +
        self.water_vapor * 18.0 +
        self.argon * 40.0
    }
}

/// 3D grid for atmospheric simulation
#[derive(Clone)]
pub struct AtmosphereGrid {
    pub resolution: (usize, usize, usize), // (lon, lat, altitude)
    /// Temperature (K)
    pub temperature: Vec<f64>,
    /// Pressure (Pa)
    pub pressure: Vec<f64>,
    /// Humidity (0-1)
    pub humidity: Vec<f64>,
    /// Wind velocity (m/s)
    pub wind: Vec<Vec3>,
    /// Cloud cover (0-1)
    pub cloud_cover: Vec<f64>,
}

impl AtmosphereGrid {
    pub fn new(nx: usize, ny: usize, nz: usize) -> Self {
        let size = nx * ny * nz;
        Self {
            resolution: (nx, ny, nz),
            temperature: vec![288.0; size],
            pressure: vec![101325.0; size],
            humidity: vec![0.5; size],
            wind: vec![Vec3::ZERO; size],
            cloud_cover: vec![0.0; size],
        }
    }
    
    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        let (nx, ny, _) = self.resolution;
        x + y * nx + z * nx * ny
    }
    
    /// Get temperature at a position (interpolated)
    pub fn sample_temperature(&self, lon: f64, lat: f64, alt_fraction: f64) -> f64 {
        let (nx, ny, nz) = self.resolution;
        
        let gx = ((lon / (2.0 * std::f64::consts::PI) + 0.5) * nx as f64) as usize % nx;
        let gy = ((lat / std::f64::consts::PI + 0.5) * ny as f64).clamp(0.0, (ny - 1) as f64) as usize;
        let gz = (alt_fraction * nz as f64).clamp(0.0, (nz - 1) as f64) as usize;
        
        self.temperature[self.index(gx, gy, gz)]
    }
    
    /// Get wind at a position (interpolated)
    pub fn sample_wind(&self, lon: f64, lat: f64, alt_fraction: f64) -> Vec3 {
        let (nx, ny, nz) = self.resolution;
        
        let gx = ((lon / (2.0 * std::f64::consts::PI) + 0.5) * nx as f64) as usize % nx;
        let gy = ((lat / std::f64::consts::PI + 0.5) * ny as f64).clamp(0.0, (ny - 1) as f64) as usize;
        let gz = (alt_fraction * nz as f64).clamp(0.0, (nz - 1) as f64) as usize;
        
        self.wind[self.index(gx, gy, gz)]
    }
}

/// 3D grid for ocean simulation
#[derive(Clone)]
pub struct OceanGrid {
    pub resolution: (usize, usize, usize), // (lon, lat, depth)
    /// Temperature (K)
    pub temperature: Vec<f64>,
    /// Salinity (ppt)
    pub salinity: Vec<f64>,
    /// Current velocity (m/s)
    pub current: Vec<Vec3>,
}

impl OceanGrid {
    pub fn new(nx: usize, ny: usize, nz: usize) -> Self {
        let size = nx * ny * nz;
        Self {
            resolution: (nx, ny, nz),
            temperature: vec![285.0; size], // ~12°C average
            salinity: vec![35.0; size],     // 35 ppt average
            current: vec![Vec3::ZERO; size],
        }
    }
}

/// Climate simulation settings
#[derive(Clone, Debug)]
pub struct ClimateSettings {
    /// Time step for simulation (seconds)
    pub dt: f64,
    /// Enable Coriolis effect
    pub coriolis_enabled: bool,
    /// Enable ocean currents
    pub ocean_enabled: bool,
    /// Solar constant (W/m²)
    pub solar_constant: f64,
    /// Albedo (global average reflectivity)
    pub albedo: f64,
}

impl Default for ClimateSettings {
    fn default() -> Self {
        Self {
            dt: 3600.0, // 1 hour
            coriolis_enabled: true,
            ocean_enabled: false, // Disabled until ocean systems are implemented
            solar_constant: 1361.0, // Earth average
            albedo: 0.3,
        }
    }
}

/// Update climate simulation
fn update_climate(
    mut state: ResMut<ClimateState>,
    config: Res<AtmosphereConfig>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // Update day of year
    let days_per_step = dt / 86400.0;
    state.day_of_year = (state.day_of_year + days_per_step) % 1.0;
    state.season = state.day_of_year; // Simplified
    
    // Update atmospheric circulation
    let day = state.day_of_year;
    update_circulation(&mut state.atmosphere, &config, day);
    
    // Update ocean currents
    if state.settings.ocean_enabled {
        let atm_copy = state.atmosphere.clone();
        update_ocean_currents(&mut state.ocean, &atm_copy);
    }
}

fn update_circulation(
    atmosphere: &mut AtmosphereGrid,
    config: &AtmosphereConfig,
    day_of_year: f64,
) {
    let (nx, ny, nz) = atmosphere.resolution;
    
    // Calculate solar heating based on latitude and season
    for y in 0..ny {
        let lat = ((y as f64 / ny as f64) - 0.5) * std::f64::consts::PI;
        
        // Solar angle affected by axial tilt and time of year
        let declination = config.axial_tilt * (day_of_year * 2.0 * std::f64::consts::PI).sin();
        let solar_angle = (lat - declination).cos().max(0.0);
        
        // Surface heating
        let surface_temp = config.surface_temperature * (0.8 + 0.4 * solar_angle);
        
        for x in 0..nx {
            // Altitude-dependent temperature
            for z in 0..nz {
                let alt_fraction = z as f64 / nz as f64;
                let altitude = alt_fraction * config.scale_height * 3.0; // Up to 3 scale heights
                
                let idx = atmosphere.index(x, y, z);
                
                // Temperature decreases with altitude
                atmosphere.temperature[idx] = surface_temp - altitude * config.lapse_rate;
                
                // Pressure decreases exponentially
                atmosphere.pressure[idx] = config.surface_pressure * (-altitude / config.scale_height).exp();
            }
            
            // Surface wind from Hadley circulation
            let idx_surface = atmosphere.index(x, y, 0);
            
            // Simplified global circulation
            let abs_lat = lat.abs();
            if abs_lat < 0.52 { // 0-30 degrees: Hadley cell
                // Trade winds (easterly)
                atmosphere.wind[idx_surface] = Vec3::new(-5.0, 0.0, lat.signum() as f32 * -2.0);
            } else if abs_lat < 1.05 { // 30-60 degrees: Ferrel cell
                // Westerlies
                atmosphere.wind[idx_surface] = Vec3::new(8.0, 0.0, lat.signum() as f32 * 2.0);
            } else { // 60-90 degrees: Polar cell
                // Polar easterlies
                atmosphere.wind[idx_surface] = Vec3::new(-3.0, 0.0, lat.signum() as f32 * -1.0);
            }
        }
    }
}

fn update_ocean_currents(ocean: &mut OceanGrid, atmosphere: &AtmosphereGrid) {
    let (nx, ny, nz) = ocean.resolution;
    
    // Wind-driven surface currents
    for y in 0..ny {
        for x in 0..nx {
            let idx_surface = y * nx + x;
            
            // Get wind at this location
            let lat = ((y as f64 / ny as f64) - 0.5) * std::f64::consts::PI;
            let lon = (x as f64 / nx as f64) * 2.0 * std::f64::consts::PI;
            let wind = atmosphere.sample_wind(lon, lat, 0.0);
            
            // Surface current is ~3% of wind speed, rotated by Ekman spiral
            let current_speed = wind.length() * 0.03;
            let ekman_angle = std::f32::consts::PI / 4.0 * lat.signum() as f32;
            let current_dir = Vec3::new(
                wind.x * ekman_angle.cos() - wind.z * ekman_angle.sin(),
                0.0,
                wind.x * ekman_angle.sin() + wind.z * ekman_angle.cos(),
            ).normalize_or_zero();
            
            ocean.current[idx_surface] = current_dir * current_speed;
        }
    }
}

/// Weather event types
#[derive(Clone, Debug)]
pub enum WeatherEvent {
    Rain {
        location: Vec3,
        intensity: f64, // mm/hour
        duration: f64,  // hours
    },
    Snow {
        location: Vec3,
        intensity: f64,
        duration: f64,
    },
    Storm {
        location: Vec3,
        intensity: f64, // Wind speed m/s
        category: u8,   // 1-5 for hurricanes
    },
    Drought {
        region: Vec3,
        severity: f64,
    },
}

/// Generate weather events based on climate state
pub fn generate_weather_events(
    climate: &ClimateState,
    time_span: f64, // hours
) -> Vec<WeatherEvent> {
    let mut events = Vec::new();
    let mut seed: u64 = 54321;

    let (nx, ny, _) = climate.atmosphere.resolution;

    for y in 0..ny {
        for x in 0..nx {
            let idx = y * nx + x;
            let humidity = climate.atmosphere.humidity[idx];
            let temp = climate.atmosphere.temperature[idx];

            // Rain probability based on humidity and temperature
            let rain_prob = humidity.powi(2) * 0.01 * time_span;

            if rand_float(&mut seed) < rain_prob {
                let lat = ((y as f64 / ny as f64) - 0.5) * std::f64::consts::PI;
                let lon = (x as f64 / nx as f64) * 2.0 * std::f64::consts::PI;

                let location = Vec3::new(
                    lon.cos() as f32 * lat.cos() as f32,
                    lat.sin() as f32,
                    lon.sin() as f32 * lat.cos() as f32,
                );

                if temp > 273.0 {
                    events.push(WeatherEvent::Rain {
                        location,
                        intensity: rand_float(&mut seed) * 20.0 + 1.0,
                        duration: rand_float(&mut seed) * 12.0 + 1.0,
                    });
                } else {
                    events.push(WeatherEvent::Snow {
                        location,
                        intensity: rand_float(&mut seed) * 10.0 + 0.5,
                        duration: rand_float(&mut seed) * 24.0 + 2.0,
                    });
                }
            }
        }
    }

    events
}

/// Simple random float (0.0 - 1.0) using a mutable seed
fn rand_float(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    (*seed as f64 / u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_atmosphere_sampling() {
        let atm = AtmosphereGrid::new(36, 18, 5);
        let temp = atm.sample_temperature(0.0, 0.0, 0.0);
        assert!(temp > 0.0);
    }
    
    #[test]
    fn test_earth_like_composition() {
        let comp = AtmosphereComposition::earth_like();
        let total = comp.nitrogen + comp.oxygen + comp.carbon_dioxide + 
                    comp.water_vapor + comp.argon + comp.other;
        assert!((total - 1.0).abs() < 0.01);
    }
}

