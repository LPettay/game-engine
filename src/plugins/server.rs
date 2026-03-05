// Persistent World Server Architecture
// Infinite Universe Engine - Server that runs for years
//
// Design for 10-year progression to multiverse:
// - 100 players averaging 2 hours/week
// - 1,040,000 collective player-hours to first multiverse
// - World continues when players are offline
// - No FOMO mechanics - no decay while away

use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Duration;

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerState>()
           .init_resource::<CollectiveKnowledge>()
           .init_resource::<ServerHistory>()
           .add_message::<MilestoneEvent>()
           .add_systems(Update, (
               track_milestones,
               process_offline_progress,
           ).chain());
    }
}

// ============================================================================
// SERVER STATE
// ============================================================================

/// Server configuration and state
#[derive(Resource)]
pub struct ServerState {
    /// Server type/configuration
    pub server_type: ServerType,
    /// World creation time (real world)
    pub created_at: Duration,
    /// Total server uptime
    pub uptime: Duration,
    /// Connected players
    pub online_players: Vec<u64>,
    /// All players (online and offline)
    pub all_players: HashMap<u64, PlayerRecord>,
    /// Server simulation state
    pub simulation: SimulationHandle,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            server_type: ServerType::Standard {
                time_compression: 24.0,
                expected_years: 10,
            },
            created_at: Duration::ZERO,
            uptime: Duration::ZERO,
            online_players: Vec::new(),
            all_players: HashMap::new(),
            simulation: SimulationHandle::default(),
        }
    }
}

/// Server type determines progression speed
#[derive(Clone, Debug)]
pub enum ServerType {
    Standard {
        time_compression: f64,       // 24.0 = 1hr = 1 day
        expected_years: u32,         // 10 years to multiverse
    },
    Accelerated {
        time_compression: f64,       // 168.0 = 1hr = 1 week
        expected_years: u32,         // 2 years to multiverse
    },
    Hardcore {
        time_compression: f64,
        permadeath: bool,
        expected_years: u32,         // 20 years
    },
    Creative {
        unlimited_resources: bool,
        discovery_unlocked: bool,
    },
}

impl Default for ServerType {
    fn default() -> Self {
        Self::Standard {
            time_compression: 24.0,
            expected_years: 10,
        }
    }
}

/// Handle for the simulation thread
#[derive(Clone, Debug, Default)]
pub struct SimulationHandle {
    /// Simulation tick rate (ticks per real second)
    pub tick_rate: f64,
    /// Current time compression
    pub time_compression: f64,
    /// Whether to run faster when no players online
    pub catchup_mode: bool,
    /// Current simulation tick
    pub current_tick: u64,
}

/// Record for a player
#[derive(Clone, Debug)]
pub struct PlayerRecord {
    pub id: u64,
    pub name: String,
    /// Total time played
    pub playtime: Duration,
    /// Last login time
    pub last_login: Duration,
    /// Whether currently online
    pub online: bool,
    /// Player's progression state
    pub progression: PlayerProgression,
    /// Player's contributions
    pub contributions: Vec<ContributionRecord>,
}

/// Player's progression state
#[derive(Clone, Debug, Default)]
pub struct PlayerProgression {
    /// Discovered phenomena
    pub discoveries: Vec<u32>,
    /// Unlocked technologies
    pub technologies: Vec<u32>,
    /// Specializations and skill levels
    pub specializations: HashMap<Specialization, f64>,
    /// Players mentored
    pub mentored_players: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Specialization {
    // Science
    PhysicsResearch,
    ChemistryResearch,
    BiologyResearch,
    Astronomy,
    
    // Engineering
    MechanicalEngineering,
    ElectricalEngineering,
    ChemicalEngineering,
    AerospaceEngineering,
    
    // Support
    ResourceGathering,
    Manufacturing,
    Teaching,
    Exploration,
}

/// Record of a contribution to collective progress
#[derive(Clone, Debug)]
pub struct ContributionRecord {
    pub contribution_type: ContributionType,
    pub timestamp: Duration,
    pub description: String,
    pub value: f64,
}

#[derive(Clone, Debug)]
pub enum ContributionType {
    Discovery,
    Research,
    Blueprint,
    Teaching,
    Infrastructure,
    Expedition,
}

// ============================================================================
// COLLECTIVE KNOWLEDGE
// ============================================================================

/// Knowledge shared by all players
#[derive(Resource, Default)]
pub struct CollectiveKnowledge {
    /// All discoveries made
    pub discoveries: Vec<DiscoveryRecord>,
    /// Published research
    pub research_papers: Vec<ResearchPaper>,
    /// Available blueprints
    pub blueprints: Vec<BlueprintRecord>,
    /// Teaching materials
    pub teaching_materials: Vec<TeachingMaterial>,
    /// Experimental data (including failed experiments)
    pub experimental_data: Vec<ExperimentRecord>,
}

/// A recorded discovery
#[derive(Clone, Debug)]
pub struct DiscoveryRecord {
    pub phenomenon_id: u32,
    pub discoverer: u64,
    pub timestamp: Duration,
    pub documentation: String,
    pub reproducibility: f64,
    pub accessibility: AccessLevel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessLevel {
    Public,
    Guild { guild_id: u64 },
    Private,
}

/// A published research paper
#[derive(Clone, Debug)]
pub struct ResearchPaper {
    pub id: u64,
    pub title: String,
    pub author: u64,
    pub timestamp: Duration,
    pub content: String,
    pub phenomena: Vec<u32>,
    pub citations: u32,
}

/// A blueprint that can be shared
#[derive(Clone, Debug)]
pub struct BlueprintRecord {
    pub id: u64,
    pub name: String,
    pub creator: u64,
    pub technology_design: u32,
    pub timestamp: Duration,
    pub usage_count: u32,
}

/// Teaching material
#[derive(Clone, Debug)]
pub struct TeachingMaterial {
    pub id: u64,
    pub title: String,
    pub teacher: u64,
    pub phenomenon: u32,
    pub content: String,
    pub students_taught: u32,
}

/// Record of an experiment
#[derive(Clone, Debug)]
pub struct ExperimentRecord {
    pub id: u64,
    pub experimenter: u64,
    pub hypothesis: String,
    pub procedure: String,
    pub result: String,
    pub success: bool,
    pub timestamp: Duration,
}

// ============================================================================
// SERVER HISTORY
// ============================================================================

/// History of the server (emergent lore)
#[derive(Resource, Default)]
pub struct ServerHistory {
    /// Server founding date
    pub founding_date: Duration,
    /// Major milestones achieved
    pub milestones: Vec<HistoricalMilestone>,
    /// Notable players
    pub notable_players: Vec<PlayerLegacy>,
    /// Failed expeditions (for learning)
    pub failed_expeditions: Vec<ExpeditionRecord>,
    /// First achievements
    pub first_achievements: Vec<FirstAchievement>,
}

/// A major milestone in server history
#[derive(Clone, Debug)]
pub struct HistoricalMilestone {
    pub event: MilestoneType,
    pub timestamp: Duration,
    pub participants: Vec<u64>,
    pub documentation: String,
    pub monument_location: Option<Vec3>,
}

#[derive(Clone, Debug)]
pub enum MilestoneType {
    FirstFire,
    FirstSmelting,
    FirstElectricity,
    FirstComputer,
    FirstNuclear,
    FirstSpaceflight,
    FirstMoonLanding,
    FirstInterstellar,
    FirstBlackHoleApproach,
    FirstMultiverseTraversal,
}

/// A player's lasting legacy
#[derive(Clone, Debug)]
pub struct PlayerLegacy {
    pub player_id: u64,
    pub name: String,
    pub notable_achievements: Vec<String>,
    pub total_playtime: Duration,
    pub active_period: (Duration, Duration),
}

/// Record of a failed expedition
#[derive(Clone, Debug)]
pub struct ExpeditionRecord {
    pub id: u64,
    pub participants: Vec<u64>,
    pub objective: String,
    pub outcome: String,
    pub lessons_learned: String,
    pub timestamp: Duration,
}

/// First-time achievement
#[derive(Clone, Debug)]
pub struct FirstAchievement {
    pub achievement: String,
    pub achiever: u64,
    pub timestamp: Duration,
}

/// Milestone event
#[derive(Message)]
pub struct MilestoneEvent {
    pub milestone: MilestoneType,
    pub participants: Vec<u64>,
    pub timestamp: Duration,
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Track and record milestones
fn track_milestones(
    mut events: MessageReader<MilestoneEvent>,
    mut history: ResMut<ServerHistory>,
    server: Res<ServerState>,
) {
    for event in events.read() {
        info!(
            "[Server] MILESTONE: {:?} achieved by {} players!",
            event.milestone,
            event.participants.len()
        );
        
        history.milestones.push(HistoricalMilestone {
            event: event.milestone.clone(),
            timestamp: event.timestamp,
            participants: event.participants.clone(),
            documentation: String::new(),
            monument_location: None,
        });
    }
}

/// Process offline progress
fn process_offline_progress(
    mut server: ResMut<ServerState>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    server.uptime += Duration::from_secs_f64(dt);
    
    // When no players are online, world still progresses
    if server.online_players.is_empty() {
        // Crops grow, experiments complete, etc.
        // But no decay - anti-FOMO design
    }
}

// ============================================================================
// ANTI-DECAY DESIGN
// ============================================================================

/// Things that DON'T happen when players are offline (anti-FOMO):
/// - Buildings don't decay
/// - Crops don't die (just stop growing at maturity)
/// - Equipment doesn't break
/// - Knowledge doesn't fade
/// - Territory isn't lost
///
/// Things that DO happen:
/// - Seasons change
/// - Weather patterns evolve
/// - Ecosystems slowly change
/// - Other players' progress continues

/// Calculate what happens during offline period
pub fn calculate_offline_progress(
    player: &PlayerRecord,
    offline_duration: Duration,
    server_type: &ServerType,
) -> OfflineProgress {
    let time_compression = match server_type {
        ServerType::Standard { time_compression, .. } => *time_compression,
        ServerType::Accelerated { time_compression, .. } => *time_compression,
        ServerType::Hardcore { time_compression, .. } => *time_compression,
        ServerType::Creative { .. } => 1.0,
    };
    
    let sim_days = offline_duration.as_secs_f64() / 3600.0 * time_compression / 24.0;
    
    OfflineProgress {
        simulation_days_passed: sim_days,
        crops_grown: true, // Crops mature but don't die
        experiments_completed: true, // Ongoing experiments finish
        buildings_intact: true, // No decay
        resources_intact: true, // Nothing stolen/lost
    }
}

#[derive(Clone, Debug)]
pub struct OfflineProgress {
    pub simulation_days_passed: f64,
    pub crops_grown: bool,
    pub experiments_completed: bool,
    pub buildings_intact: bool,
    pub resources_intact: bool,
}

// ============================================================================
// SESSION DESIGN
// ============================================================================

/// Designed for 2-hour play sessions that feel complete
#[derive(Clone, Debug)]
pub struct SessionType {
    pub name: String,
    pub typical_duration: Duration,
    pub expected_progress: String,
    pub satisfaction_level: f64,
}

impl SessionType {
    pub fn gathering() -> Self {
        Self {
            name: "Gathering".to_string(),
            typical_duration: Duration::from_secs(3600), // 1 hour
            expected_progress: "Fill inventory with resources".to_string(),
            satisfaction_level: 0.7,
        }
    }
    
    pub fn experimentation() -> Self {
        Self {
            name: "Experimentation".to_string(),
            typical_duration: Duration::from_secs(7200), // 2 hours
            expected_progress: "Test hypothesis, collect data".to_string(),
            satisfaction_level: 0.8,
        }
    }
    
    pub fn building() -> Self {
        Self {
            name: "Building".to_string(),
            typical_duration: Duration::from_secs(5400), // 1.5 hours
            expected_progress: "Complete a structure or device".to_string(),
            satisfaction_level: 0.9,
        }
    }
    
    pub fn teaching() -> Self {
        Self {
            name: "Teaching".to_string(),
            typical_duration: Duration::from_secs(1800), // 30 min
            expected_progress: "Help a new player learn".to_string(),
            satisfaction_level: 0.85,
        }
    }
    
    pub fn exploration() -> Self {
        Self {
            name: "Exploration".to_string(),
            typical_duration: Duration::from_secs(5400), // 1.5 hours
            expected_progress: "Map new area, find resources".to_string(),
            satisfaction_level: 0.8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_offline_progress() {
        let player = PlayerRecord {
            id: 1,
            name: "Test".to_string(),
            playtime: Duration::ZERO,
            last_login: Duration::ZERO,
            online: false,
            progression: PlayerProgression::default(),
            contributions: vec![],
        };
        
        let server_type = ServerType::Standard {
            time_compression: 24.0,
            expected_years: 10,
        };
        
        // 1 hour offline = 1 sim day
        let progress = calculate_offline_progress(
            &player,
            Duration::from_secs(3600),
            &server_type,
        );
        
        assert!((progress.simulation_days_passed - 1.0).abs() < 0.01);
        assert!(progress.buildings_intact);
        assert!(progress.resources_intact);
    }
}

