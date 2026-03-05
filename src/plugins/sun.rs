use bevy::prelude::*;
use bevy::light::GlobalAmbientLight;

pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SunSettings>()
           .add_systems(Startup, setup_sun)
           .add_systems(Update, update_sun_intensity);
    }
}

#[derive(Component)]
pub struct Sun;

#[derive(Component)]
struct SunLight; // Marker for the directional light

#[derive(Resource)]
pub struct SunSettings {
    pub illuminance: f32,
}

impl Default for SunSettings {
    fn default() -> Self {
        Self {
            illuminance: 8000.0,
        }
    }
}

fn setup_sun(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    sun_settings: Res<SunSettings>,
) {
    // Position sun inside the far plane (200,000) but far enough to look distant
    let sun_distance = 150_000.0;
    let sun_position = Vec3::new(sun_distance, sun_distance * 0.2, 0.0);

    // Sun Mesh
    // Angular size ~0.5 deg. Radius approx 1300.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1500.0).mesh().ico(4).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(1000.0, 900.0, 700.0, 100.0), // Extremely bright
            unlit: true,
            ..default()
        })),
        Transform::from_translation(sun_position),
        Sun,
    ));

    // Directional Light (Sunlight)
    commands.spawn((
        DirectionalLight {
            illuminance: sun_settings.illuminance,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(sun_position).looking_at(Vec3::ZERO, Vec3::Y),
        SunLight,
    ));

    // Ambient Light (Weak fill) — Bevy 0.18: AmbientLight is a Component, use GlobalAmbientLight resource
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 0.05, // Reduced from 0.1
        affects_lightmapped_meshes: true,
    });
}

fn update_sun_intensity(
    mut query: Query<&mut DirectionalLight, With<SunLight>>,
    sun_settings: Res<SunSettings>,
) {
    if sun_settings.is_changed() {
        for mut light in query.iter_mut() {
            light.illuminance = sun_settings.illuminance;
        }
    }
}
