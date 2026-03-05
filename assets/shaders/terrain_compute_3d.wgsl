// 3D Density Field Compute Shader
// Based on terrain_compute.wgsl, adapted for 3D density volume output.
// Evaluates SDF: density = radial_distance - surface_radius

struct TerrainParams {
    seed: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
    face_direction: vec3<f32>,
    _pad_face: f32,
    axis_a: vec3<f32>,
    _pad_a: f32,
    axis_b: vec3<f32>,
    _pad_b: f32,
    chunk_start: vec2<f32>,
    chunk_step: vec2<f32>,
    resolution: u32,
    radius: f32,
    observer_scale: f32,
    min_detail_scale: f32,
};

@group(0) @binding(0) var<uniform> params: TerrainParams;
@group(0) @binding(1) var<storage, read_write> density_output: array<f32>;

// Simple hash-based noise (GPU-friendly)
fn hash(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash(i);
    let n100 = hash(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash(i + vec3<f32>(1.0, 1.0, 1.0));

    let nx0 = mix(mix(n000, n100, u.x), mix(n010, n110, u.x), u.y);
    let nx1 = mix(mix(n001, n101, u.x), mix(n011, n111, u.x), u.y);
    return mix(nx0, nx1, u.z);
}

fn fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise3d(pos * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    return value;
}

fn evaluate_terrain_elevation(unit_point: vec3<f32>) -> f32 {
    let p = unit_point;
    let seed_offset = vec3<f32>(f32(params.seed) * 0.1, 0.0, 0.0);

    // Continental shape
    let continent = fbm(p * 0.5 + seed_offset, 6) * 0.45;

    // Mountains (ridged noise approximation)
    let mountain_mask = fbm(p * 0.25 + seed_offset + vec3<f32>(10.0, 0.0, 0.0), 4);
    let mountain_raw = fbm(p * 2.0 + seed_offset + vec3<f32>(1.0, 0.0, 0.0), 8);
    let mountain_strength = max(0.0, (mountain_mask + 1.0) * 0.5 - 0.35);
    let mountain = max(0.0, mountain_raw) * mountain_strength * 0.45;

    // Rivers
    let river_raw = fbm(p * 1.0 + seed_offset + vec3<f32>(2.0, 0.0, 0.0), 4);
    let river_depth = select(0.0, (1.0 - (abs(river_raw) / 0.04) * (abs(river_raw) / 0.04)) * 0.1,
                             abs(river_raw) < 0.04 && continent > 0.0);

    var elevation = continent + mountain - river_depth;
    return max(elevation, -0.25);
}

@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let res = params.resolution;

    if (global_id.x >= res || global_id.y >= res || global_id.z >= res) {
        return;
    }

    // Calculate world position for this voxel
    let grid_pos = vec3<f32>(f32(global_id.x), f32(global_id.y), f32(global_id.z));
    let normalized = grid_pos / f32(res - 1u);

    // Map to chunk-local coordinates
    let face_u = params.chunk_start.x + normalized.x * params.chunk_step.x;
    let face_v = params.chunk_start.y + normalized.y * params.chunk_step.y;
    let radial_t = normalized.z; // 0 = inner, 1 = outer

    // Point on cube face → project to sphere
    let point_on_cube = params.face_direction + face_u * params.axis_a + face_v * params.axis_b;
    let unit_point = normalize(point_on_cube);

    // Get terrain elevation
    let elevation = evaluate_terrain_elevation(unit_point);
    let surface_radius = params.radius * (1.0 + elevation * 0.15);

    // Radial position for this voxel (spans from below to above surface)
    let chunk_radial_extent = params.radius * 2.0 / f32(1u << u32(log2(f32(res))));
    let radial_distance = surface_radius - chunk_radial_extent + radial_t * chunk_radial_extent * 2.0;

    // SDF: positive = air, negative = solid
    let density = radial_distance - surface_radius;

    // Write to output buffer
    let index = global_id.z * res * res + global_id.y * res + global_id.x;
    density_output[index] = density;
}
