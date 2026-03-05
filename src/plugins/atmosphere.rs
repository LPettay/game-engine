use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::mesh::MeshVertexBufferLayoutRef;
use crate::plugins::planet::PlanetSettings; // Imported from planet.rs
use crate::plugins::player::Player;
use crate::plugins::sun::{Sun, SunSettings};

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<AtmosphereMaterial>::default())
           .add_systems(Startup, setup_atmosphere_visuals)
           .add_systems(Update, (
               atmospheric_drag_system.after(crate::plugins::player::player_gravity_system),
               update_atmosphere_visuals,
               update_atmosphere_material,
            ));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AtmosphereMaterial {
    #[uniform(0)]
    pub radii_params: Vec4, // x=planet_radius, y=atmosphere_radius, z=unused, w=unused
    #[uniform(0)]
    pub rayleigh_scattering_scale_height: Vec4, // xyz = scattering, w = scale height
    #[uniform(0)]
    pub mie_scattering_scale_height_asymmetry: Vec4, // x = scattering, y = scale height, z = asymmetry, w = unused
    #[uniform(0)]
    pub sun_position: Vec4, // xyz = pos, w = sun intensity
    #[uniform(0)]
    pub view_position: Vec4, // xyz = pos, w = unused
}

impl Material for AtmosphereMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/atmosphere.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component)]
struct AtmosphereShell;

fn setup_atmosphere_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtmosphereMaterial>>,
    planet_settings: Res<PlanetSettings>,
    sun_settings: Res<SunSettings>,
) {
    // Create Atmosphere Shell
    let atmosphere_radius = planet_settings.radius + planet_settings.atmosphere_height;

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(atmosphere_radius).mesh().ico(4).unwrap())),
        MeshMaterial3d(materials.add(AtmosphereMaterial {
            radii_params: Vec4::new(planet_settings.radius, atmosphere_radius, 0.0, 0.0),
            rayleigh_scattering_scale_height: planet_settings.rayleigh_scattering.extend(planet_settings.rayleigh_scale_height),
            mie_scattering_scale_height_asymmetry: Vec4::new(
                planet_settings.mie_scattering,
                planet_settings.mie_scale_height,
                planet_settings.mie_asymmetry,
                0.0
            ),
            sun_position: (Vec3::Y * 10000.0).extend(sun_settings.illuminance / 1000.0),
            view_position: Vec4::ZERO,
        })),
        Transform::default(),
        AtmosphereShell,
    ));
}

fn update_atmosphere_visuals(
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&Mesh3d, &mut Visibility), With<AtmosphereShell>>,
    planet_settings: Res<PlanetSettings>,
) {
    if planet_settings.is_changed() {
        for (mesh_handle, mut visibility) in query.iter_mut() {
            // Toggle visibility
            *visibility = if planet_settings.atmosphere_enabled {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };

            if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
                 let atmosphere_radius = planet_settings.radius + planet_settings.atmosphere_height;
                *mesh = Sphere::new(atmosphere_radius).mesh().ico(4).unwrap();
            }
        }
    }
}

fn update_atmosphere_material(
    mut materials: ResMut<Assets<AtmosphereMaterial>>,
    query: Query<&MeshMaterial3d<AtmosphereMaterial>, With<AtmosphereShell>>,
    planet_settings: Res<PlanetSettings>,
    sun_settings: Res<SunSettings>,
    camera_query: Query<&Transform, With<Camera3d>>,
    sun_query: Query<&Transform, (With<Sun>, Without<Camera3d>)>,
) {
    let Ok(camera_transform) = camera_query.single() else { return; };
    let Ok(sun_transform) = sun_query.single() else { return; };

    for handle in query.iter() {
        if let Some(material) = materials.get_mut(&handle.0) {
             material.radii_params = Vec4::new(planet_settings.radius, planet_settings.radius + planet_settings.atmosphere_height, 0.0, 0.0);
             material.rayleigh_scattering_scale_height = planet_settings.rayleigh_scattering.extend(planet_settings.rayleigh_scale_height);
             material.mie_scattering_scale_height_asymmetry = Vec4::new(
                 planet_settings.mie_scattering,
                 planet_settings.mie_scale_height,
                 planet_settings.mie_asymmetry,
                 0.0
             );
             material.sun_position = sun_transform.translation.extend(sun_settings.illuminance / 1000.0);
             material.view_position = camera_transform.translation.extend(0.0);
        }
    }
}

fn atmospheric_drag_system(
    mut query: Query<(&mut Velocity, &Transform), With<Player>>,
    planet_settings: Res<PlanetSettings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut velocity, transform) in query.iter_mut() {
        let distance_from_center = transform.translation.length();
        let altitude = distance_from_center - planet_settings.radius;
        
        if altitude < 0.0 || altitude > planet_settings.atmosphere_height * 2.0 {
            continue;
        }

        // Exponential atmosphere - Simplified for drag
        // Using scale height from Rayleigh setting for consistency or separate?
        // Let's use rayleigh_scale_height as "main" scale height.
        let density = planet_settings.air_density_sea_level * (-altitude / planet_settings.rayleigh_scale_height).exp();
        
        // Drag equation
        let speed = velocity.linvel.length();
        if speed > 0.001 {
            let drag_direction = -velocity.linvel.normalize();
            let drag_magnitude = 0.5 * density * speed * speed * 0.5; 
            
            // Apply drag directly to velocity
            velocity.linvel += drag_direction * drag_magnitude * dt;
        }
    }
}
