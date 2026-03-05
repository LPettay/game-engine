use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Rapier 0.33: RapierConfiguration is a Component on the context entity, not a Resource.
        // Use RapierPhysicsPlugin with default() and configure via system after context entity is spawned.
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
           .add_systems(Startup, configure_rapier);
    }
}

fn configure_rapier(
    mut config: Query<&mut RapierConfiguration, With<DefaultRapierContext>>,
) {
    if let Ok(mut config) = config.single_mut() {
        config.gravity = Vec3::ZERO; // We handle gravity manually in player.rs
        config.physics_pipeline_active = true;
        config.scaled_shape_subdivision = 10;
        // Ensure colliders update when transforms change (critical for rotating planet)
        config.force_update_from_transform_changes = true;
    }
}
