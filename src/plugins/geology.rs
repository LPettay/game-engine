// Geological Simulation Module
// Infinite Universe Engine - Plate tectonics, erosion, and geological history
//
// This module simulates geological processes that shape planetary surfaces:
// - Plate tectonics (mantle convection, continental drift)
// - Erosion (hydraulic, thermal, wind, chemical)
// - Volcanic activity and mountain building
// - Sediment deposition and rock formation
//
// Used during pre-bake to generate realistic planetary history

use bevy::prelude::*;
use std::collections::HashMap;

pub struct GeologyPlugin;

impl Plugin for GeologyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TectonicsState>()
           .init_resource::<ErosionState>()
           .add_systems(Update, (
               update_plate_tectonics,
               update_erosion,
           ).chain().run_if(in_state(crate::GameState::Playing)));
    }
}

// ============================================================================
// PLATE TECTONICS
// ============================================================================

/// Unique identifier for a tectonic plate
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlateId(pub u32);

/// A tectonic plate
#[derive(Clone, Debug)]
pub struct TectonicPlate {
    pub id: PlateId,
    /// Plate type
    pub plate_type: PlateType,
    /// Center of mass on unit sphere
    pub center: Vec3,
    /// Angular velocity (rotation axis * angular speed in rad/s)
    pub angular_velocity: Vec3,
    /// Density (kg/m³) - affects subduction
    pub density: f64,
    /// Average thickness (m)
    pub thickness: f64,
    /// Age in simulation years
    pub age: f64,
    /// Cells belonging to this plate
    pub cells: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlateType {
    Continental, // Lighter, thicker, doesn't subduct easily
    Oceanic,     // Denser, thinner, can subduct
}

/// State of the tectonic simulation
#[derive(Resource)]
pub struct TectonicsState {
    /// All tectonic plates
    pub plates: HashMap<PlateId, TectonicPlate>,
    /// Grid of mantle convection cells (velocity field)
    pub mantle_convection: MantleConvection,
    /// Plate boundaries and their types
    pub boundaries: Vec<PlateBoundary>,
    /// Simulation settings
    pub settings: TectonicsSettings,
    /// Current simulation time (years)
    pub sim_time: f64,
}

impl Default for TectonicsState {
    fn default() -> Self {
        Self {
            plates: HashMap::new(),
            mantle_convection: MantleConvection::default(),
            boundaries: Vec::new(),
            settings: TectonicsSettings::default(),
            sim_time: 0.0,
        }
    }
}

/// Mantle convection drives plate movement
#[derive(Clone)]
pub struct MantleConvection {
    /// Grid resolution (longitude, latitude)
    pub resolution: (usize, usize),
    /// Velocity field (upwelling positive, downwelling negative)
    pub velocity: Vec<Vec3>,
    /// Temperature field (K)
    pub temperature: Vec<f64>,
}

impl Default for MantleConvection {
    fn default() -> Self {
        let res = (36, 18); // 10-degree cells
        let size = res.0 * res.1;
        Self {
            resolution: res,
            velocity: vec![Vec3::ZERO; size],
            temperature: vec![4000.0; size], // Average mantle temperature
        }
    }
}

/// Types of plate boundaries
#[derive(Clone, Debug)]
pub struct PlateBoundary {
    pub plate1: PlateId,
    pub plate2: PlateId,
    pub boundary_type: BoundaryType,
    /// Points along the boundary
    pub points: Vec<Vec3>,
    /// Activity level (affects earthquake/volcano frequency)
    pub activity: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryType {
    /// Plates moving apart (mid-ocean ridges)
    Divergent,
    /// Plates colliding (mountain building, subduction)
    Convergent { subducting: Option<PlateId> },
    /// Plates sliding past each other
    Transform,
}

/// Settings for tectonic simulation
#[derive(Clone, Debug)]
pub struct TectonicsSettings {
    /// Time step in years
    pub dt_years: f64,
    /// Plate speed scale (cm/year)
    pub plate_speed_scale: f64,
    /// Mountain building rate (m/million years)
    pub orogeny_rate: f64,
    /// Subduction rate (km/million years)
    pub subduction_rate: f64,
}

impl Default for TectonicsSettings {
    fn default() -> Self {
        Self {
            dt_years: 1_000_000.0, // 1 million year steps for pre-bake
            plate_speed_scale: 5.0, // ~5 cm/year average
            orogeny_rate: 1000.0, // 1km per million years at active boundaries
            subduction_rate: 50.0, // 50 km depth per million years
        }
    }
}

/// Update plate tectonics (runs during pre-bake)
fn update_plate_tectonics(
    mut state: ResMut<TectonicsState>,
    time: Res<Time>,
) {
    // Skip if no plates defined
    if state.plates.is_empty() {
        return;
    }
    
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // Update mantle convection
    update_mantle_convection(&mut state.mantle_convection, dt);
    
    // Move plates based on mantle convection
    let dt_years = state.settings.dt_years;
    let mantle_copy = state.mantle_convection.clone();
    for (_, plate) in state.plates.iter_mut() {
        // Calculate net force from mantle convection
        // Plates are driven by convection currents beneath them
        
        // Simple model: plate velocity proportional to mantle velocity at plate center
        let mantle_v = sample_mantle_velocity(&mantle_copy, plate.center);
        plate.angular_velocity = mantle_v * 0.001; // Scale to plate motion
        
        // Update plate age
        plate.age += dt * dt_years;
    }
    
    // Update boundaries
    update_plate_boundaries(&mut state);
    
    state.sim_time += dt * state.settings.dt_years;
}

fn update_mantle_convection(mantle: &mut MantleConvection, _dt: f64) {
    // Simplified convection model
    // In reality, this would solve the heat equation with convection
    
    let (nx, ny) = mantle.resolution;
    
    for y in 0..ny {
        for x in 0..nx {
            let idx = y * nx + x;
            
            // Create convection cells (Rayleigh-Bénard-like)
            let lon = (x as f64 / nx as f64) * 2.0 * std::f64::consts::PI;
            let lat = ((y as f64 / ny as f64) - 0.5) * std::f64::consts::PI;
            
            // Simple pattern: alternating upwelling and downwelling
            let upwelling = (lon * 3.0).sin() * (lat * 2.0).cos();
            
            mantle.velocity[idx] = Vec3::new(
                (lon * 3.0).cos() as f32 * 0.1,
                upwelling as f32 * 0.1,
                (lat * 2.0).sin() as f32 * 0.1,
            );
        }
    }
}

fn sample_mantle_velocity(mantle: &MantleConvection, pos: Vec3) -> Vec3 {
    // Convert 3D position to spherical coordinates
    let r = pos.length();
    if r < 0.001 {
        return Vec3::ZERO;
    }
    
    let pos_norm = pos / r;
    let lat = pos_norm.y.asin();
    let lon = pos_norm.z.atan2(pos_norm.x);
    
    // Convert to grid coordinates
    let (nx, ny) = mantle.resolution;
    let gx = ((lon / (2.0 * std::f32::consts::PI) + 0.5) * nx as f32) as usize % nx;
    let gy = ((lat / std::f32::consts::PI + 0.5) * ny as f32).clamp(0.0, (ny - 1) as f32) as usize;
    
    let idx = gy * nx + gx;
    mantle.velocity.get(idx).copied().unwrap_or(Vec3::ZERO)
}

fn update_plate_boundaries(state: &mut TectonicsState) {
    // Detect and update boundaries between plates
    // This would analyze plate velocities to determine boundary types
    
    for boundary in state.boundaries.iter_mut() {
        let plate1 = state.plates.get(&boundary.plate1);
        let plate2 = state.plates.get(&boundary.plate2);
        
        if let (Some(p1), Some(p2)) = (plate1, plate2) {
            // Calculate relative velocity at boundary
            let rel_velocity = p1.angular_velocity - p2.angular_velocity;
            let speed = rel_velocity.length();
            
            // Update activity based on relative motion
            boundary.activity = speed as f64 * 100.0; // Arbitrary scaling
        }
    }
}

// ============================================================================
// EROSION SYSTEMS
// ============================================================================

/// State of erosion simulation
#[derive(Resource, Default)]
pub struct ErosionState {
    pub settings: ErosionSettings,
    pub total_eroded: f64,
    pub total_deposited: f64,
}

/// Erosion simulation settings
#[derive(Clone, Debug)]
pub struct ErosionSettings {
    /// Hydraulic erosion rate
    pub hydraulic_rate: f64,
    /// Thermal erosion rate
    pub thermal_rate: f64,
    /// Wind erosion rate
    pub wind_rate: f64,
    /// Chemical weathering rate
    pub chemical_rate: f64,
    /// Talus angle (maximum stable slope in radians)
    pub talus_angle: f64,
    /// Sediment capacity of water
    pub sediment_capacity: f64,
    /// Evaporation rate
    pub evaporation_rate: f64,
}

impl Default for ErosionSettings {
    fn default() -> Self {
        Self {
            hydraulic_rate: 0.01,
            thermal_rate: 0.005,
            wind_rate: 0.002,
            chemical_rate: 0.001,
            talus_angle: 0.7, // ~40 degrees
            sediment_capacity: 1.0,
            evaporation_rate: 0.05,
        }
    }
}

/// Update erosion systems
fn update_erosion(
    mut state: ResMut<ErosionState>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // Erosion would be applied to terrain heightmaps
    // This is a placeholder for the actual implementation
}

/// Apply hydraulic erosion to a heightmap
/// Uses particle-based simulation for realistic river carving
pub fn apply_hydraulic_erosion(
    heightmap: &mut [f64],
    width: usize,
    height: usize,
    settings: &ErosionSettings,
    iterations: usize,
) {
    let mut seed: u64 = 12345;
    for _ in 0..iterations {
        // Spawn water droplets at random positions
        for _ in 0..1000 {
            let x = (rand_float(&mut seed) * width as f64) as usize;
            let y = (rand_float(&mut seed) * height as f64) as usize;

            simulate_droplet(heightmap, width, height, x, y, settings, &mut seed);
        }
    }
}

fn simulate_droplet(
    heightmap: &mut [f64],
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
    settings: &ErosionSettings,
    seed: &mut u64,
) {
    let mut x = start_x as f64;
    let mut y = start_y as f64;
    let mut sediment = 0.0;
    let mut water = 1.0;
    let mut velocity = 0.0;
    
    let max_steps = 100;
    
    for _ in 0..max_steps {
        let ix = x as usize;
        let iy = y as usize;
        
        if ix >= width - 1 || iy >= height - 1 {
            break;
        }
        
        // Calculate gradient
        let idx = iy * width + ix;
        let h = heightmap[idx];
        let h_right = heightmap[idx + 1];
        let h_down = heightmap[(iy + 1) * width + ix];
        
        let gx = h_right - h;
        let gy = h_down - h;
        let gradient = (gx * gx + gy * gy).sqrt();
        
        // Move downhill
        if gradient > 0.0001 {
            x -= gx / gradient;
            y -= gy / gradient;
        } else {
            // Random walk in flat areas
            x += rand_float(seed) * 2.0 - 1.0;
            y += rand_float(seed) * 2.0 - 1.0;
        }
        
        // Update velocity
        velocity = (velocity * velocity + gradient).sqrt();
        
        // Calculate sediment capacity
        let capacity = velocity * water * settings.sediment_capacity;
        
        // Erode or deposit
        if sediment < capacity {
            // Erode
            let erode_amount = (capacity - sediment).min(settings.hydraulic_rate) * velocity;
            heightmap[idx] -= erode_amount;
            sediment += erode_amount;
        } else {
            // Deposit
            let deposit_amount = (sediment - capacity) * 0.1;
            heightmap[idx] += deposit_amount;
            sediment -= deposit_amount;
        }
        
        // Evaporate water
        water *= 1.0 - settings.evaporation_rate;
        
        if water < 0.001 {
            break;
        }
    }
}

/// Apply thermal erosion (freeze-thaw cycles)
pub fn apply_thermal_erosion(
    heightmap: &mut [f64],
    width: usize,
    height: usize,
    settings: &ErosionSettings,
    iterations: usize,
) {
    let mut talus = vec![0.0; width * height];
    
    for _ in 0..iterations {
        // Calculate slopes and move material
        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let idx = y * width + x;
                let h = heightmap[idx];
                
                // Check all 8 neighbors
                let neighbors = [
                    ((x as i32 - 1) as usize, y),
                    ((x as i32 + 1) as usize, y),
                    (x, (y as i32 - 1) as usize),
                    (x, (y as i32 + 1) as usize),
                ];
                
                for (nx, ny) in neighbors {
                    if nx >= width || ny >= height {
                        continue;
                    }
                    
                    let nidx = ny * width + nx;
                    let nh = heightmap[nidx];
                    let slope = (h - nh).abs();
                    
                    // If slope exceeds talus angle, move material
                    if slope > settings.talus_angle {
                        let amount = (slope - settings.talus_angle) * settings.thermal_rate;
                        if h > nh {
                            talus[idx] -= amount;
                            talus[nidx] += amount;
                        }
                    }
                }
            }
        }
        
        // Apply talus changes
        for i in 0..heightmap.len() {
            heightmap[i] += talus[i];
            talus[i] = 0.0;
        }
    }
}

/// Simple random float (0.0 - 1.0) using a mutable seed
fn rand_float(seed: &mut u64) -> f64 {
    // Simple LCG for deterministic results
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    (*seed as f64 / u64::MAX as f64)
}

/// Geological events that can occur
#[derive(Clone, Debug)]
pub enum GeologicalEvent {
    Earthquake {
        epicenter: Vec3,
        magnitude: f64, // Richter scale
        depth: f64,     // km
    },
    VolcanicEruption {
        location: Vec3,
        intensity: f64, // VEI scale (0-8)
        volume: f64,    // km³ of ejecta
    },
    MountainBuilding {
        location: Vec3,
        rate: f64, // m/million years
    },
    RiftFormation {
        start: Vec3,
        end: Vec3,
        spreading_rate: f64, // cm/year
    },
}

/// Generate geological events based on tectonic state
pub fn generate_geological_events(
    tectonics: &TectonicsState,
    time_span: f64, // years
) -> Vec<GeologicalEvent> {
    let mut events = Vec::new();
    let mut seed: u64 = 12345;

    for boundary in &tectonics.boundaries {
        // Probability of events based on boundary activity
        let quake_prob = boundary.activity * 0.001 * time_span;

        if rand_float(&mut seed) < quake_prob {
            if let Some(point) = boundary.points.first() {
                events.push(GeologicalEvent::Earthquake {
                    epicenter: *point,
                    magnitude: 4.0 + rand_float(&mut seed) * 4.0, // 4-8 magnitude
                    depth: 10.0 + rand_float(&mut seed) * 50.0,   // 10-60 km
                });
            }
        }

        // Volcanic activity at subduction zones
        if let BoundaryType::Convergent { subducting: Some(_) } = boundary.boundary_type {
            let volcano_prob = boundary.activity * 0.0001 * time_span;

            if rand_float(&mut seed) < volcano_prob {
                if let Some(point) = boundary.points.first() {
                    events.push(GeologicalEvent::VolcanicEruption {
                        location: *point,
                        intensity: 2.0 + rand_float(&mut seed) * 4.0, // VEI 2-6
                        volume: 0.001 + rand_float(&mut seed) * 1.0,  // 0.001-1 km³
                    });
                }
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_erosion_settings() {
        let settings = ErosionSettings::default();
        assert!(settings.talus_angle > 0.0);
        assert!(settings.hydraulic_rate > 0.0);
    }
    
    #[test]
    fn test_thermal_erosion() {
        let mut heightmap = vec![1.0; 100];
        // Create a spike
        heightmap[55] = 2.0;
        
        apply_thermal_erosion(&mut heightmap, 10, 10, &ErosionSettings::default(), 10);
        
        // Spike should be somewhat reduced
        assert!(heightmap[55] < 2.0);
    }
}

