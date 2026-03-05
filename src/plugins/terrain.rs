use bevy::prelude::*;
use noise::{NoiseFn, Fbm, Perlin, RidgedMulti, MultiFractal, Seedable};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, _app: &mut App) {
        // No systems yet, mostly a library for planet generation
    }
}

/// Scale thresholds for different detail layers (in meters of observer scale)
pub mod scale {
    pub const PLANETARY: f32 = 100000.0;    // 100+ km - continental shapes
    pub const CONTINENTAL: f32 = 10000.0;   // 10-100 km - mountain ranges
    pub const REGIONAL: f32 = 1000.0;       // 1-10 km - individual mountains, valleys
    pub const LOCAL: f32 = 100.0;           // 100m-1km - hills, cliffs
    pub const DETAIL: f32 = 10.0;           // 10-100m - boulders, rock formations
    pub const FINE: f32 = 1.0;              // 1-10m - rocks, large vegetation
    pub const MICRO: f32 = 0.1;             // 10cm-1m - pebbles, grass clumps
    pub const ANT: f32 = 0.01;              // 1-10cm - soil texture, individual pebbles
}

/// Infinite-scale terrain noise system
/// Adds progressively more detail as observer gets closer
#[derive(Resource)]
pub struct TerrainNoise {
    seed: u32,
    // Base terrain layers (always visible)
    pub continent_noise: Fbm<Perlin>,
    pub mountain_noise: RidgedMulti<Perlin>,
    pub mountain_range_noise: Fbm<Perlin>,
    pub river_noise: Fbm<Perlin>,
    pub moisture_noise: Fbm<Perlin>,
    pub warp_noise: Fbm<Perlin>,
    
    // Detail layers for close-up viewing
    pub hill_noise: Fbm<Perlin>,           // Regional hills and undulation
    pub cliff_noise: RidgedMulti<Perlin>,  // Cliff faces and rock outcrops
    pub boulder_noise: Fbm<Perlin>,        // Boulder fields
    pub rock_noise: Fbm<Perlin>,           // Individual rocks
    pub pebble_noise: Fbm<Perlin>,         // Pebble-scale detail
    pub micro_noise: Fbm<Perlin>,          // Ant-scale soil texture
}

impl TerrainNoise {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            // Continental scale - large landmasses, ocean basins
            continent_noise: Fbm::new(seed)
                .set_frequency(0.5)
                .set_octaves(6)
                .set_persistence(0.5)
                .set_lacunarity(2.0),
            
            // Mountain peaks - ridged for sharp peaks
            mountain_noise: RidgedMulti::new(seed + 1)
                .set_frequency(2.0)
                .set_octaves(10)
                .set_lacunarity(2.2),
            
            // Mountain range placement - where ranges form
            mountain_range_noise: Fbm::new(seed + 10)
                .set_frequency(0.25)
                .set_octaves(4)
                .set_persistence(0.6),
            
            // Rivers and valleys
            river_noise: Fbm::new(seed + 2)
                .set_frequency(1.0)
                .set_octaves(4),
            
            // Climate/biome
            moisture_noise: Fbm::new(seed + 3)
                .set_frequency(0.6)
                .set_octaves(5),
            
            // Domain warping for natural shapes
            warp_noise: Fbm::new(seed + 4)
                .set_frequency(1.2)
                .set_octaves(4),
            
            // === DETAIL LAYERS (revealed as you zoom in) ===
            
            // Hills and terrain undulation (visible from ~1km)
            hill_noise: Fbm::new(seed + 100)
                .set_frequency(8.0)
                .set_octaves(6)
                .set_persistence(0.55),
            
            // Cliff faces and rock outcrops (visible from ~100m)
            cliff_noise: RidgedMulti::new(seed + 101)
                .set_frequency(25.0)
                .set_octaves(8)
                .set_lacunarity(2.1),
            
            // Boulder fields (visible from ~50m)
            boulder_noise: Fbm::new(seed + 102)
                .set_frequency(80.0)
                .set_octaves(6)
                .set_persistence(0.6),
            
            // Individual rocks (visible from ~10m)
            rock_noise: Fbm::new(seed + 103)
                .set_frequency(200.0)
                .set_octaves(5)
                .set_persistence(0.65),
            
            // Pebbles and small stones (visible from ~1m)
            pebble_noise: Fbm::new(seed + 104)
                .set_frequency(500.0)
                .set_octaves(4)
                .set_persistence(0.7),
            
            // Micro detail - soil, sand grains (visible from ~10cm)
            micro_noise: Fbm::new(seed + 105)
                .set_frequency(2000.0)
                .set_octaves(4)
                .set_persistence(0.75),
        }
    }
    
    /// Get elevation with scale-dependent detail
    /// observer_scale: approximate meters per pixel at current zoom level
    pub fn get_elevation_at_scale(&self, point: Vec3, observer_scale: f32) -> f32 {
        let p = [point.x as f64, point.y as f64, point.z as f64];
        
        // Domain warping for organic shapes
        let warp_strength = 0.06;
        let q = self.apply_domain_warp(p, warp_strength);
        
        // === LAYER 1: Continental (always visible) ===
        let continent_val = self.continent_noise.get(q) as f32;
        
        // === LAYER 2: Mountain Ranges ===
        let range_mask = self.mountain_range_noise.get(q) as f32;
        let range_normalized = (range_mask + 1.0) * 0.5;
        let mountain_range_threshold = 0.35;
        let range_strength = if range_normalized > mountain_range_threshold {
            let t = (range_normalized - mountain_range_threshold) / (1.0 - mountain_range_threshold);
            t * t
        } else {
            0.0
        };
        
        // === LAYER 3: Mountain Peaks ===
        let mountain_base = self.mountain_noise.get(q) as f32;
        let height_multiplier = 1.0 + (range_normalized - mountain_range_threshold).max(0.0) * 2.5;
        let mountain_val = if continent_val > -0.15 && range_strength > 0.0 {
            mountain_base.max(0.0).powf(1.15) * 3.0 * range_strength * height_multiplier
        } else {
            0.0
        };
        
        // === LAYER 4: Rivers ===
        let river_base = self.river_noise.get(q) as f32;
        let river_val = river_base.abs();
        let river_depth = if river_val < 0.04 && continent_val > 0.0 {
            let t = river_val / 0.04;
            (1.0 - t * t) * 0.1
        } else {
            0.0
        };
        
        // Base elevation
        let mut elevation = continent_val * 0.45 + mountain_val * 0.45 - river_depth;
        
        // === SCALE-DEPENDENT DETAIL LAYERS ===
        // These add progressively more detail as you zoom in
        
        // Hills (visible when < 5km observer scale)
        if observer_scale < scale::CONTINENTAL * 0.5 {
            let blend = smoothstep(scale::CONTINENTAL * 0.5, scale::REGIONAL, observer_scale);
            let hill_val = self.hill_noise.get(q) as f32;
            elevation += hill_val * 0.025 * blend;
        }
        
        // Cliff detail (visible when < 500m)
        if observer_scale < scale::LOCAL * 5.0 {
            let blend = smoothstep(scale::LOCAL * 5.0, scale::LOCAL, observer_scale);
            let cliff_val = self.cliff_noise.get(q) as f32;
            // Cliffs more prominent on steep terrain (use elevation gradient as proxy)
            let steepness_factor = (mountain_val * 2.0).min(1.0);
            elevation += cliff_val * 0.012 * blend * (0.3 + steepness_factor * 0.7);
        }
        
        // Boulder detail (visible when < 100m)
        if observer_scale < scale::LOCAL {
            let blend = smoothstep(scale::LOCAL, scale::DETAIL, observer_scale);
            let boulder_val = self.boulder_noise.get(q) as f32;
            elevation += boulder_val * 0.004 * blend;
        }
        
        // Rock detail (visible when < 20m)
        if observer_scale < scale::DETAIL * 2.0 {
            let blend = smoothstep(scale::DETAIL * 2.0, scale::FINE, observer_scale);
            let rock_val = self.rock_noise.get(q) as f32;
            elevation += rock_val * 0.0015 * blend;
        }
        
        // Pebble detail (visible when < 2m)
        if observer_scale < scale::FINE * 2.0 {
            let blend = smoothstep(scale::FINE * 2.0, scale::MICRO, observer_scale);
            let pebble_val = self.pebble_noise.get(q) as f32;
            elevation += pebble_val * 0.0004 * blend;
        }
        
        // Micro/ant-scale detail (visible when < 20cm)
        if observer_scale < scale::MICRO * 2.0 {
            let blend = smoothstep(scale::MICRO * 2.0, scale::ANT, observer_scale);
            let micro_val = self.micro_noise.get(q) as f32;
            elevation += micro_val * 0.0001 * blend;
        }
        
        // Clamp ocean floor
        elevation.max(-0.25)
    }
    
    fn apply_domain_warp(&self, p: [f64; 3], strength: f64) -> [f64; 3] {
        [
            p[0] + self.warp_noise.get([p[0], p[1], p[2]]) * strength,
            p[1] + self.warp_noise.get([p[0] + 5.2, p[1] + 1.3, p[2] + 2.8]) * strength,
            p[2] + self.warp_noise.get([p[0] + 1.7, p[1] + 9.2, p[2] + 5.2]) * strength,
        ]
    }
    
    /// Legacy elevation function - uses default scale (from orbit)
    pub fn get_elevation(&self, point: Vec3) -> f32 {
        // Default to orbital view scale (1km per pixel equivalent)
        self.get_elevation_at_scale(point, scale::REGIONAL)
    }

    /// Get terrain data (elevation + color) - legacy version
    pub fn get_data(&self, point: Vec3) -> (f32, Color) {
        self.get_data_at_scale(point, scale::REGIONAL)
    }
    
    /// Get terrain data with scale-dependent detail
    pub fn get_data_at_scale(&self, point: Vec3, observer_scale: f32) -> (f32, Color) {
        let p = [point.x as f64, point.y as f64, point.z as f64];
        
        let elevation = self.get_elevation_at_scale(point, observer_scale);

        // Domain warp for consistency
        let warp_strength = 0.06;
        let q = self.apply_domain_warp(p, warp_strength);

        let continent_val = self.continent_noise.get(q) as f32;
        let river_base = self.river_noise.get(q) as f32;
        let river_val = river_base.abs();
        let is_river = river_val < 0.035 && continent_val > 0.0;

        // Latitude with noise for natural variation
        let lat_noise = self.moisture_noise.get([p[0] * 2.0, p[1] * 2.0, p[2] * 2.0]) as f32 * 0.12;
        let latitude = (point.y.abs() + lat_noise).clamp(0.0, 1.0); 
        
        let moisture = self.moisture_noise.get(q) as f32;

        // Get base biome color
        let mut color = calculate_biome_color(elevation, latitude, moisture, is_river);
        
        // Add scale-dependent color variation for close-up detail
        if observer_scale < scale::DETAIL {
            // Add subtle color variation from rock/pebble noise
            let detail_variation = self.rock_noise.get(q) as f32 * 0.08;
            color = add_color_variation(color, detail_variation);
        }
        
        if observer_scale < scale::FINE {
            // Even finer color detail
            let micro_variation = self.pebble_noise.get(q) as f32 * 0.04;
            color = add_color_variation(color, micro_variation);
        }

        (elevation, color)
    }
}

/// Smooth interpolation between edge0 and edge1
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Add subtle variation to a color
fn add_color_variation(color: Color, variation: f32) -> Color {
    match color {
        Color::LinearRgba(c) => Color::LinearRgba(LinearRgba {
            red: (c.red + variation).clamp(0.0, 1.0),
            green: (c.green + variation).clamp(0.0, 1.0),
            blue: (c.blue + variation * 0.5).clamp(0.0, 1.0),
            alpha: c.alpha,
        }),
        _ => color,
    }
}

fn mix_colors(c1: LinearRgba, c2: LinearRgba, t: f32) -> LinearRgba {
    let t = t.clamp(0.0, 1.0);
    LinearRgba {
        red: c1.red + (c2.red - c1.red) * t,
        green: c1.green + (c2.green - c1.green) * t,
        blue: c1.blue + (c2.blue - c1.blue) * t,
        alpha: c1.alpha + (c2.alpha - c1.alpha) * t,
    }
}

fn calculate_biome_color(elevation: f32, latitude: f32, moisture: f32, is_river: bool) -> Color {
    // Colors (LinearRgba for mixing)
    let c_deep_ocean = LinearRgba::new(0.02, 0.05, 0.2, 1.0);
    let c_ocean = LinearRgba::new(0.0, 0.2, 0.6, 1.0);
    let c_beach = LinearRgba::new(0.76, 0.70, 0.50, 1.0);
    
    let c_snow = LinearRgba::new(0.95, 0.95, 1.0, 1.0);
    let c_tundra = LinearRgba::new(0.6, 0.6, 0.5, 1.0);
    let c_bare = LinearRgba::new(0.5, 0.5, 0.4, 1.0);
    let _c_scorched = LinearRgba::new(0.3, 0.2, 0.1, 1.0);
    
    let c_taiga = LinearRgba::new(0.2, 0.4, 0.3, 1.0);
    let c_shrubland = LinearRgba::new(0.5, 0.6, 0.3, 1.0);
    let c_grassland = LinearRgba::new(0.3, 0.6, 0.2, 1.0);
    
    let c_rainforest = LinearRgba::new(0.05, 0.3, 0.05, 1.0);
    let c_forest = LinearRgba::new(0.1, 0.4, 0.1, 1.0);
    let c_desert = LinearRgba::new(0.8, 0.7, 0.4, 1.0);

    let c_river = LinearRgba::new(0.1, 0.35, 0.8, 1.0);

    // 1. Ocean / Land blending
    if elevation < 0.0 {
        // Ocean Gradient
        let t = (elevation + 0.2) / 0.2; // 0 at deep, 1 at surface
        let final_c = mix_colors(c_deep_ocean, c_ocean, t);
        return Color::LinearRgba(final_c);
    }

    if is_river {
        return Color::LinearRgba(c_river);
    }

    // Base Land Color based on Moisture/Latitude
    // Normalize inputs
    // Latitude: 0 (Equator) to 1 (Pole)
    // Moisture: -1 (Dry) to 1 (Wet). Remap to 0..1
    let m = (moisture + 1.0) * 0.5;
    let l = latitude;

    // Determine target biome colors to mix
    // We can define a "temperature" roughly inverse to latitude
    let temp = 1.0 - l; // 1.0 (Hot/Equator) -> 0.0 (Cold/Pole)

    // Biome mixing logic
    let mut land_color;
    
    if temp < 0.2 { // Polar / Alpine
        let t = temp / 0.2;
        land_color = mix_colors(c_snow, c_tundra, t);
    } else if temp < 0.4 { // Sub-polar / Boreal
        let t = (temp - 0.2) / 0.2;
        if m < 0.3 {
            land_color = mix_colors(c_bare, c_taiga, t);
        } else {
            // Actually simpler:
            land_color = mix_colors(c_tundra, c_taiga, t);
        }
    } else if temp < 0.7 { // Temperate
        let t = (temp - 0.4) / 0.3;
        if m < 0.2 {
            land_color = mix_colors(c_desert, c_grassland, m / 0.2);
        } else if m < 0.6 {
            land_color = mix_colors(c_grassland, c_forest, (m - 0.2) / 0.4);
        } else {
            land_color = mix_colors(c_forest, c_rainforest, (m - 0.6) / 0.4);
        }
        // Blend with colder prev band
        let prev = mix_colors(c_taiga, c_shrubland, m);
        land_color = mix_colors(prev, land_color, t);

    } else { // Tropical / Equatorial
        let _t = (temp - 0.7) / 0.3;
        if m < 0.2 {
            land_color = mix_colors(c_desert, c_shrubland, m / 0.2);
        } else if m < 0.5 {
            land_color = mix_colors(c_shrubland, c_forest, (m - 0.2) / 0.3);
        } else {
            land_color = mix_colors(c_forest, c_rainforest, (m - 0.5) / 0.5);
        }
    }

    // Beach transition
    let beach_threshold = 0.02;
    if elevation < beach_threshold {
        let t = elevation / beach_threshold;
        let final_c = mix_colors(c_beach, land_color, t);
        return Color::LinearRgba(final_c);
    }
    
    // Mountain Snow peaks (Independent of latitude, based on height)
    if elevation > 0.25 { // High peaks
        let t = (elevation - 0.25) / 0.15;
        let final_c = mix_colors(land_color, c_snow, t);
        return Color::LinearRgba(final_c);
    }

    Color::LinearRgba(land_color)
}
