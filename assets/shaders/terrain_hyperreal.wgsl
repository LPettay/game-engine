// Hyperreal Terrain Shader
// Combines vertex displacement, normal mapping, and parallax occlusion mapping
// for maximum visual fidelity without native GPU tessellation

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    pbr_types,
    pbr_functions,
    forward_io::{Vertex, VertexOutput},
}
#import bevy_render::instance_index::get_instance_index

// Material uniforms
struct TerrainMaterial {
    // x = detail_scale, y = displacement_strength, z = roughness, w = metallic
    params: vec4<f32>,
    // x = parallax_scale, y = parallax_layers, z = normal_strength, w = reserved
    parallax_params: vec4<f32>,
    // Camera position for LOD and parallax
    camera_pos: vec3<f32>,
    _padding: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: TerrainMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var heightmap_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var heightmap_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var detail_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var detail_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var normal_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var normal_sampler: sampler;

// ===== VERTEX SHADER WITH DISPLACEMENT =====

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    // Get world position from mesh
    var world_from_local = mesh_functions::get_world_from_local(get_instance_index(vertex.instance_index));
    
    // Sample heightmap for displacement
    let uv = vertex.uv;
    let height = textureSampleLevel(heightmap_texture, heightmap_sampler, uv, 0.0).r;
    
    // Calculate displacement direction (surface normal approximation)
    // For a sphere, this is the normalized position
    let local_pos = vertex.position;
    let displacement_dir = normalize(local_pos);
    
    // Calculate distance to camera for LOD-based displacement
    let world_pos_base = (world_from_local * vec4<f32>(local_pos, 1.0)).xyz;
    let dist_to_camera = length(material.camera_pos - world_pos_base);
    
    // Displacement strength fades with distance to prevent popping
    let displacement_strength = material.params.y;
    let max_displacement_dist = 1000.0; // Full displacement within 1km
    let displacement_fade = saturate(1.0 - dist_to_camera / max_displacement_dist);
    
    // Apply displacement along surface normal
    let displaced_pos = local_pos + displacement_dir * height * displacement_strength * displacement_fade;
    
    // Transform to world space
    out.world_position = world_from_local * vec4<f32>(displaced_pos, 1.0);
    out.position = position_world_to_clip(out.world_position.xyz);
    
    // Calculate displaced normal using heightmap gradients
    let texel_size = 1.0 / 512.0; // Heightmap resolution
    let h_left = textureSampleLevel(heightmap_texture, heightmap_sampler, uv - vec2<f32>(texel_size, 0.0), 0.0).r;
    let h_right = textureSampleLevel(heightmap_texture, heightmap_sampler, uv + vec2<f32>(texel_size, 0.0), 0.0).r;
    let h_down = textureSampleLevel(heightmap_texture, heightmap_sampler, uv - vec2<f32>(0.0, texel_size), 0.0).r;
    let h_up = textureSampleLevel(heightmap_texture, heightmap_sampler, uv + vec2<f32>(0.0, texel_size), 0.0).r;
    
    let gradient_x = (h_right - h_left) * displacement_strength * 0.5;
    let gradient_y = (h_up - h_down) * displacement_strength * 0.5;
    
    // Perturb normal based on height gradients
    let base_normal = normalize((world_from_local * vec4<f32>(vertex.normal, 0.0)).xyz);
    let tangent = normalize(cross(base_normal, vec3<f32>(0.0, 1.0, 0.0)));
    let bitangent = cross(base_normal, tangent);
    
    let perturbed_normal = normalize(base_normal - tangent * gradient_x - bitangent * gradient_y);
    out.world_normal = mix(base_normal, perturbed_normal, displacement_fade);
    
    // Pass through other attributes
    out.uv = uv;
    
    #ifdef VERTEX_COLORS
        out.color = vertex.color;
    #endif
    
    #ifdef VERTEX_TANGENTS
        out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
            world_from_local,
            vertex.tangent,
            get_instance_index(vertex.instance_index)
        );
    #endif
    
    return out;
}

// ===== FRAGMENT SHADER WITH PARALLAX OCCLUSION MAPPING =====

// Steep Parallax Occlusion Mapping for close-up detail
fn parallax_occlusion_mapping(uv: vec2<f32>, view_dir: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let parallax_scale = material.parallax_params.x;
    let num_layers = i32(material.parallax_params.y);
    
    if (parallax_scale < 0.001 || num_layers < 1) {
        return uv;
    }
    
    // Calculate tangent space view direction
    // Simplified: project view onto surface plane
    let view_on_surface = normalize(view_dir - normal * dot(view_dir, normal));
    
    // Layer stepping
    let layer_depth = 1.0 / f32(num_layers);
    var current_layer_depth = 0.0;
    
    // Calculate UV offset per layer
    let p = view_on_surface.xy * parallax_scale;
    let delta_uv = p / f32(num_layers);
    
    var current_uv = uv;
    var current_depth = textureSample(heightmap_texture, heightmap_sampler, current_uv).r;
    
    // Step through layers until we hit the surface
    for (var i = 0; i < num_layers; i++) {
        if (current_layer_depth >= current_depth) {
            break;
        }
        current_uv -= delta_uv;
        current_depth = textureSample(heightmap_texture, heightmap_sampler, current_uv).r;
        current_layer_depth += layer_depth;
    }
    
    // Binary search refinement for smoother result
    let prev_uv = current_uv + delta_uv;
    let prev_depth = current_layer_depth - layer_depth;
    
    let after_depth = current_depth - current_layer_depth;
    let before_depth = textureSample(heightmap_texture, heightmap_sampler, prev_uv).r - prev_depth;
    
    let weight = after_depth / (after_depth - before_depth);
    return mix(current_uv, prev_uv, weight);
}

// Triplanar sampling with smooth blending
fn triplanar_sample_detail(world_pos: vec3<f32>, normal: vec3<f32>, scale: f32) -> vec3<f32> {
    let weights = pow(abs(normal), vec3<f32>(4.0));
    let total_weight = weights.x + weights.y + weights.z;
    let w = weights / max(total_weight, 0.0001);
    
    let col_x = textureSample(detail_texture, detail_sampler, world_pos.yz * scale).rgb;
    let col_y = textureSample(detail_texture, detail_sampler, world_pos.xz * scale).rgb;
    let col_z = textureSample(detail_texture, detail_sampler, world_pos.xy * scale).rgb;
    
    return col_x * w.x + col_y * w.y + col_z * w.z;
}

// Triplanar normal sampling
fn triplanar_sample_normal(world_pos: vec3<f32>, surface_normal: vec3<f32>, scale: f32) -> vec3<f32> {
    let weights = pow(abs(surface_normal), vec3<f32>(4.0));
    let total_weight = weights.x + weights.y + weights.z;
    let w = weights / max(total_weight, 0.0001);
    
    // Sample normal maps from each projection
    let n_x = textureSample(normal_texture, normal_sampler, world_pos.yz * scale).rgb * 2.0 - 1.0;
    let n_y = textureSample(normal_texture, normal_sampler, world_pos.xz * scale).rgb * 2.0 - 1.0;
    let n_z = textureSample(normal_texture, normal_sampler, world_pos.xy * scale).rgb * 2.0 - 1.0;
    
    // Reorient normals for each projection
    let n_x_reoriented = vec3<f32>(n_x.z, n_x.y, -n_x.x);
    let n_y_reoriented = vec3<f32>(n_x.x, n_x.z, n_x.y);
    let n_z_reoriented = n_z;
    
    return normalize(n_x_reoriented * w.x + n_y_reoriented * w.y + n_z_reoriented * w.z);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let view_dir = normalize(material.camera_pos - world_pos);
    let dist_to_camera = length(material.camera_pos - world_pos);
    
    // Apply parallax occlusion mapping for close surfaces
    let parallax_max_dist = 100.0; // Only apply POM within 100m
    let parallax_fade = saturate(1.0 - dist_to_camera / parallax_max_dist);
    var final_uv = in.uv;
    if (parallax_fade > 0.01) {
        let parallax_uv = parallax_occlusion_mapping(in.uv, view_dir, in.world_normal);
        final_uv = mix(in.uv, parallax_uv, parallax_fade);
    }
    
    // Sample detail textures with multi-scale triplanar
    let detail_scale = material.params.x;
    let detail_coarse = triplanar_sample_detail(world_pos, in.world_normal, detail_scale);
    let detail_fine = triplanar_sample_detail(world_pos, in.world_normal, detail_scale * 4.0);
    let detail_micro = triplanar_sample_detail(world_pos, in.world_normal, detail_scale * 16.0);
    
    // Blend detail scales based on distance
    let fine_blend = saturate(1.0 - dist_to_camera / 500.0);
    let micro_blend = saturate(1.0 - dist_to_camera / 50.0);
    
    var combined_detail = detail_coarse;
    combined_detail = mix(combined_detail, (combined_detail + detail_fine) * 0.5, fine_blend);
    combined_detail = mix(combined_detail, (combined_detail + detail_micro) * 0.5, micro_blend);
    
    // Sample and blend normal maps
    var final_normal = in.world_normal;
    let normal_strength = material.parallax_params.z;
    if (normal_strength > 0.001) {
        let sampled_normal = triplanar_sample_normal(world_pos, in.world_normal, detail_scale);
        let normal_blend = saturate(1.0 - dist_to_camera / 200.0) * normal_strength;
        final_normal = normalize(mix(in.world_normal, sampled_normal, normal_blend));
    }
    
    // Build PBR input using Bevy's default constructor
    var pbr_input = pbr_types::pbr_input_new();

    // Apply detail modulation to vertex color
    let detail_intensity = 0.3 * (1.0 - dist_to_camera / 1000.0);
    let detail_mod = mix(vec3<f32>(1.0), combined_detail * 1.5, max(detail_intensity, 0.0));

    #ifdef VERTEX_COLORS
        pbr_input.material.base_color = vec4<f32>(in.color.rgb * detail_mod, in.color.a);
    #else
        pbr_input.material.base_color = vec4<f32>(detail_mod, 1.0);
    #endif

    pbr_input.material.perceptual_roughness = material.params.z;
    pbr_input.material.metallic = material.params.w;

    // Vary roughness with detail texture
    pbr_input.material.perceptual_roughness = mix(
        pbr_input.material.perceptual_roughness,
        1.0,
        combined_detail.r * 0.3 * fine_blend
    );

    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    pbr_input.world_normal = pbr_functions::prepare_world_normal(final_normal, false, is_front);
    pbr_input.N = pbr_input.world_normal;
    pbr_input.V = view_dir;

    return pbr_functions::apply_pbr_lighting(pbr_input);
}

