// Da Vinci Observation System
// Players discover phenomena by sustained, quality observation of the world.
// No UI progress bars — the world teaches through direct attention.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::GameState;
use crate::plugins::discovery::{
    DiscoveryEvent, DiscoveryMethod, PhenomenonId, PlayerKnowledge, TechTree,
};
use crate::plugins::player::Player;
use crate::plugins::vegetation::Tree;

pub struct ObservationPlugin;

impl Plugin for ObservationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObservationTracker>()
            .add_systems(
                Update,
                (
                    tag_vegetation_observable,
                    update_observation_focus,
                    accumulate_observation_insight,
                    check_observation_discoveries,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnEnter(GameState::Playing), spawn_observation_points);
    }
}

// ============================================================================
// COMPONENTS
// ============================================================================

/// Identifies which player this entity represents (for multi-player lookup)
#[derive(Component)]
pub struct PlayerId(pub u64);

/// Tracks what the player is currently observing
#[derive(Component, Default)]
pub struct ObservationFocus {
    /// Entity being observed
    pub target: Option<Entity>,
    /// How long the current target has been gazed at (seconds)
    pub gaze_duration: f32,
    /// Quality of observation — movement penalizes (0.0–1.0)
    pub stillness: f32,
    /// Previous frame position for movement detection
    pub last_position: Option<Vec3>,
}

/// Marks a world entity as something that can be observed to trigger discovery
#[derive(Component)]
pub struct Observable {
    /// Which phenomenon observing this teaches
    pub phenomenon: PhenomenonId,
    /// How much accumulated insight is needed for discovery
    pub threshold: f32,
    /// Flavor text describing what the player notices
    pub description: String,
}

/// Auto-generated journal of observation-based discoveries
#[derive(Component, Default)]
pub struct ObservationJournal {
    pub entries: Vec<JournalEntry>,
    pub show: bool,
}

pub struct JournalEntry {
    pub text: String,
    pub timestamp: f32,
    pub phenomenon: PhenomenonId,
}

/// Marker for spawned observation point entities (campfires, rocks, etc.)
#[derive(Component)]
pub struct ObservationPoint;

// ============================================================================
// RESOURCES
// ============================================================================

/// Tracks accumulated insight per (entity, phenomenon) pair
#[derive(Resource, Default)]
pub struct ObservationTracker {
    pub insight: HashMap<(Entity, PhenomenonId), f32>,
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Determine what the player is looking at based on camera direction
fn update_observation_focus(
    mut player_query: Query<(&Transform, &mut ObservationFocus), With<Player>>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    observable_query: Query<(Entity, &GlobalTransform), With<Observable>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let Ok((player_transform, mut focus)) = player_query.single_mut() else {
        return;
    };
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };

    // Update stillness based on player movement
    let current_pos = player_transform.translation;
    if let Some(last_pos) = focus.last_position {
        let movement_speed = (current_pos - last_pos).length() / dt.max(0.001);
        if movement_speed < 0.5 {
            focus.stillness = (focus.stillness + dt * 0.5).min(1.0);
        } else {
            focus.stillness = (focus.stillness - dt * movement_speed * 0.2).max(0.0);
        }
    }
    focus.last_position = Some(current_pos);

    // Camera forward direction in world space
    let (_, cam_rotation, cam_pos) = camera_gt.to_scale_rotation_translation();
    let cam_forward = cam_rotation * Vec3::NEG_Z;

    let observation_range = 80.0;
    let view_cone_cos = 0.9; // ~25 degree half-angle

    // Find the closest observable entity in the view cone
    let mut best_target: Option<(Entity, f32)> = None;

    for (entity, global_transform) in observable_query.iter() {
        let target_pos = global_transform.translation();
        let to_target = target_pos - cam_pos;
        let distance = to_target.length();

        if distance > observation_range || distance < 0.1 {
            continue;
        }

        let direction = to_target / distance;
        let dot = cam_forward.dot(direction);

        if dot > view_cone_cos {
            if best_target.is_none() || distance < best_target.unwrap().1 {
                best_target = Some((entity, distance));
            }
        }
    }

    // Update focus target
    match (best_target, focus.target) {
        (Some((entity, _)), Some(prev)) if entity == prev => {
            // Same target — accumulate gaze
            focus.gaze_duration += dt;
        }
        (Some((entity, _)), _) => {
            // New target
            focus.target = Some(entity);
            focus.gaze_duration = 0.0;
        }
        (None, _) => {
            // Nothing in view
            focus.target = None;
            focus.gaze_duration = 0.0;
        }
    }
}

/// Accumulate insight toward discovery when observing something
fn accumulate_observation_insight(
    player_query: Query<&ObservationFocus, With<Player>>,
    observable_query: Query<&Observable>,
    mut tracker: ResMut<ObservationTracker>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let Ok(focus) = player_query.single() else {
        return;
    };

    let Some(target) = focus.target else {
        return;
    };
    let Ok(observable) = observable_query.get(target) else {
        return;
    };

    // Insight rate = stillness × gaze_ramp × (1 / difficulty)
    let gaze_ramp = (focus.gaze_duration / 2.0).min(1.0); // ramps up over 2 seconds
    let difficulty = observable.threshold.max(1.0);
    let insight_rate = focus.stillness * gaze_ramp / difficulty;

    let key = (target, observable.phenomenon);
    let accumulated = tracker.insight.entry(key).or_insert(0.0);
    *accumulated += insight_rate * dt;
}

/// Fire DiscoveryEvent when accumulated insight crosses the threshold
fn check_observation_discoveries(
    player_query: Query<(&ObservationFocus, &PlayerKnowledge, &PlayerId), With<Player>>,
    observable_query: Query<&Observable>,
    mut tracker: ResMut<ObservationTracker>,
    mut discovery_events: MessageWriter<DiscoveryEvent>,
    mut journal_query: Query<&mut ObservationJournal, With<Player>>,
    tech_tree: Res<TechTree>,
    time: Res<Time>,
) {
    let Ok((focus, knowledge, player_id)) = player_query.single() else {
        return;
    };
    let Some(target) = focus.target else {
        return;
    };
    let Ok(observable) = observable_query.get(target) else {
        return;
    };

    let key = (target, observable.phenomenon);
    let Some(accumulated) = tracker.insight.get(&key).copied() else {
        return;
    };

    // Already discovered — skip
    if knowledge.discovered.contains(&observable.phenomenon) {
        return;
    }

    if accumulated >= observable.threshold {
        let phenomenon_name = tech_tree
            .phenomena
            .get(&observable.phenomenon)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Phenomenon {:?}", observable.phenomenon));

        info!(
            "[OBSERVATION] Discovered '{}' through sustained observation!",
            phenomenon_name
        );

        discovery_events.write(DiscoveryEvent {
            player: player_id.0,
            phenomenon: observable.phenomenon,
            method: DiscoveryMethod::Observation {
                condition: observable.description.clone(),
            },
            is_first: true,
        });

        // Journal entry
        if let Ok(mut journal) = journal_query.single_mut() {
            journal.entries.push(JournalEntry {
                text: format!(
                    "Through patient observation I understood: {}",
                    observable.description
                ),
                timestamp: time.elapsed_secs(),
                phenomenon: observable.phenomenon,
            });
        }

        // Clear accumulated insight for this target
        tracker.insight.remove(&key);
    }
}

/// Attach Observable component to trees that don't have one yet
fn tag_vegetation_observable(
    mut commands: Commands,
    tree_query: Query<Entity, (With<Tree>, Without<Observable>)>,
) {
    for entity in tree_query.iter() {
        commands.entity(entity).insert(Observable {
            phenomenon: PhenomenonId(3), // PlantGrowth
            threshold: 8.0,
            description: "Plants grow from seeds with water and sunlight".to_string(),
        });
    }
}

/// Seed environmental observation points on the planet surface
fn spawn_observation_points(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    planet_query: Query<Entity, With<crate::plugins::planet::Planet>>,
    planet_settings: Res<crate::plugins::planet::PlanetSettings>,
) {
    let Ok(planet_entity) = planet_query.single() else {
        return;
    };
    let terrain_noise = crate::plugins::terrain::TerrainNoise::new(planet_settings.terrain_seed);

    // Campfire mesh — small glowing cube
    let campfire_mesh = meshes.add(Cuboid::new(1.0, 0.5, 1.0));
    let campfire_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.4, 0.05),
        ..default()
    });

    // Rock mesh — irregular gray block
    let rock_mesh = meshes.add(Cuboid::new(2.0, 3.0, 1.5));
    let rock_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });

    // Campfire positions (normalized directions on planet surface)
    let campfire_dirs: &[Vec3] = &[
        Vec3::new(0.3, 0.9, 0.2).normalize(),
        Vec3::new(-0.5, 0.8, 0.3).normalize(),
        Vec3::new(0.1, 0.85, -0.5).normalize(),
    ];

    for dir in campfire_dirs {
        let elevation = terrain_noise.get_elevation(*dir);
        if elevation < 0.0 {
            continue;
        }
        let height_mult = 1.0 + elevation * 0.1;
        let pos = *dir * planet_settings.radius * height_mult;
        let rotation = Quat::from_rotation_arc(Vec3::Y, pos.normalize());

        commands.spawn((
            Mesh3d(campfire_mesh.clone()),
            MeshMaterial3d(campfire_material.clone()),
            Transform::from_translation(pos).with_rotation(rotation),
            Observable {
                phenomenon: PhenomenonId(0), // Fire
                threshold: 5.0,
                description: "Combustion releases heat and light".to_string(),
            },
            ObservationPoint,
            ChildOf(planet_entity),
        ));
    }

    // Rock positions for Gravity observation
    let rock_dirs: &[Vec3] = &[
        Vec3::new(0.6, 0.7, 0.3).normalize(),
        Vec3::new(-0.3, 0.75, 0.6).normalize(),
        Vec3::new(0.4, 0.8, -0.4).normalize(),
        Vec3::new(-0.6, 0.65, -0.3).normalize(),
    ];

    for dir in rock_dirs {
        let elevation = terrain_noise.get_elevation(*dir);
        if elevation < 0.02 {
            continue;
        }
        let height_mult = 1.0 + elevation * 0.1;
        let pos = *dir * planet_settings.radius * height_mult;
        let rotation = Quat::from_rotation_arc(Vec3::Y, pos.normalize());

        commands.spawn((
            Mesh3d(rock_mesh.clone()),
            MeshMaterial3d(rock_material.clone()),
            Transform::from_translation(pos).with_rotation(rotation),
            Observable {
                phenomenon: PhenomenonId(1), // Gravity
                threshold: 4.0,
                description: "Objects fall toward massive bodies".to_string(),
            },
            ObservationPoint,
            ChildOf(planet_entity),
        ));
    }

    info!("[OBSERVATION] Spawned observation points on planet surface");
}
