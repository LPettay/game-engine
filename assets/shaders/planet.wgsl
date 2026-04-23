#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_bindings
#import bevy_pbr::pbr_types as pbr_types
#import bevy_pbr::pbr_functions as pbr_functions
#import bevy_pbr::forward_io::VertexOutput

struct PlanetMaterial {
    scaling: vec4<f32>, // xy = texture scale, z = roughness, w = metallic
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: PlanetMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var detail_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var detail_sampler: sampler;

// Smooth Triplanar Mapping with power-based blending to reduce seams
fn triplanar_sample(
    world_pos: vec3<f32>, 
    normal: vec3<f32>, 
    scale: f32
) -> vec3<f32> {
    // Use power-based weights for smoother blending (reduces visible seams)
    let weights = abs(normal);
    // Power of 4 creates smoother transitions
    let weights_powered = vec3<f32>(
        pow(weights.x, 4.0),
        pow(weights.y, 4.0),
        pow(weights.z, 4.0)
    );
    let total_weight = weights_powered.x + weights_powered.y + weights_powered.z;
    let w = weights_powered / max(total_weight, 0.0001); // Avoid division by zero

    let uv_x = world_pos.yz * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;

    let col_x = textureSample(detail_texture, detail_sampler, uv_x).rgb;
    let col_y = textureSample(detail_texture, detail_sampler, uv_y).rgb;
    let col_z = textureSample(detail_texture, detail_sampler, uv_z).rgb;

    return col_x * w.x + col_y * w.y + col_z * w.z;
}

// Generate procedural normal perturbation from noise
fn procedural_normal_detail(
    world_pos: vec3<f32>,
    surface_normal: vec3<f32>,
    scale: f32,
    strength: f32
) -> vec3<f32> {
    // Sample height at neighboring points to calculate gradient
    let eps = 0.01;
    
    // Build tangent frame from surface normal
    var tangent: vec3<f32>;
    if (abs(surface_normal.y) < 0.99) {
        tangent = normalize(cross(surface_normal, vec3<f32>(0.0, 1.0, 0.0)));
    } else {
        tangent = normalize(cross(surface_normal, vec3<f32>(1.0, 0.0, 0.0)));
    }
    let bitangent = cross(surface_normal, tangent);
    
    // Sample detail texture at offset positions
    let h_center = triplanar_sample(world_pos, surface_normal, scale).r;
    let h_right = triplanar_sample(world_pos + tangent * eps, surface_normal, scale).r;
    let h_up = triplanar_sample(world_pos + bitangent * eps, surface_normal, scale).r;
    
    // Calculate gradient in tangent space
    let dx = (h_right - h_center) / eps;
    let dy = (h_up - h_center) / eps;
    
    // Perturb normal
    let perturbed = normalize(surface_normal - tangent * dx * strength - bitangent * dy * strength);
    
    return perturbed;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    // Use Bevy's default PbrInput constructor (initializes all fields correctly)
    var pbr_input = pbr_types::pbr_input_new();

    // Override material properties we care about
    pbr_input.material.base_color = in.color;
    pbr_input.material.perceptual_roughness = material.scaling.z;
    pbr_input.material.metallic = material.scaling.w;

    // Triplanar Noise with improved sampling
    let noise_scale = material.scaling.x;
    let soft_lighting = material.scaling.y; // 0.0 or 1.0

    // Use higher frequency noise for finer detail
    let noise = triplanar_sample(in.world_position.xyz, in.world_normal, noise_scale);
    let noise_fine = triplanar_sample(in.world_position.xyz * 4.0, in.world_normal, noise_scale * 4.0);
    let noise_micro = triplanar_sample(in.world_position.xyz * 16.0, in.world_normal, noise_scale * 16.0);

    // Combine multiple octaves for richer detail (3 octaves for hyperreal)
    let combined_noise = noise * 0.5 + noise_fine * 0.35 + noise_micro * 0.15;

    // Modulate Base Color with moderate intensity
    let detail_intensity = 0.25;
    let detail_mod = mix(vec3<f32>(1.0), combined_noise * 1.5, detail_intensity);
    pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * detail_mod, pbr_input.material.base_color.a);

    // Apply procedural normal mapping for surface detail
    let normal_scale = noise_scale * 8.0;
    let normal_strength = 0.3;
    var perturbed_normal = procedural_normal_detail(
        in.world_position.xyz,
        in.world_normal,
        normal_scale,
        normal_strength
    );

    // Add finer normal detail at close range
    let fine_normal = procedural_normal_detail(
        in.world_position.xyz,
        perturbed_normal,
        normal_scale * 4.0,
        normal_strength * 0.5
    );
    perturbed_normal = normalize(mix(perturbed_normal, fine_normal, 0.5));

    if (soft_lighting > 0.5) {
        // Soft lighting mode: Make it fully emissive
        pbr_input.material.emissive = vec4<f32>(pbr_input.material.base_color.rgb * 0.5, 1.0);
        pbr_input.material.perceptual_roughness = 1.0;
        pbr_input.material.reflectance = vec3<f32>(0.0);
    } else {
        // Default PBR with enhanced detail
        // Vary roughness based on noise for material variation
        pbr_input.material.perceptual_roughness = mix(
            pbr_input.material.perceptual_roughness * 0.8,
            pbr_input.material.perceptual_roughness * 1.2,
            combined_noise.r
        );
        pbr_input.material.perceptual_roughness = clamp(pbr_input.material.perceptual_roughness, 0.1, 1.0);
    }

    // PBR Environment Setup
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = vec4<f32>(in.world_position.xyz, 1.0);

    // Use perturbed normal for lighting calculations
    pbr_input.world_normal = pbr_functions::prepare_world_normal(
        perturbed_normal,
        false,
        is_front,
    );

    pbr_input.is_orthographic = bevy_pbr::mesh_view_bindings::view.clip_from_view[3].w == 1.0;
    pbr_input.N = pbr_input.world_normal;
    pbr_input.V = pbr_functions::calculate_view(in.world_position, pbr_input.is_orthographic);

    return pbr_functions::apply_pbr_lighting(pbr_input);
}
