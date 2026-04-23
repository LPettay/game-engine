use bevy::prelude::*;

use genesis_engine::plugins;
use genesis_engine::physics;
use genesis_engine::net;
use genesis_engine::GameState;

use plugins::{
    planet::PlanetPlugin,
    camera::CameraPlugin,
    player::PlayerPlugin,
    physics::PhysicsPlugin,
    terrain::TerrainPlugin,
    gpu_terrain::GpuTerrainPlugin,
    simulation::SimulationPlugin,
    speculative_physics::SpeculativePhysicsPlugin,
    discovery::DiscoveryPlugin,
    geology::GeologyPlugin,
    climate::ClimatePlugin,
    ecosystem::EcosystemPlugin,
    technology::TechnologyPlugin,
    economy::EconomyPlugin,
    server::ServerPlugin,
    observation::ObservationPlugin,
    creature::CreaturePlugin,
    voxel::VoxelTerrainPlugin,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        // Core simulation systems
        .add_plugins((
            SimulationPlugin,
            physics::UniversePhysicsPlugin,
            PlanetPlugin,
            TerrainPlugin,
            plugins::chunked_terrain::ChunkedTerrainPlugin,
            GpuTerrainPlugin,
        ))
        // Player and world systems
        .add_plugins((
            CameraPlugin,
            PlayerPlugin,
            PhysicsPlugin,
            plugins::atmosphere::AtmospherePlugin,
            plugins::sun::SunPlugin,
            plugins::vegetation::VegetationPlugin,
        ))
        // World simulation systems
        .add_plugins((
            GeologyPlugin,
            ClimatePlugin,
            EcosystemPlugin,
        ))
        // Progression and discovery systems
        .add_plugins((
            DiscoveryPlugin,
            SpeculativePhysicsPlugin,
            TechnologyPlugin,
            EconomyPlugin,
            ServerPlugin,
            ObservationPlugin,
        ))
        // Creature and terrain systems
        .add_plugins((
            CreaturePlugin,
            VoxelTerrainPlugin,
            net::NetPlugin,
        ))
        // UI systems
        .add_plugins((
            plugins::ui::UiPlugin,
            plugins::trajectory::TrajectoryPlugin,
            plugins::telemetry::TelemetryPlugin,
            plugins::indicator::IndicatorPlugin,
        ))
        // Agent diagnostics
        .add_plugins(plugins::agent_eyes::AgentEyesPlugin)
        .init_state::<GameState>()
        .run();
}
