// Creature AI System
// Bridges aggregate Lotka-Volterra populations from ecosystem.rs
// to individual creature entities the player can observe.
// Behavior trees drive emergent, watchable behavior.

pub mod behavior;
pub mod species;
pub mod meshes;

use bevy::prelude::*;
use std::collections::HashMap;

use crate::GameState;
use crate::plugins::ecosystem::{EcosystemState, SpeciesId, SpeciesType};
use crate::plugins::observation::Observable;
use crate::plugins::player::Player;

pub struct CreaturePlugin;

impl Plugin for CreaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreatureManager>()
            .add_systems(
                Update,
                (
                    spawn_creatures_from_populations,
                    update_creature_behaviors,
                    execute_creature_actions,
                    update_creature_observability,
                    despawn_distant_creatures,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

// ============================================================================
// COMPONENTS
// ============================================================================

/// Core creature component
#[derive(Component)]
pub struct Creature {
    pub species_id: SpeciesId,
    pub species_type: SpeciesType,
    pub hunger: f32,   // 0.0 = full, 1.0 = starving
    pub energy: f32,   // 0.0 = exhausted, 1.0 = full
    pub threat_level: f32,
    pub health: f32,   // 0.0 = dead, 1.0 = full
}

/// Movement state for a creature
#[derive(Component)]
pub struct CreatureMovement {
    pub target_position: Option<Vec3>,
    pub speed: f32,
    pub wander_center: Vec3,
    pub wander_radius: f32,
}

/// Behavior tree state for a creature
#[derive(Component)]
pub struct BehaviorState {
    pub tree: behavior::BehaviorNode,
    pub current_action: Option<behavior::CreatureAction>,
    pub action_timer: f32,
    pub action_cooldown: f32,
}

// ============================================================================
// RESOURCES
// ============================================================================

/// Species template for spawning creatures
pub struct SpeciesTemplate {
    pub species_id: SpeciesId,
    pub species_type: SpeciesType,
    pub name: String,
    pub speed: f32,
    pub wander_radius: f32,
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub scale: Vec3,
    pub behavior_tree: behavior::BehaviorNode,
}

/// Manages creature spawning, budgets, and templates
#[derive(Resource)]
pub struct CreatureManager {
    pub max_creatures: usize,
    pub spawn_radius: f32,
    pub despawn_radius: f32,
    pub ai_radius: f32,
    pub templates: HashMap<SpeciesId, SpeciesTemplate>,
    pub initialized: bool,
    pub spawn_timer: f32,
}

impl Default for CreatureManager {
    fn default() -> Self {
        Self {
            max_creatures: 200,
            spawn_radius: 150.0,
            despawn_radius: 250.0,
            ai_radius: 100.0,
            templates: HashMap::new(),
            initialized: false,
            spawn_timer: 0.0,
        }
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Spawn creature entities from aggregate population data
fn spawn_creatures_from_populations(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut manager: ResMut<CreatureManager>,
    ecosystem: Res<EcosystemState>,
    player_query: Query<&Transform, With<Player>>,
    creature_query: Query<&Creature>,
    planet_query: Query<Entity, With<crate::plugins::planet::Planet>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    // Only spawn periodically
    manager.spawn_timer += dt;
    if manager.spawn_timer < 2.0 {
        return;
    }
    manager.spawn_timer = 0.0;

    let Ok(player_transform) = player_query.single() else { return };
    let Ok(planet_entity) = planet_query.single() else { return };
    let player_pos = player_transform.translation;
    let current_count = creature_query.iter().count();

    if current_count >= manager.max_creatures {
        return;
    }

    // Initialize templates on first run
    if !manager.initialized {
        initialize_templates(&mut manager, &ecosystem, &mut meshes, &mut materials);
        manager.initialized = true;
    }

    // Spawn creatures proportional to species populations
    let spawn_budget = (manager.max_creatures - current_count).min(10); // max 10 per tick
    let mut spawned = 0;

    let planet_radius = player_pos.length(); // approximate

    for (species_id, species) in ecosystem.species.iter() {
        if spawned >= spawn_budget {
            break;
        }
        if species.population < 1.0 {
            continue;
        }

        // Skip producers — they're represented by vegetation
        if species.species_type == SpeciesType::Producer {
            continue;
        }

        let template = match manager.templates.get(species_id) {
            Some(t) => t,
            None => continue,
        };

        // How many of this species should exist near the player?
        // Scale by population relative to total, capped by budget
        let total_pop: f64 = ecosystem.species.values()
            .filter(|s| s.species_type != SpeciesType::Producer)
            .map(|s| s.population)
            .sum();
        let ratio = if total_pop > 0.0 { species.population / total_pop } else { 0.0 };
        let target_count = (ratio * manager.max_creatures as f64 * 0.5).ceil() as usize;

        // Count existing creatures of this species
        let existing = creature_query.iter()
            .filter(|c| c.species_id == *species_id)
            .count();

        if existing >= target_count {
            continue;
        }

        let to_spawn = (target_count - existing).min(spawn_budget - spawned);

        for _ in 0..to_spawn {
            // Random position near player on planet surface
            let offset_angle = spawned as f32 * 2.39996; // golden angle
            let offset_radius = manager.spawn_radius * 0.3 + (spawned as f32 * 7.0) % manager.spawn_radius;
            let player_up = player_pos.normalize();

            // Create a tangent vector for offset
            let tangent = if player_up.y.abs() > 0.9 {
                Vec3::X
            } else {
                Vec3::Y
            };
            let right = player_up.cross(tangent).normalize();
            let forward = player_up.cross(right).normalize();

            let offset = (right * offset_angle.cos() + forward * offset_angle.sin()) * offset_radius;
            let spawn_pos = (player_pos + offset).normalize() * planet_radius;

            let up = spawn_pos.normalize();
            let rotation = Quat::from_rotation_arc(Vec3::Y, up);

            commands.spawn((
                Mesh3d(template.mesh.clone()),
                MeshMaterial3d(template.material.clone()),
                Transform::from_translation(spawn_pos)
                    .with_rotation(rotation)
                    .with_scale(template.scale),
                Creature {
                    species_id: *species_id,
                    species_type: template.species_type.clone(),
                    hunger: 0.3,
                    energy: 0.8,
                    threat_level: 0.0,
                    health: 1.0,
                },
                CreatureMovement {
                    target_position: None,
                    speed: template.speed,
                    wander_center: spawn_pos,
                    wander_radius: template.wander_radius,
                },
                BehaviorState {
                    tree: template.behavior_tree.clone(),
                    current_action: None,
                    action_timer: 0.0,
                    action_cooldown: 0.0,
                },
                ChildOf(planet_entity),
            ));

            spawned += 1;
        }
    }

    if spawned > 0 {
        info!("[CREATURES] Spawned {} creatures (total: {})", spawned, current_count + spawned);
    }
}

/// Evaluate behavior trees for creatures within AI radius
fn update_creature_behaviors(
    mut creature_query: Query<(&Creature, &Transform, &mut BehaviorState)>,
    player_query: Query<&Transform, With<Player>>,
    other_creatures: Query<(&Creature, &Transform), Without<Player>>,
    manager: Res<CreatureManager>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation;

    for (creature, transform, mut behavior) in creature_query.iter_mut() {
        let pos = transform.translation;
        let distance_to_player = (pos - player_pos).length();

        // Only run AI for creatures within AI radius
        if distance_to_player > manager.ai_radius {
            continue;
        }

        // Cooldown
        if behavior.action_cooldown > 0.0 {
            behavior.action_cooldown -= dt;
            continue;
        }

        // Threat from player proximity
        let player_threat = if distance_to_player < 15.0 {
            (1.0 - distance_to_player / 15.0) * 0.8
        } else {
            0.0
        };

        // Check for nearby prey/flock
        let mut near_prey = false;
        let mut prey_distance = f32::MAX;
        let mut near_flock = false;
        let mut flock_distance = f32::MAX;

        for (other, other_transform) in other_creatures.iter() {
            let dist = (pos - other_transform.translation).length();

            if creature.species_type == SpeciesType::Carnivore
                && other.species_type == SpeciesType::Herbivore
                && dist < 50.0
            {
                near_prey = true;
                prey_distance = prey_distance.min(dist);
            }

            if other.species_id == creature.species_id && dist < 30.0 && dist > 0.1 {
                near_flock = true;
                flock_distance = flock_distance.min(dist);
            }
        }

        let ctx = behavior::BehaviorContext {
            hunger: creature.hunger,
            energy: creature.energy,
            threat_level: creature.threat_level.max(player_threat),
            near_prey,
            near_flock,
            prey_distance,
            flock_distance,
        };

        let (_, action) = behavior::evaluate(&behavior.tree, &ctx);
        behavior.current_action = action;
        behavior.action_timer = 0.0;
    }
}

/// Translate behavior decisions to creature movement
fn execute_creature_actions(
    mut creature_query: Query<(
        &mut Creature,
        &mut CreatureMovement,
        &mut Transform,
        &BehaviorState,
    )>,
    player_query: Query<&Transform, (With<Player>, Without<Creature>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation;

    for (mut creature, mut movement, mut transform, behavior) in creature_query.iter_mut() {
        let pos = transform.translation;
        let up = pos.normalize();
        let planet_radius = pos.length();

        // Natural hunger/energy cycles
        creature.hunger = (creature.hunger + dt * 0.02).min(1.0);
        creature.energy = (creature.energy - dt * 0.01).max(0.0);

        let Some(action) = &behavior.current_action else {
            continue;
        };

        match action {
            behavior::CreatureAction::Wander => {
                // Pick a random-ish target near wander center
                if movement.target_position.is_none() {
                    let angle = (pos.x * 100.0 + dt * 37.0).sin() * std::f32::consts::TAU;
                    let offset_dist = movement.wander_radius * 0.5;
                    let tangent = if up.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
                    let right = up.cross(tangent).normalize();
                    let fwd = up.cross(right).normalize();
                    let offset = (right * angle.cos() + fwd * angle.sin()) * offset_dist;
                    let target = (movement.wander_center + offset).normalize() * planet_radius;
                    movement.target_position = Some(target);
                }
            }
            behavior::CreatureAction::Forage => {
                // Move slowly, reduce hunger
                creature.hunger = (creature.hunger - dt * 0.05).max(0.0);
                creature.energy = (creature.energy + dt * 0.02).min(1.0);
            }
            behavior::CreatureAction::FleeFrom => {
                // Move away from player
                let away = (pos - player_pos).normalize();
                let flee_target = (pos + away * 30.0).normalize() * planet_radius;
                movement.target_position = Some(flee_target);
            }
            behavior::CreatureAction::HuntPrey => {
                // Move faster toward prey (simplified: move toward wander center)
                // Full implementation would find nearest herbivore
            }
            behavior::CreatureAction::Rest => {
                // Stay still, recover energy
                creature.energy = (creature.energy + dt * 0.1).min(1.0);
                movement.target_position = None;
            }
            behavior::CreatureAction::Socialize | behavior::CreatureAction::CallOut => {
                // Gentle movement, reduce threat
                creature.threat_level = (creature.threat_level - dt * 0.1).max(0.0);
            }
            _ => {}
        }

        // Move toward target
        if let Some(target) = movement.target_position {
            let to_target = target - pos;
            let dist = to_target.length();

            if dist < 1.0 {
                movement.target_position = None;
            } else {
                let move_dir = to_target.normalize();
                let speed = match action {
                    behavior::CreatureAction::FleeFrom => movement.speed * 2.0,
                    behavior::CreatureAction::HuntPrey => movement.speed * 1.5,
                    behavior::CreatureAction::Rest => 0.0,
                    _ => movement.speed,
                };
                let new_pos = pos + move_dir * speed * dt;
                // Keep on planet surface
                transform.translation = new_pos.normalize() * planet_radius;
                // Align to surface
                let new_up = transform.translation.normalize();
                transform.rotation = Quat::from_rotation_arc(Vec3::Y, new_up);
            }
        }
    }
}

/// Attach Observable component to creatures that don't have one
fn update_creature_observability(
    mut commands: Commands,
    creature_query: Query<(Entity, &Creature), Without<Observable>>,
) {
    for (entity, creature) in creature_query.iter() {
        if let Some(phenomenon) = species::observable_phenomenon(&creature.species_type) {
            let threshold = species::observation_threshold(&creature.species_type);
            let description = match creature.species_type {
                SpeciesType::Herbivore => "Grazing animals depend on plant life".to_string(),
                SpeciesType::Carnivore => "Predators chase prey — action and reaction".to_string(),
                SpeciesType::Omnivore => "Adaptable creatures find food everywhere".to_string(),
                _ => "Life persists through cycles of growth and decay".to_string(),
            };

            commands.entity(entity).insert(Observable {
                phenomenon,
                threshold,
                description,
            });
        }
    }
}

/// Remove creatures that are too far from the player
fn despawn_distant_creatures(
    mut commands: Commands,
    creature_query: Query<(Entity, &Transform), With<Creature>>,
    player_query: Query<&Transform, With<Player>>,
    manager: Res<CreatureManager>,
) {
    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation;

    for (entity, transform) in creature_query.iter() {
        let distance = (transform.translation - player_pos).length();
        if distance > manager.despawn_radius {
            commands.entity(entity).despawn();
        }
    }
}

// ============================================================================
// HELPERS
// ============================================================================

fn initialize_templates(
    manager: &mut CreatureManager,
    ecosystem: &EcosystemState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    for (species_id, species_data) in ecosystem.species.iter() {
        if species_data.species_type == SpeciesType::Producer {
            continue; // Plants are handled by vegetation system
        }

        let mesh = meshes.add(meshes::creature_mesh(&species_data.species_type));
        let material = materials.add(StandardMaterial {
            base_color: meshes::creature_color(&species_data.species_type),
            ..default()
        });
        let scale = meshes::creature_scale(&species_data.species_type);
        let behavior_tree = species::behavior_for_species(&species_data.species_type);

        let speed = match species_data.species_type {
            SpeciesType::Herbivore => 5.0,
            SpeciesType::Carnivore => 8.0,
            SpeciesType::Omnivore => 6.0,
            SpeciesType::Decomposer => 2.0,
            _ => 1.0,
        };

        manager.templates.insert(*species_id, SpeciesTemplate {
            species_id: *species_id,
            species_type: species_data.species_type.clone(),
            name: species_data.name.clone(),
            speed,
            wander_radius: 40.0,
            mesh,
            material,
            scale,
            behavior_tree,
        });
    }
}
