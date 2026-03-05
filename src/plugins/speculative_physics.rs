// Speculative Physics Module
// Infinite Universe Engine - Hidden physics layer for beyond-human discoveries
//
// This module implements physics mechanics that extend beyond current human knowledge,
// forming the basis for the infinite tech tree and multiverse exploration.
//
// Design philosophy:
// - Hidden by default - players must discover through experimentation
// - Internally consistent - speculative mechanics follow their own rules
// - Progressive revelation - each discovery hints at deeper mysteries
// - Community-driven discovery - first-discoverers create lasting legacy

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use crate::physics::GravitationalBody;

pub struct SpeculativePhysicsPlugin;

impl Plugin for SpeculativePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpeculativePhysicsState>()
           .init_resource::<BlackHoleRegistry>()
           .init_resource::<MultiverseState>()
           .add_message::<SpeculativeDiscoveryEvent>()
           .add_systems(Update, (
               update_black_hole_physics,
               check_traversal_conditions,
               process_speculative_discoveries,
           ).chain());
    }
}

/// State of speculative physics systems
#[derive(Resource, Default)]
pub struct SpeculativePhysicsState {
    /// Which hidden mechanics have been globally discovered
    pub global_discoveries: HashSet<HiddenMechanicId>,
    /// Per-player discovery state (for multiplayer)
    pub player_discoveries: HashMap<u64, PlayerDiscoveryState>,
    /// Whether to show hints about hidden mechanics
    pub hints_enabled: bool,
    /// Difficulty of discovery (multiplier on conditions)
    pub discovery_difficulty: f64,
}

impl SpeculativePhysicsState {
    pub fn new(difficulty: f64) -> Self {
        Self {
            discovery_difficulty: difficulty,
            hints_enabled: true,
            ..default()
        }
    }
}

/// Player's personal discovery progress
#[derive(Clone, Debug, Default)]
pub struct PlayerDiscoveryState {
    /// Mechanics this player has discovered
    pub discovered: HashSet<HiddenMechanicId>,
    /// Observations that hint at hidden mechanics
    pub observations: Vec<SpeculativeObservation>,
    /// Failed experiments (useful for community knowledge)
    pub failed_experiments: Vec<FailedExperiment>,
    /// Successful experiments
    pub successful_experiments: Vec<SuccessfulExperiment>,
}

/// Unique identifier for hidden mechanics
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HiddenMechanicId {
    // Tier 5: Relativistic Effects
    TimeDilationMastery,
    LengthContraction,
    MassEnergyEquivalence,
    
    // Tier 6: Quantum Mechanics
    QuantumTunneling,
    QuantumEntanglement,
    WaveFunctionCollapse,
    
    // Tier 7: Black Hole Physics
    EventHorizonSurvival,
    HawkingRadiation,
    BlackHoleThermodynamics,
    
    // Tier 8: Exotic Matter
    NegativeEnergy,
    ExoticMatterSynthesis,
    WarpBubbleFormation,
    
    // Tier 9: Wormholes
    WormholeStabilization,
    TraversableWormhole,
    WormholeConstruction,
    
    // Tier 10+: Ultimate Secrets
    ClosedTimelikeCurves,
    UniverseConstantManipulation,
    MultiverseAwareness,
    RealityEditing,
}

/// The legendary black hole traversal secret
/// This is the crown jewel of the discovery system
#[derive(Clone, Debug)]
pub struct BlackHoleTraversalConditions {
    /// Required velocity relative to black hole (as fraction of c)
    /// Optimal range: 0.9c - 0.95c
    pub velocity_range: (f64, f64),
    
    /// Required angle to spin axis (radians)
    /// Must be within 5 degrees of spin axis
    pub approach_angle_tolerance: f64,
    
    /// Maximum survivable acceleration during transit (g)
    /// Must stay below 10g
    pub max_acceleration: f64,
    
    /// Minimum spin parameter of black hole (a/M)
    /// Requires Kerr black hole with sufficient spin
    pub min_spin_parameter: f64,
    
    /// Required trajectory through ergosphere
    pub ergosphere_path: ErgospherePath,
}

impl Default for BlackHoleTraversalConditions {
    fn default() -> Self {
        Self {
            velocity_range: (0.9, 0.95),
            approach_angle_tolerance: 5.0_f64.to_radians(),
            max_acceleration: 10.0, // 10g max
            min_spin_parameter: 0.9, // Nearly maximally spinning
            ergosphere_path: ErgospherePath::ProgradeEquatorial,
        }
    }
}

/// Path through the ergosphere (required for traversal)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ErgospherePath {
    /// Co-rotating with black hole spin (most stable)
    #[default]
    ProgradeEquatorial,
    /// Counter-rotating (extremely dangerous)
    RetrogradeEquatorial,
    /// Polar approach (near spin axis)
    PolarApproach,
}

/// A single black hole in the universe
#[derive(Component, Clone, Debug)]
pub struct BlackHole {
    /// Mass in kg
    pub mass: f64,
    /// Spin parameter (0 = Schwarzschild, 1 = maximum Kerr)
    pub spin_parameter: f64,
    /// Spin axis direction
    pub spin_axis: Vec3,
    /// Has this black hole been successfully traversed?
    pub traversed: bool,
    /// Destination universe seed (if traversable)
    pub destination_seed: Option<u64>,
    /// Number of failed traversal attempts
    pub failed_attempts: u32,
    /// ID of first successful traverser (for legacy)
    pub first_traverser: Option<u64>,
}

impl BlackHole {
    /// Calculate Schwarzschild radius
    pub fn event_horizon_radius(&self, g: f64, c: f64) -> f64 {
        2.0 * g * self.mass / (c * c)
    }
    
    /// Calculate ergosphere outer radius (at equator)
    pub fn ergosphere_radius(&self, g: f64, c: f64) -> f64 {
        let rs = self.event_horizon_radius(g, c);
        rs * (1.0 + (1.0 - self.spin_parameter * self.spin_parameter).sqrt())
    }
    
    /// Calculate innermost stable circular orbit
    pub fn isco_radius(&self, g: f64, c: f64) -> f64 {
        let rs = self.event_horizon_radius(g, c);
        // Simplified - full calculation depends on spin
        if self.spin_parameter > 0.0 {
            rs * (3.0 - 2.0 * self.spin_parameter) // Prograde
        } else {
            rs * 3.0 // Schwarzschild
        }
    }
    
    /// Check if an approach matches traversal conditions
    pub fn check_traversal_attempt(
        &self,
        conditions: &BlackHoleTraversalConditions,
        velocity: Vec3,
        position: Vec3,
        c: f64,
    ) -> TraversalResult {
        // Check velocity magnitude
        let v_magnitude = velocity.length() as f64;
        let v_fraction = v_magnitude / c;
        
        if v_fraction < conditions.velocity_range.0 {
            return TraversalResult::Failed(TraversalFailure::VelocityTooLow);
        }
        if v_fraction > conditions.velocity_range.1 {
            return TraversalResult::Failed(TraversalFailure::VelocityTooHigh);
        }
        
        // Check approach angle to spin axis
        let approach_dir = (-position).normalize();
        let angle_to_axis = approach_dir.angle_between(self.spin_axis);
        let angle_from_axis = angle_to_axis.min(std::f32::consts::PI - angle_to_axis);
        
        if angle_from_axis > conditions.approach_angle_tolerance as f32 {
            return TraversalResult::Failed(TraversalFailure::AngleTooWide);
        }
        
        // Check spin parameter
        if self.spin_parameter < conditions.min_spin_parameter {
            return TraversalResult::Failed(TraversalFailure::InsufficientSpin);
        }
        
        // All conditions met!
        TraversalResult::Success {
            destination_seed: self.destination_seed.unwrap_or_else(|| {
                // Generate pseudo-random seed based on system time
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(12345)
            }),
        }
    }
}

/// Result of a black hole traversal attempt
#[derive(Clone, Debug)]
pub enum TraversalResult {
    Success { destination_seed: u64 },
    Failed(TraversalFailure),
}

/// Reasons a traversal can fail
#[derive(Clone, Debug)]
pub enum TraversalFailure {
    VelocityTooLow,
    VelocityTooHigh,
    AngleTooWide,
    InsufficientSpin,
    AccelerationExceeded,
    SpaghettificationFatal,
    ErgospherePerturbation,
}

impl TraversalFailure {
    /// Get a hint message for failed attempts (without revealing solution)
    pub fn hint_message(&self) -> &'static str {
        match self {
            Self::VelocityTooLow => "The gravitational pull was too strong to escape.",
            Self::VelocityTooHigh => "The extreme velocity caused catastrophic tidal stresses.",
            Self::AngleTooWide => "Something about the approach angle felt wrong...",
            Self::InsufficientSpin => "The black hole's rotation wasn't significant enough.",
            Self::AccelerationExceeded => "The acceleration exceeded survivable limits.",
            Self::SpaghettificationFatal => "Tidal forces exceeded structural integrity.",
            Self::ErgospherePerturbation => "Chaotic forces in the ergosphere were uncontrollable.",
        }
    }
}

/// Registry of all black holes in the current universe
#[derive(Resource, Default)]
pub struct BlackHoleRegistry {
    pub black_holes: HashMap<Entity, BlackHole>,
    pub traversal_conditions: BlackHoleTraversalConditions,
    pub total_failed_attempts: u64,
    pub total_successful_traversals: u32,
}

/// Multiverse state - connected universes
#[derive(Resource, Default)]
pub struct MultiverseState {
    /// Current universe seed
    pub current_universe_seed: u64,
    /// Known connected universes (via successful traversals)
    pub known_universes: Vec<UniverseInfo>,
    /// Active portals (bidirectional once established)
    pub active_portals: Vec<MultiversePortal>,
}

/// Information about a discovered universe
#[derive(Clone, Debug)]
pub struct UniverseInfo {
    pub seed: u64,
    pub discovered_by: u64, // Player ID
    pub discovered_at: f64, // Universe time
    pub physical_constants_variation: f64, // How different from "normal"
    pub notable_features: Vec<String>,
}

/// A portal between universes
#[derive(Clone, Debug)]
pub struct MultiversePortal {
    pub source_universe: u64,
    pub destination_universe: u64,
    pub source_black_hole: Entity,
    pub established_at: f64,
    pub traversals: u32,
}

/// Speculative observation that hints at hidden mechanics
#[derive(Clone, Debug)]
pub struct SpeculativeObservation {
    pub mechanic: HiddenMechanicId,
    pub observation_type: ObservationType,
    pub timestamp: f64,
    pub location: Vec3,
    pub details: String,
}

#[derive(Clone, Debug)]
pub enum ObservationType {
    Anomaly,      // Something that doesn't fit known physics
    Measurement,  // Precise measurement that reveals hidden effect
    Accident,     // Accidental discovery
    Experiment,   // Result of deliberate experiment
}

/// Failed experiment record (for community knowledge)
#[derive(Clone, Debug)]
pub struct FailedExperiment {
    pub mechanic: HiddenMechanicId,
    pub hypothesis: String,
    pub procedure: String,
    pub result: String,
    pub timestamp: f64,
}

/// Successful experiment record
#[derive(Clone, Debug)]
pub struct SuccessfulExperiment {
    pub mechanic: HiddenMechanicId,
    pub procedure: String,
    pub result: String,
    pub timestamp: f64,
    pub reproducible: bool,
}

/// Event when a speculative mechanic is discovered
#[derive(Message)]
pub struct SpeculativeDiscoveryEvent {
    pub mechanic: HiddenMechanicId,
    pub discoverer: Option<u64>, // Player ID, None for NPCs
    pub method: DiscoveryMethod,
    pub timestamp: f64,
    pub first_discovery: bool, // Is this the first time anyone discovered it?
}

#[derive(Clone, Debug)]
pub enum DiscoveryMethod {
    Observation,
    Experiment,
    Accident,
    Teaching, // Learned from another player
    Research, // Read from documentation
}

/// System to update black hole physics
fn update_black_hole_physics(
    time: Res<Time>,
    mut black_holes: Query<(&BlackHole, &GlobalTransform)>,
    nearby_objects: Query<(&GlobalTransform, Option<&GravitationalBody>)>,
) {
    // This system would apply tidal forces, frame dragging, etc.
    // For now, just a placeholder for the physics calculations
    
    for (bh, bh_transform) in black_holes.iter() {
        let bh_pos = bh_transform.translation();
        
        // Apply gravitational effects to nearby objects
        for (obj_transform, _grav_body) in nearby_objects.iter() {
            let obj_pos = obj_transform.translation();
            let distance = (obj_pos - bh_pos).length();
            
            // Calculate relativistic effects based on distance
            // This would modify time flow, apply tidal forces, etc.
            if distance < 1e-6 {
                continue; // Too close, would cause numerical issues
            }
        }
    }
}

/// System to check if any entity is meeting traversal conditions
fn check_traversal_conditions(
    mut commands: Commands,
    registry: Res<BlackHoleRegistry>,
    mut speculative_state: ResMut<SpeculativePhysicsState>,
    mut multiverse: ResMut<MultiverseState>,
    mut discovery_events: MessageWriter<SpeculativeDiscoveryEvent>,
    black_holes: Query<(Entity, &BlackHole, &GlobalTransform)>,
    potential_traversers: Query<(Entity, &GlobalTransform, &GravitationalBody)>,
) {
    // Check if any entity with velocity is approaching a black hole correctly
    // This is the core mechanic for multiverse discovery
    
    // Speed of light (from constants)
    let c = 299_792_458.0;
    
    for (bh_entity, black_hole, bh_transform) in black_holes.iter() {
        let bh_pos = bh_transform.translation();
        
        for (traverser_entity, obj_transform, grav_body) in potential_traversers.iter() {
            let obj_pos = obj_transform.translation();
            let relative_pos = obj_pos - bh_pos;
            let distance = relative_pos.length();
            
            // Check if close enough to attempt traversal
            // (within 10x Schwarzschild radius)
            let rs = black_hole.event_horizon_radius(6.674e-11, c) as f32;
            if distance > rs * 10.0 {
                continue;
            }
            
            // Check traversal conditions
            let velocity = Vec3::new(
                grav_body.velocity.x as f32,
                grav_body.velocity.y as f32,
                grav_body.velocity.z as f32,
            );
            
            let result = black_hole.check_traversal_attempt(
                &registry.traversal_conditions,
                velocity,
                relative_pos,
                c,
            );
            
            match result {
                TraversalResult::Success { destination_seed } => {
                    // Epic success! First multiverse traversal!
                    info!("[SPECULATIVE] BLACK HOLE TRAVERSAL SUCCESS! Destination: {}", destination_seed);
                    
                    // Send discovery event
                    discovery_events.write(SpeculativeDiscoveryEvent {
                        mechanic: HiddenMechanicId::EventHorizonSurvival,
                        discoverer: None, // Would be player ID
                        method: DiscoveryMethod::Experiment,
                        timestamp: 0.0, // Would be simulation time
                        first_discovery: !speculative_state.global_discoveries.contains(&HiddenMechanicId::EventHorizonSurvival),
                    });
                    
                    // Record discovery
                    speculative_state.global_discoveries.insert(HiddenMechanicId::EventHorizonSurvival);
                    
                    // Establish portal
                    multiverse.known_universes.push(UniverseInfo {
                        seed: destination_seed,
                        discovered_by: 0, // Player ID
                        discovered_at: 0.0, // Sim time
                        physical_constants_variation: (destination_seed as f64 % 1000.0) / 5000.0, // 0-20% variation
                        notable_features: vec!["Newly Discovered".to_string()],
                    });
                    
                    let source_universe = multiverse.current_universe_seed;
                    multiverse.active_portals.push(MultiversePortal {
                        source_universe,
                        destination_universe: destination_seed,
                        source_black_hole: bh_entity,
                        established_at: 0.0,
                        traversals: 1,
                    });
                }
                TraversalResult::Failed(failure) => {
                    // Log failure (for player learning)
                    warn!("[SPECULATIVE] Traversal failed: {:?}", failure);
                    warn!("Hint: {}", failure.hint_message());
                }
            }
        }
    }
}

/// System to process speculative discoveries
fn process_speculative_discoveries(
    mut events: MessageReader<SpeculativeDiscoveryEvent>,
    mut state: ResMut<SpeculativePhysicsState>,
) {
    for event in events.read() {
        if event.first_discovery {
            info!(
                "[SPECULATIVE] FIRST DISCOVERY: {:?} by {:?} via {:?}",
                event.mechanic, event.discoverer, event.method
            );
            
            // This would trigger server-wide announcements, achievements, etc.
        }
        
        // Record in global discoveries
        state.global_discoveries.insert(event.mechanic);
        
        // Award player (if any)
        if let Some(player_id) = event.discoverer {
            let player_state = state.player_discoveries
                .entry(player_id)
                .or_default();
            player_state.discovered.insert(event.mechanic);
        }
    }
}

/// Warp drive mechanics (Tier 8+)
pub mod warp_drive {
    /// Calculate energy required for Alcubierre-style warp bubble
    /// Based on exotic matter requirements
    pub fn warp_energy_requirement(
        bubble_radius: f64,  // meters
        warp_factor: f64,    // multiples of c
        _travel_distance: f64, // meters
    ) -> f64 {
        // Simplified - real calculation involves spacetime metric
        // Energy scales with bubble surface area and warp factor cubed
        let surface_area = 4.0 * std::f64::consts::PI * bubble_radius * bubble_radius;
        let c: f64 = 299_792_458.0;
        
        // Exotic matter energy (negative energy density required)
        // This is astronomical for realistic warp - gameplay can tune this
        surface_area * c * c * c * c * warp_factor * warp_factor * warp_factor * 1e-20 // Tuned for gameplay
    }
    
    /// Check if warp drive is possible with available exotic matter
    pub fn can_engage_warp(
        available_exotic_matter: f64, // kg equivalent
        bubble_radius: f64,
        warp_factor: f64,
    ) -> bool {
        let c = 299_792_458.0;
        let energy_available = available_exotic_matter * c * c;
        let energy_required = warp_energy_requirement(bubble_radius, warp_factor, 1.0);
        energy_available >= energy_required
    }
}

/// Time manipulation mechanics (Tier 10+)
pub mod time_manipulation {
    /// Calculate closed timelike curve stability
    /// CTC near a rotating black hole
    pub fn ctc_stability(
        black_hole_mass: f64,
        black_hole_spin: f64, // 0-1
        orbit_radius: f64,    // meters
    ) -> f64 {
        // CTCs are only possible inside the inner horizon of a Kerr black hole
        // This is deeply speculative physics
        
        // Simplified stability metric (0 = unstable, 1 = stable)
        let g = 6.674e-11;
        let c = 299_792_458.0;
        let rs = 2.0 * g * black_hole_mass / (c * c);
        
        // Inner horizon radius for Kerr black hole
        let r_inner = rs * (1.0 - (1.0 - black_hole_spin * black_hole_spin).sqrt()) / 2.0;
        
        if orbit_radius > r_inner {
            0.0 // CTCs don't exist outside inner horizon
        } else {
            // Stability increases deeper inside (but access is harder)
            let depth_factor = 1.0 - orbit_radius / r_inner;
            let spin_factor = black_hole_spin.powf(0.5); // Higher spin = more stable
            depth_factor * spin_factor
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_black_hole_event_horizon() {
        let bh = BlackHole {
            mass: 1.989e30, // Solar mass
            spin_parameter: 0.0,
            spin_axis: Vec3::Y,
            traversed: false,
            destination_seed: Some(12345),
            failed_attempts: 0,
            first_traverser: None,
        };
        
        let g = 6.674e-11;
        let c = 299_792_458.0;
        let rs = bh.event_horizon_radius(g, c);
        
        // Schwarzschild radius of Sun should be ~2953 m
        assert!((rs - 2953.0).abs() < 10.0);
    }
    
    #[test]
    fn test_traversal_conditions() {
        let conditions = BlackHoleTraversalConditions::default();
        
        // Velocity range should be 0.9c - 0.95c
        assert!((conditions.velocity_range.0 - 0.9).abs() < 0.01);
        assert!((conditions.velocity_range.1 - 0.95).abs() < 0.01);
        
        // Angle tolerance should be 5 degrees
        assert!((conditions.approach_angle_tolerance - 5.0_f64.to_radians()).abs() < 0.01);
    }
}

