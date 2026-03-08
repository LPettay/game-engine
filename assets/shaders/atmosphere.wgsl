#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_bindings
#import bevy_pbr::forward_io::VertexOutput

struct AtmosphereMaterial {
    planet_radius: f32,
    atmosphere_radius: f32,
    // Packed Vec4s (16-byte alignment)
    rayleigh_scattering_scale_height: vec4<f32>, // xyz = scattering, w = scale height
    mie_scattering_scale_height_asymmetry: vec4<f32>, // x = scattering, y = scale height, z = asymmetry, w = unused
    sun_position: vec4<f32>, // xyz = pos
    view_position: vec4<f32>, // xyz = pos
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: AtmosphereMaterial;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    // Unpack Uniforms
    let planet_radius = material.planet_radius;
    let atmosphere_radius = material.atmosphere_radius;
    let rayleigh_scattering = material.rayleigh_scattering_scale_height.xyz;
    let rayleigh_scale_height = material.rayleigh_scattering_scale_height.w;
    let mie_scattering = material.mie_scattering_scale_height_asymmetry.x;
    let mie_scale_height = material.mie_scattering_scale_height_asymmetry.y;
    let mie_asymmetry = material.mie_scattering_scale_height_asymmetry.z;
    let sun_position = material.sun_position.xyz;
    let sun_intensity = material.sun_position.w; // Use intensity
    let view_pos = material.view_position.xyz;

    // Ray Casting Logic
    let ray_origin = view_pos;
    let ray_dir = normalize(in.world_position.xyz - view_pos);
    
    let planet_center = vec3<f32>(0.0, 0.0, 0.0);
    
    // Intersect Atmosphere (Outer Shell)
    // Ray-Sphere Intersection
    // t^2 + 2*b*t + c = 0
    let L = ray_origin - planet_center;
    let a = dot(ray_dir, ray_dir);
    let b = 2.0 * dot(ray_dir, L);
    let c = dot(L, L) - atmosphere_radius * atmosphere_radius;
    
    let delta = b * b - 4.0 * a * c;
    
    if (delta < 0.0) {
        discard; 
    }
    
    let sqrt_delta = sqrt(delta);
    let t0 = (-b - sqrt_delta) / (2.0 * a);
    let t1 = (-b + sqrt_delta) / (2.0 * a);
    
    // Determine start and end of ray through atmosphere
    var t_start = max(t0, 0.0);
    var t_end = max(t1, 0.0);
    
    // If we are inside the atmosphere (usual case for player), start is 0
    if (length(L) < atmosphere_radius) {
        t_start = 0.0;
        // t_end is the exit point
    } else {
        // Outside looking in
        if (t0 < 0.0 && t1 < 0.0) { discard; } // Behind camera
        t_start = max(0.0, t0);
        t_end = t1;
    }

    // Check for planet occlusion (ground intersection)
    let c_planet = dot(L, L) - planet_radius * planet_radius;
    let delta_planet = b * b - 4.0 * a * c_planet;
    if (delta_planet >= 0.0) {
        let t_planet0 = (-b - sqrt(delta_planet)) / (2.0 * a);
        let t_planet1 = (-b + sqrt(delta_planet)) / (2.0 * a);
        
        // If ray hits planet, stop at planet surface
        // We need the FIRST positive intersection
        if (t_planet0 > 0.0) {
             t_end = min(t_end, t_planet0);
        } else if (t_planet1 > 0.0) {
             t_end = min(t_end, t_planet1);
        }
    }

    if (t_end <= t_start) {
        discard;
    }

    // Raymarch
    let num_samples = 8; // Reduced for performance
    let step_size = (t_end - t_start) / f32(num_samples);
    
    var current_t = t_start;
    var optical_depth_rayleigh = 0.0;
    var optical_depth_mie = 0.0;
    
    var total_rayleigh = vec3<f32>(0.0);
    var total_mie = vec3<f32>(0.0);
    
    let sun_dir = normalize(sun_position - planet_center); 
    
    for (var i = 0; i < num_samples; i++) {
        let sample_pos = ray_origin + ray_dir * (current_t + step_size * 0.5);
        let height = length(sample_pos - planet_center) - planet_radius;
        
        if (height < 0.0) { current_t += step_size; continue; } 
        
        // Density
        let hr = exp(-height / rayleigh_scale_height) * step_size;
        let hm = exp(-height / mie_scale_height) * step_size;
        
        optical_depth_rayleigh += hr;
        optical_depth_mie += hm;
        
        // Light Sample (Ray to Sun)
        let L_sun = sample_pos - planet_center;
        let b_sun = 2.0 * dot(sun_dir, L_sun);
        let c_sun = dot(L_sun, L_sun) - planet_radius * planet_radius;
        let delta_sun = b_sun * b_sun - 4.0 * c_sun; 
        
        var light_blocked = false;
        if (delta_sun >= 0.0) {
            let t_sun0 = (-b_sun - sqrt(delta_sun)) / 2.0;
            let t_sun1 = (-b_sun + sqrt(delta_sun)) / 2.0;
            if (t_sun0 > 0.0 && t_sun1 > 0.0) {
                 light_blocked = true; 
            }
        }

        if (!light_blocked) {
            var sun_ray_depth_r = 0.0;
            var sun_ray_depth_m = 0.0;
            
            // Distance to atmosphere exit towards sun
            let b_atmo = 2.0 * dot(sun_dir, L_sun);
            let c_atmo = dot(L_sun, L_sun) - atmosphere_radius * atmosphere_radius;
            let delta_atmo = b_atmo * b_atmo - 4.0 * c_atmo;
            let t_sun_exit = (-b_atmo + sqrt(delta_atmo)) / 2.0;
            
            // Single sample for sun (optimization) or few samples
            let sun_step = t_sun_exit / 2.0; 
            for (var j = 0; j < 2; j++) {
                let sun_sample_pos = sample_pos + sun_dir * (f32(j) * sun_step + sun_step * 0.5);
                let sun_height = length(sun_sample_pos) - planet_radius;
                if (sun_height > 0.0) {
                     sun_ray_depth_r += exp(-sun_height / rayleigh_scale_height) * sun_step;
                     sun_ray_depth_m += exp(-sun_height / mie_scale_height) * sun_step;
                }
            }
            
            let tau = rayleigh_scattering * (optical_depth_rayleigh + sun_ray_depth_r) + 
                      mie_scattering * 1.1 * (optical_depth_mie + sun_ray_depth_m);
            
            let attenuation = exp(-tau);
            
            total_rayleigh += attenuation * hr;
            total_mie += attenuation * hm;
        }
        
        current_t += step_size;
    }
    
    // Phase Functions
    let mu = dot(ray_dir, sun_dir);
    let phase_r = 3.0 / (16.0 * 3.14159) * (1.0 + mu * mu);
    let g = mie_asymmetry;
    let phase_m = 3.0 / (8.0 * 3.14159) * ((1.0 - g * g) * (1.0 + mu * mu)) / ((2.0 + g * g) * pow(1.0 + g * g - 2.0 * g * mu, 1.5));
    
    let color = (total_rayleigh * rayleigh_scattering * phase_r + total_mie * mie_scattering * phase_m) * sun_intensity;
    
    // Exposure
    let exposed_color = vec3<f32>(1.0) - exp(-color * 1.0); 
    
    // Alpha approximation
    let extinction = exp(-(rayleigh_scattering * optical_depth_rayleigh + mie_scattering * 1.1 * optical_depth_mie));
    let T = (extinction.x + extinction.y + extinction.z) / 3.0;
    let alpha = 1.0 - T;
    
    if (alpha < 0.001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    return vec4<f32>(color / alpha, alpha);
}
