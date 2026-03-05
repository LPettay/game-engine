// GPU Compute Shader for Terrain Generation
// Infinite Universe Engine - Dynamic Octave Fractal Noise System
// Supports infinite scale rendering with observer-dependent detail levels

// Constants for infinite scale system
const MIN_DETAIL_SCALE: f32 = 0.001;      // Minimum observable detail (1mm)
const MAX_OCTAVES: u32 = 24u;              // Maximum octaves to prevent infinite loops
const LACUNARITY: f32 = 2.0;               // Frequency multiplier per octave
const PERSISTENCE: f32 = 0.5;              // Amplitude multiplier per octave

// Scale layer thresholds (in meters)
const SCALE_CONTINENTAL: f32 = 10000000.0; // 10000+ km - continental shapes
const SCALE_MOUNTAIN: f32 = 100000.0;      // 100-10000 km - mountain ranges
const SCALE_VALLEY: f32 = 1000.0;          // 1-100 km - valleys, hills
const SCALE_CLIFF: f32 = 10.0;             // 10-1000 m - cliffs, boulders
const SCALE_ROCK: f32 = 0.1;               // 0.1-10 m - rocks, vegetation
const SCALE_MICRO: f32 = 0.001;            // < 0.1 m - pebbles, grass, soil

// Noise function parameters with observer scale for infinite LOD
struct TerrainParams {
    seed: u32,
    face_direction: vec3<f32>,
    axis_a: vec3<f32>,
    axis_b: vec3<f32>,
    chunk_start: vec2<f32>,     // chunk_start_x, chunk_start_y
    chunk_step: vec2<f32>,      // chunk_step_x, chunk_step_y
    resolution: u32,
    radius: f32,
    observer_scale: f32,        // Observer's current scale (meters per pixel)
    min_detail_scale: f32,      // Minimum detail to render at this LOD
}

@group(0) @binding(0) var<uniform> params: TerrainParams;
@group(0) @binding(1) var heightmap: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var colormap: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var detail_heightmap: texture_storage_2d<rgba8unorm, write>;  // For displacement mapping

// Hash function for pseudo-random number generation
fn hash(p: vec3<u32>) -> f32 {
    var h = p.x + p.y * 57u + p.z * 131u;
    h = h ^ h >> 15u;
    h *= 2246822507u;
    h ^= h >> 13u;
    h *= 3266489909u;
    h ^= h >> 16u;
    return f32(h) / f32(0xFFFFFFFFu);
}

// 3D hash for Perlin noise
fn hash3(p: vec3<f32>) -> vec3<f32> {
    let p_int = vec3<u32>(u32(p.x), u32(p.y), u32(p.z));
    return vec3<f32>(
        hash(p_int),
        hash(p_int + vec3<u32>(1u, 0u, 0u)),
        hash(p_int + vec3<u32>(0u, 1u, 0u))
    );
}

// Perlin noise (simplified 3D version)
fn perlin_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let f_smooth = f * f * (3.0 - 2.0 * f);
    
    let n000 = dot(hash3(i + vec3<f32>(0.0, 0.0, 0.0)), f - vec3<f32>(0.0, 0.0, 0.0));
    let n100 = dot(hash3(i + vec3<f32>(1.0, 0.0, 0.0)), f - vec3<f32>(1.0, 0.0, 0.0));
    let n010 = dot(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), f - vec3<f32>(0.0, 1.0, 0.0));
    let n110 = dot(hash3(i + vec3<f32>(1.0, 1.0, 0.0)), f - vec3<f32>(1.0, 1.0, 0.0));
    let n001 = dot(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), f - vec3<f32>(0.0, 0.0, 1.0));
    let n101 = dot(hash3(i + vec3<f32>(1.0, 0.0, 1.0)), f - vec3<f32>(1.0, 0.0, 1.0));
    let n011 = dot(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), f - vec3<f32>(0.0, 1.0, 1.0));
    let n111 = dot(hash3(i + vec3<f32>(1.0, 1.0, 1.0)), f - vec3<f32>(1.0, 1.0, 1.0));
    
    let x00 = mix(n000, n100, f_smooth.x);
    let x10 = mix(n010, n110, f_smooth.x);
    let x01 = mix(n001, n101, f_smooth.x);
    let x11 = mix(n011, n111, f_smooth.x);
    
    let y0 = mix(x00, x10, f_smooth.y);
    let y1 = mix(x01, x11, f_smooth.y);
    
    return mix(y0, y1, f_smooth.z);
}

// Calculate dynamic octave count based on observer scale
// This enables infinite detail - more octaves as you zoom in
fn calculate_dynamic_octaves(observer_scale: f32, base_frequency: f32) -> u32 {
    // effective_octaves = log2(observer_scale / min_detail_scale)
    // More octaves = more detail visible at current zoom level
    let scale_ratio = observer_scale / MIN_DETAIL_SCALE;
    let raw_octaves = log2(max(scale_ratio, 1.0));
    
    // Clamp to reasonable range and ensure at least 1 octave
    return clamp(u32(raw_octaves), 1u, MAX_OCTAVES);
}

// Fractional Brownian Motion (FBM) noise with FIXED octave count
// Use this for features that should be consistent regardless of zoom
fn fbm_noise(p: vec3<f32>, octaves: u32, frequency: f32, seed: u32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var freq = frequency;
    var max_value = 0.0;
    
    for (var i = 0u; i < octaves; i++) {
        value += perlin_noise(p * freq + vec3<f32>(f32(seed), f32(seed * 2u), f32(seed * 3u))) * amplitude;
        max_value += amplitude;
        amplitude *= PERSISTENCE;
        freq *= LACUNARITY;
    }
    
    return value / max_value;
}

// Dynamic FBM that adds detail based on observer scale
// This is the core of infinite scale rendering - detail emerges as you zoom in
fn fbm_noise_dynamic(p: vec3<f32>, base_frequency: f32, seed: u32, observer_scale: f32, min_freq: f32, max_freq: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var freq = base_frequency;
    var max_value = 0.0;
    
    // Calculate how many octaves we need based on observer scale
    let target_octaves = calculate_dynamic_octaves(observer_scale, base_frequency);
    
    // Start from lowest frequency and go to highest visible frequency
    for (var i = 0u; i < target_octaves; i++) {
        // Only include frequencies within the visible range
        if (freq >= min_freq && freq <= max_freq) {
            let seed_offset = vec3<f32>(f32(seed + i), f32(seed * 2u + i), f32(seed * 3u + i));
            value += perlin_noise(p * freq + seed_offset) * amplitude;
            max_value += amplitude;
        }
        amplitude *= PERSISTENCE;
        freq *= LACUNARITY;
    }
    
    if (max_value > 0.0) {
        return value / max_value;
    }
    return 0.0;
}

// Ridged Multi-Fractal noise (for mountains) - fixed octaves
fn ridged_noise(p: vec3<f32>, octaves: u32, frequency: f32, seed: u32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var freq = frequency;
    var max_value = 0.0;
    var weight = 1.0;
    
    for (var i = 0u; i < octaves; i++) {
        let n = perlin_noise(p * freq + vec3<f32>(f32(seed), f32(seed * 2u), f32(seed * 3u)));
        let ridged = 1.0 - abs(n);
        let signal = ridged * ridged * weight;
        value += signal * amplitude;
        max_value += amplitude;
        
        // Weight successive octaves by previous signal
        weight = clamp(signal * 2.0, 0.0, 1.0);
        amplitude *= PERSISTENCE;
        freq *= LACUNARITY;
    }
    
    return value / max_value;
}

// Dynamic ridged noise for infinite scale mountain detail
fn ridged_noise_dynamic(p: vec3<f32>, base_frequency: f32, seed: u32, observer_scale: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var freq = base_frequency;
    var max_value = 0.0;
    var weight = 1.0;
    
    let target_octaves = calculate_dynamic_octaves(observer_scale, base_frequency);
    
    for (var i = 0u; i < target_octaves; i++) {
        let seed_offset = vec3<f32>(f32(seed + i), f32(seed * 2u + i), f32(seed * 3u + i));
        let n = perlin_noise(p * freq + seed_offset);
        let ridged = 1.0 - abs(n);
        let signal = ridged * ridged * weight;
        value += signal * amplitude;
        max_value += amplitude;
        
        weight = clamp(signal * 2.0, 0.0, 1.0);
        amplitude *= PERSISTENCE;
        freq *= LACUNARITY;
    }
    
    if (max_value > 0.0) {
        return value / max_value;
    }
    return 0.0;
}

// Domain warping
fn domain_warp(p: vec3<f32>, warp_strength: f32, seed: u32) -> vec3<f32> {
    let warp1 = fbm_noise(p, 3u, 1.5, seed + 4u);
    let warp2 = fbm_noise(p + vec3<f32>(5.2, 1.3, 2.8), 3u, 1.5, seed + 4u);
    let warp3 = fbm_noise(p + vec3<f32>(1.7, 9.2, 5.2), 3u, 1.5, seed + 4u);
    return p + vec3<f32>(warp1, warp2, warp3) * warp_strength;
}

// Calculate elevation with scale-dependent detail layers
// This is the core terrain generation with infinite scale support
fn calculate_elevation(world_pos: vec3<f32>, seed: u32) -> f32 {
    return calculate_elevation_with_scale(world_pos, seed, 1000.0); // Default 1km scale
}

// Scale-dependent elevation calculation
// Different physical phenomena dominate at different scales:
// - Continental: tectonic plates, ocean basins (10000+ km)
// - Mountain: ranges, large valleys (100-10000 km)  
// - Valley: rivers, hills (1-100 km)
// - Cliff: rock formations, boulders (10-1000 m)
// - Rock: individual rocks, vegetation (0.1-10 m)
// - Micro: pebbles, grass, soil (< 0.1 m)
fn calculate_elevation_with_scale(world_pos: vec3<f32>, seed: u32, observer_scale: f32) -> f32 {
    // Domain warping for natural-looking distortion
    let warp_strength = 0.05;
    let q = domain_warp(world_pos, warp_strength, seed);
    
    // ========================================
    // LAYER 1: Continental Scale (always visible)
    // These features form the base terrain structure
    // ========================================
    let continent_val = fbm_noise(q, 5u, 0.5, seed);
    
    // Rivers (carved by erosion over geological time)
    let river_base = fbm_noise(q, 1u, 1.2, seed + 2u);
    let river_val = abs(river_base);
    let river_depth = select(0.0, (1.0 - (river_val / 0.04) * (river_val / 0.04)) * 0.08, 
        river_val < 0.04 && continent_val > 0.0);
    
    // Mountain range placement (tectonic boundaries)
    let range_mask = fbm_noise(q, 3u, 0.3, seed + 10u);
    let range_normalized = (range_mask + 1.0) * 0.5;
    let mountain_range_threshold = 0.4;
    let range_strength = select(0.0, 
        pow((range_normalized - mountain_range_threshold) / (1.0 - mountain_range_threshold), 2.0),
        range_normalized > mountain_range_threshold);
    
    // Mountains (ridged noise for realistic peaks)
    let mountain_base = ridged_noise(q, 8u, 2.5, seed + 1u);
    let height_multiplier = 1.0 + (range_normalized - mountain_range_threshold) * 2.0;
    let mountain_val = select(0.0,
        pow(max(mountain_base, 0.0), 1.2) * 2.5 * range_strength * height_multiplier,
        continent_val > -0.1 && range_strength > 0.0);
    
    // Base elevation from continental features
    var elevation = continent_val * 0.4 + mountain_val * 0.4 - river_depth;
    
    // ========================================
    // LAYER 2: Valley Scale (visible when observer_scale < 100km)
    // Hills, smaller valleys, terrain undulation
    // ========================================
    if (observer_scale < SCALE_MOUNTAIN) {
        let valley_detail = fbm_noise_dynamic(q, 10.0, seed + 100u, observer_scale, 5.0, 50.0);
        let valley_weight = smoothstep(SCALE_MOUNTAIN, SCALE_VALLEY, observer_scale);
        elevation += valley_detail * 0.02 * (1.0 - valley_weight);
    }
    
    // ========================================
    // LAYER 3: Cliff Scale (visible when observer_scale < 1km)
    // Cliffs, rock outcrops, large boulders
    // ========================================
    if (observer_scale < SCALE_VALLEY) {
        let cliff_detail = ridged_noise_dynamic(q, 100.0, seed + 200u, observer_scale);
        let cliff_weight = smoothstep(SCALE_VALLEY, SCALE_CLIFF, observer_scale);
        // Cliffs are more prominent on steep terrain (use noise-based approximation since dpdx/dpdy unavailable in compute)
        let slope_noise = abs(fbm_noise(q * 50.0, 2u, 1.0, seed + 201u));
        elevation += cliff_detail * 0.005 * (1.0 - cliff_weight) * (1.0 + slope_noise * 5.0);
    }
    
    // ========================================
    // LAYER 4: Rock Scale (visible when observer_scale < 10m)
    // Individual rocks, boulders, vegetation bumps
    // ========================================
    if (observer_scale < SCALE_CLIFF) {
        let rock_detail = fbm_noise_dynamic(q, 1000.0, seed + 300u, observer_scale, 100.0, 5000.0);
        let rock_weight = smoothstep(SCALE_CLIFF, SCALE_ROCK, observer_scale);
        elevation += rock_detail * 0.001 * (1.0 - rock_weight);
    }
    
    // ========================================
    // LAYER 5: Micro Scale (visible when observer_scale < 0.1m)
    // Pebbles, grass texture, soil variation
    // ========================================
    if (observer_scale < SCALE_ROCK) {
        let micro_detail = fbm_noise_dynamic(q, 10000.0, seed + 400u, observer_scale, 1000.0, 100000.0);
        let micro_weight = smoothstep(SCALE_ROCK, SCALE_MICRO, observer_scale);
        elevation += micro_detail * 0.0001 * (1.0 - micro_weight);
    }
    
    // Clamp for ocean floor
    elevation = max(elevation, -0.2);
    
    return elevation;
}

// Calculate biome color with scale-dependent detail
fn calculate_color(elevation: f32, world_pos: vec3<f32>, seed: u32, observer_scale: f32) -> vec4<f32> {
    // Base biome colors
    var base_color: vec4<f32>;
    
    if (elevation < -0.15) {
        // Deep ocean - darker blue
        let depth = clamp((-elevation - 0.15) / 0.05, 0.0, 1.0);
        base_color = mix(vec4<f32>(0.0, 0.3, 0.8, 1.0), vec4<f32>(0.0, 0.1, 0.4, 1.0), depth);
    } else if (elevation < 0.0) {
        // Shallow water to beach transition
        let t = (elevation + 0.15) / 0.15;
        base_color = mix(vec4<f32>(0.0, 0.4, 0.9, 1.0), vec4<f32>(0.9, 0.85, 0.7, 1.0), t);
    } else if (elevation < 0.05) {
        // Beach/sand
        base_color = vec4<f32>(0.92, 0.85, 0.65, 1.0);
    } else if (elevation < 0.15) {
        // Grass/plains
        let t = (elevation - 0.05) / 0.10;
        base_color = mix(vec4<f32>(0.4, 0.7, 0.2, 1.0), vec4<f32>(0.2, 0.5, 0.15, 1.0), t);
    } else if (elevation < 0.3) {
        // Forest
        let t = (elevation - 0.15) / 0.15;
        base_color = mix(vec4<f32>(0.15, 0.45, 0.1, 1.0), vec4<f32>(0.3, 0.35, 0.25, 1.0), t);
    } else if (elevation < 0.5) {
        // Mountain/rock
        let t = (elevation - 0.3) / 0.2;
        base_color = mix(vec4<f32>(0.45, 0.4, 0.35, 1.0), vec4<f32>(0.6, 0.55, 0.5, 1.0), t);
    } else {
        // Snow caps
        let t = clamp((elevation - 0.5) / 0.2, 0.0, 1.0);
        base_color = mix(vec4<f32>(0.6, 0.55, 0.5, 1.0), vec4<f32>(0.95, 0.95, 0.98, 1.0), t);
    }
    
    // Add scale-dependent color variation
    if (observer_scale < SCALE_CLIFF) {
        // Add micro-variation for rocks and vegetation
        let color_noise = fbm_noise(world_pos * 500.0, 3u, 1.0, seed + 500u);
        base_color = base_color + vec4<f32>(color_noise * 0.05, color_noise * 0.05, color_noise * 0.03, 0.0);
    }
    
    if (observer_scale < SCALE_ROCK) {
        // Add fine detail color variation
        let detail_noise = fbm_noise(world_pos * 5000.0, 2u, 1.0, seed + 600u);
        base_color = base_color + vec4<f32>(detail_noise * 0.02, detail_noise * 0.02, detail_noise * 0.02, 0.0);
    }
    
    return clamp(base_color, vec4<f32>(0.0), vec4<f32>(1.0));
}

// Legacy color function for backward compatibility
fn calculate_color_simple(elevation: f32, world_pos: vec3<f32>, seed: u32) -> vec4<f32> {
    return calculate_color(elevation, world_pos, seed, 1000.0);
}

// Generate high-frequency detail for displacement mapping
// This adds surface micro-detail that the vertex displacement shader will use
fn calculate_displacement_detail(world_pos: vec3<f32>, seed: u32, observer_scale: f32) -> vec4<f32> {
    // Multiple scales of detail for parallax/displacement
    let scale1 = 100.0;  // Medium detail (rocks, bumps)
    let scale2 = 500.0;  // Fine detail (pebbles)
    let scale3 = 2000.0; // Micro detail (soil texture)
    
    // Calculate detail at each scale
    let detail1 = fbm_noise(world_pos * scale1, 4u, 1.0, seed + 1000u);
    let detail2 = fbm_noise(world_pos * scale2, 3u, 1.0, seed + 1001u);
    let detail3 = fbm_noise(world_pos * scale3, 2u, 1.0, seed + 1002u);
    
    // Add ridged detail for rocky surfaces
    let ridged = ridged_noise(world_pos * scale1 * 0.5, 3u, 2.0, seed + 1003u);
    
    // Combine into displacement value
    // R channel: primary displacement (medium scale)
    // G channel: fine displacement (small scale)  
    // B channel: micro displacement (very small scale)
    // A channel: ridged detail for rocky areas
    
    let r = (detail1 * 0.5 + 0.5);  // Normalize to 0-1
    let g = (detail2 * 0.5 + 0.5);
    let b = (detail3 * 0.5 + 0.5);
    let a = ridged;
    
    return vec4<f32>(r, g, b, a);
}

// Calculate normal map from heightmap gradients for displacement
fn calculate_detail_normal(world_pos: vec3<f32>, seed: u32, strength: f32) -> vec3<f32> {
    let eps = 0.001;
    let scale = 200.0;
    
    // Sample heights at offset positions
    let h_center = fbm_noise(world_pos * scale, 4u, 1.0, seed + 1000u);
    let h_right = fbm_noise((world_pos + vec3<f32>(eps, 0.0, 0.0)) * scale, 4u, 1.0, seed + 1000u);
    let h_up = fbm_noise((world_pos + vec3<f32>(0.0, eps, 0.0)) * scale, 4u, 1.0, seed + 1000u);
    let h_forward = fbm_noise((world_pos + vec3<f32>(0.0, 0.0, eps)) * scale, 4u, 1.0, seed + 1000u);
    
    // Calculate gradient
    let dx = (h_right - h_center) / eps * strength;
    let dy = (h_up - h_center) / eps * strength;
    let dz = (h_forward - h_center) / eps * strength;
    
    // Perturb normal (pointing outward from sphere)
    let surface_normal = normalize(world_pos);
    let perturbed = normalize(surface_normal - vec3<f32>(dx, dy, dz) * 0.1);
    
    return perturbed;
}

@compute @workgroup_size(16, 16)
fn generate_terrain(@builtin(global_invocation_id) id: vec3<u32>) {
    // Check bounds
    if (id.x >= params.resolution || id.y >= params.resolution) {
        return;
    }
    
    // Calculate cube-sphere coordinates (matching CPU version)
    let local_x = params.chunk_start.x + f32(id.x) * params.chunk_step.x;
    let local_y = params.chunk_start.y + f32(id.y) * params.chunk_step.y;
    
    // Point on unit cube face
    let point_on_unit_cube = params.face_direction + local_x * params.axis_a + local_y * params.axis_b;
    
    // Project to unit sphere (cube-sphere mapping)
    let point_on_unit_sphere = normalize(point_on_unit_cube);
    
    // World position on sphere (will be scaled by elevation later)
    let world_pos = point_on_unit_sphere * params.radius;
    
    // Get observer scale for dynamic detail level
    // This enables infinite scale rendering - more detail emerges as you zoom in
    let observer_scale = max(params.observer_scale, MIN_DETAIL_SCALE);
    
    // Calculate elevation with scale-dependent detail
    let elevation = calculate_elevation_with_scale(point_on_unit_sphere, params.seed, observer_scale);
    
    // Height multiplier for more dramatic terrain (0.15 = up to 15% of radius = 3000m on 20km planet)
    let height_factor = 0.15;
    
    // Store heightmap (primary elevation)
    textureStore(heightmap, vec2<i32>(id.xy), vec4<f32>(elevation, 0.0, 0.0, 0.0));
    
    // Calculate and store color with scale-dependent variation
    let color = calculate_color(elevation, point_on_unit_sphere, params.seed, observer_scale);
    textureStore(colormap, vec2<i32>(id.xy), color);
    
    // Calculate and store detail heightmap for displacement/parallax
    // This provides high-frequency surface detail for the hyperreal shader
    let detail = calculate_displacement_detail(point_on_unit_sphere, params.seed, observer_scale);
    textureStore(detail_heightmap, vec2<i32>(id.xy), detail);
}

