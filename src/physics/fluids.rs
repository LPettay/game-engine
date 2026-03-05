// Fluid Dynamics Simulation Module
// Simplified fluid simulation for atmosphere, oceans, and weather
//
// Supports:
// - Grid-based fluid simulation (Navier-Stokes simplified)
// - SPH (Smoothed Particle Hydrodynamics) for particle-based fluids
// - Buoyancy and convection
// - Atmospheric circulation patterns

use bevy::prelude::*;
use bevy::math::DVec3;
use super::PhysicalConstants;

/// 3D grid for fluid simulation
#[derive(Clone)]
pub struct FluidGrid {
    /// Grid dimensions
    pub dimensions: (usize, usize, usize),
    /// Cell size in meters
    pub cell_size: f64,
    /// Velocity field (x, y, z components)
    pub velocity: Vec<DVec3>,
    /// Pressure field
    pub pressure: Vec<f64>,
    /// Density field
    pub density: Vec<f64>,
    /// Temperature field
    pub temperature: Vec<f64>,
}

impl FluidGrid {
    pub fn new(nx: usize, ny: usize, nz: usize, cell_size: f64) -> Self {
        let size = nx * ny * nz;
        Self {
            dimensions: (nx, ny, nz),
            cell_size,
            velocity: vec![DVec3::ZERO; size],
            pressure: vec![0.0; size],
            density: vec![1.0; size],
            temperature: vec![300.0; size],
        }
    }
    
    /// Get index from 3D coordinates
    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        let (nx, ny, _) = self.dimensions;
        x + y * nx + z * nx * ny
    }
    
    /// Get 3D coordinates from index
    pub fn coords(&self, idx: usize) -> (usize, usize, usize) {
        let (nx, ny, _) = self.dimensions;
        let x = idx % nx;
        let y = (idx / nx) % ny;
        let z = idx / (nx * ny);
        (x, y, z)
    }
    
    /// Interpolate velocity at an arbitrary position
    pub fn sample_velocity(&self, pos: DVec3) -> DVec3 {
        let (nx, ny, nz) = self.dimensions;
        
        // Convert world position to grid coordinates
        let gx = (pos.x / self.cell_size).clamp(0.0, (nx - 1) as f64);
        let gy = (pos.y / self.cell_size).clamp(0.0, (ny - 1) as f64);
        let gz = (pos.z / self.cell_size).clamp(0.0, (nz - 1) as f64);
        
        // Trilinear interpolation
        let x0 = gx.floor() as usize;
        let y0 = gy.floor() as usize;
        let z0 = gz.floor() as usize;
        let x1 = (x0 + 1).min(nx - 1);
        let y1 = (y0 + 1).min(ny - 1);
        let z1 = (z0 + 1).min(nz - 1);
        
        let fx = gx - x0 as f64;
        let fy = gy - y0 as f64;
        let fz = gz - z0 as f64;
        
        let v000 = self.velocity[self.index(x0, y0, z0)];
        let v001 = self.velocity[self.index(x0, y0, z1)];
        let v010 = self.velocity[self.index(x0, y1, z0)];
        let v011 = self.velocity[self.index(x0, y1, z1)];
        let v100 = self.velocity[self.index(x1, y0, z0)];
        let v101 = self.velocity[self.index(x1, y0, z1)];
        let v110 = self.velocity[self.index(x1, y1, z0)];
        let v111 = self.velocity[self.index(x1, y1, z1)];
        
        let v00 = v000.lerp(v001, fz);
        let v01 = v010.lerp(v011, fz);
        let v10 = v100.lerp(v101, fz);
        let v11 = v110.lerp(v111, fz);
        
        let v0 = v00.lerp(v01, fy);
        let v1 = v10.lerp(v11, fy);
        
        v0.lerp(v1, fx)
    }
}

/// Fluid simulation parameters
#[derive(Clone, Debug)]
pub struct FluidParams {
    /// Kinematic viscosity (m²/s)
    pub viscosity: f64,
    /// Diffusion rate for density/temperature
    pub diffusion: f64,
    /// Time step
    pub dt: f64,
    /// Gravity vector
    pub gravity: DVec3,
    /// Number of pressure solver iterations
    pub pressure_iterations: usize,
}

impl Default for FluidParams {
    fn default() -> Self {
        Self {
            viscosity: 0.0001, // Low viscosity for atmosphere
            diffusion: 0.0001,
            dt: 1.0 / 60.0,
            gravity: DVec3::new(0.0, -9.81, 0.0),
            pressure_iterations: 20,
        }
    }
}

/// Perform one step of fluid simulation
/// Using simplified Navier-Stokes with operator splitting
pub fn step_fluid_simulation(grid: &mut FluidGrid, params: &FluidParams) {
    let (nx, ny, nz) = grid.dimensions;
    let n = nx * ny * nz;
    
    // 1. Add external forces (gravity, buoyancy)
    add_forces(grid, params);
    
    // 2. Advection (move quantities along velocity field)
    advect_velocity(grid, params);
    
    // 3. Diffusion (viscosity)
    if params.viscosity > 0.0 {
        diffuse_velocity(grid, params);
    }
    
    // 4. Pressure projection (make velocity divergence-free)
    project_velocity(grid, params);
}

fn add_forces(grid: &mut FluidGrid, params: &FluidParams) {
    for i in 0..grid.velocity.len() {
        // Add gravity
        grid.velocity[i] += params.gravity * params.dt;
        
        // Add buoyancy (hot air rises)
        let ambient_temp = 300.0;
        let buoyancy = (grid.temperature[i] - ambient_temp) * 0.001;
        grid.velocity[i].y += buoyancy * params.dt;
    }
}

fn advect_velocity(grid: &mut FluidGrid, params: &FluidParams) {
    let (nx, ny, nz) = grid.dimensions;
    let mut new_velocity = vec![DVec3::ZERO; grid.velocity.len()];
    
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let idx = grid.index(x, y, z);
                let pos = DVec3::new(
                    x as f64 * grid.cell_size,
                    y as f64 * grid.cell_size,
                    z as f64 * grid.cell_size,
                );
                
                // Trace back along velocity field
                let back_pos = pos - grid.velocity[idx] * params.dt;
                
                // Sample velocity at back position
                new_velocity[idx] = grid.sample_velocity(back_pos);
            }
        }
    }
    
    grid.velocity = new_velocity;
}

fn diffuse_velocity(grid: &mut FluidGrid, params: &FluidParams) {
    let (nx, ny, nz) = grid.dimensions;
    let a = params.dt * params.viscosity * (nx * ny * nz) as f64;
    
    // Gauss-Seidel relaxation
    for _ in 0..20 {
        for z in 1..(nz - 1) {
            for y in 1..(ny - 1) {
                for x in 1..(nx - 1) {
                    let idx = grid.index(x, y, z);
                    let neighbors = grid.velocity[grid.index(x - 1, y, z)]
                        + grid.velocity[grid.index(x + 1, y, z)]
                        + grid.velocity[grid.index(x, y - 1, z)]
                        + grid.velocity[grid.index(x, y + 1, z)]
                        + grid.velocity[grid.index(x, y, z - 1)]
                        + grid.velocity[grid.index(x, y, z + 1)];
                    
                    grid.velocity[idx] = (grid.velocity[idx] + neighbors * a) / (1.0 + 6.0 * a);
                }
            }
        }
    }
}

fn project_velocity(grid: &mut FluidGrid, params: &FluidParams) {
    let (nx, ny, nz) = grid.dimensions;
    let h = grid.cell_size;
    let n = nx * ny * nz;
    
    let mut divergence = vec![0.0; n];
    let mut pressure = vec![0.0; n];
    
    // Calculate divergence
    for z in 1..(nz - 1) {
        for y in 1..(ny - 1) {
            for x in 1..(nx - 1) {
                let idx = grid.index(x, y, z);
                divergence[idx] = -0.5 * h * (
                    grid.velocity[grid.index(x + 1, y, z)].x - grid.velocity[grid.index(x - 1, y, z)].x
                    + grid.velocity[grid.index(x, y + 1, z)].y - grid.velocity[grid.index(x, y - 1, z)].y
                    + grid.velocity[grid.index(x, y, z + 1)].z - grid.velocity[grid.index(x, y, z - 1)].z
                );
            }
        }
    }
    
    // Solve for pressure (Gauss-Seidel)
    for _ in 0..params.pressure_iterations {
        for z in 1..(nz - 1) {
            for y in 1..(ny - 1) {
                for x in 1..(nx - 1) {
                    let idx = grid.index(x, y, z);
                    pressure[idx] = (divergence[idx]
                        + pressure[grid.index(x - 1, y, z)]
                        + pressure[grid.index(x + 1, y, z)]
                        + pressure[grid.index(x, y - 1, z)]
                        + pressure[grid.index(x, y + 1, z)]
                        + pressure[grid.index(x, y, z - 1)]
                        + pressure[grid.index(x, y, z + 1)]) / 6.0;
                }
            }
        }
    }
    
    // Subtract pressure gradient from velocity
    for z in 1..(nz - 1) {
        for y in 1..(ny - 1) {
            for x in 1..(nx - 1) {
                let idx = grid.index(x, y, z);
                grid.velocity[idx].x -= 0.5 * (pressure[grid.index(x + 1, y, z)] - pressure[grid.index(x - 1, y, z)]) / h;
                grid.velocity[idx].y -= 0.5 * (pressure[grid.index(x, y + 1, z)] - pressure[grid.index(x, y - 1, z)]) / h;
                grid.velocity[idx].z -= 0.5 * (pressure[grid.index(x, y, z + 1)] - pressure[grid.index(x, y, z - 1)]) / h;
            }
        }
    }
    
    grid.pressure = pressure;
}

/// Atmospheric circulation calculations
pub mod atmosphere {
    use super::*;
    
    /// Calculate Coriolis force for a rotating planet
    /// Returns acceleration to add to velocity
    pub fn coriolis_acceleration(
        velocity: DVec3,
        latitude: f64, // radians
        angular_velocity: f64, // rad/s, Earth = 7.29e-5
    ) -> DVec3 {
        // Coriolis acceleration = -2 * Ω × v
        let omega = DVec3::new(
            0.0,
            angular_velocity * latitude.cos(),
            angular_velocity * latitude.sin(),
        );
        
        -2.0 * omega.cross(velocity)
    }
    
    /// Calculate geostrophic wind velocity
    /// Wind that balances pressure gradient and Coriolis force
    pub fn geostrophic_wind(
        pressure_gradient: DVec3, // Pa/m
        density: f64,
        latitude: f64,
        angular_velocity: f64,
    ) -> DVec3 {
        let f = 2.0 * angular_velocity * latitude.sin(); // Coriolis parameter
        if f.abs() < 1e-10 {
            return DVec3::ZERO; // Undefined at equator
        }
        
        // Geostrophic balance: v = (1 / ρf) × ∇p
        DVec3::new(
            -pressure_gradient.y / (density * f),
            pressure_gradient.x / (density * f),
            0.0,
        )
    }
    
    /// Calculate pressure at altitude using barometric formula
    pub fn pressure_at_altitude(
        surface_pressure: f64,
        altitude: f64,
        scale_height: f64,
    ) -> f64 {
        surface_pressure * (-altitude / scale_height).exp()
    }
    
    /// Calculate temperature at altitude (troposphere lapse rate)
    pub fn temperature_at_altitude(
        surface_temp: f64,
        altitude: f64,
        lapse_rate: f64, // K/m, Earth troposphere ~0.0065
    ) -> f64 {
        (surface_temp - lapse_rate * altitude).max(0.0)
    }
}

/// Ocean circulation calculations
pub mod ocean {
    use super::*;
    
    /// Calculate thermohaline circulation density
    /// Based on temperature and salinity
    pub fn seawater_density(
        temperature: f64, // Kelvin
        salinity: f64, // parts per thousand (ppt)
    ) -> f64 {
        // Simplified equation of state for seawater
        let t = temperature - 273.15; // Convert to Celsius
        
        // Reference density at 4°C, 35 ppt
        let rho_0 = 1025.0; // kg/m³
        
        // Temperature effect (density decreases with temperature above 4°C)
        let alpha = 0.00025; // Thermal expansion coefficient
        let t_effect = -alpha * (t - 4.0);
        
        // Salinity effect (density increases with salinity)
        let beta = 0.0008; // Haline contraction coefficient
        let s_effect = beta * (salinity - 35.0);
        
        rho_0 * (1.0 + t_effect + s_effect)
    }
    
    /// Calculate Ekman transport (wind-driven surface current)
    /// Returns mass transport per unit width (kg/(m·s))
    pub fn ekman_transport(
        wind_stress: f64, // Pa
        coriolis_parameter: f64, // 1/s
    ) -> f64 {
        wind_stress / coriolis_parameter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fluid_grid_indexing() {
        let grid = FluidGrid::new(10, 10, 10, 1.0);
        
        assert_eq!(grid.index(0, 0, 0), 0);
        assert_eq!(grid.index(9, 9, 9), 999);
        
        let (x, y, z) = grid.coords(123);
        assert_eq!(grid.index(x, y, z), 123);
    }
    
    #[test]
    fn test_pressure_altitude() {
        let surface_p = 101325.0;
        let scale_h = 8500.0; // Earth's scale height ~8.5 km
        
        let p_5km = atmosphere::pressure_at_altitude(surface_p, 5000.0, scale_h);
        // At 5km, pressure should be about 54% of surface pressure
        assert!((p_5km / surface_p - 0.55).abs() < 0.05);
    }
}

