// Discovery and Tech Tree System
// Infinite Universe Engine - Knowledge discovery and progression
//
// Players progress by discovering physics, not by gathering resources.
// The tech tree is infinite, extending into speculative physics.
//
// Key design principles:
// - Discovery through experimentation, not recipes
// - Knowledge persists and can be shared
// - Each discovery unlocks new possibilities
// - No hard ceiling - always more to discover

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct DiscoveryPlugin;

impl Plugin for DiscoveryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobalKnowledgeBase>()
           .init_resource::<TechTree>()
           .add_message::<DiscoveryEvent>()
           .add_message::<TechUnlockEvent>()
           .add_systems(Update, (
               process_discovery_events,
               check_tech_unlocks,
           ).chain().run_if(in_state(crate::GameState::Playing)));
    }
}

/// Unique identifier for a phenomenon that can be discovered
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhenomenonId(pub u32);

/// Unique identifier for a technology that can be unlocked
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TechId(pub u32);

/// Categories of phenomena
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PhenomenonCategory {
    // Tier 0: Basic Survival
    Fire,
    Gravity,
    Buoyancy,
    Leverage,
    
    // Tier 1: Agriculture & Metallurgy
    PlantGrowth,
    Fermentation,
    Smelting,
    Alloying,
    
    // Tier 2: Industrial
    SteamPower,
    MechanicalAdvantage,
    ChemicalReactions,
    Pressure,
    
    // Tier 3: Electrical
    Electricity,
    Magnetism,
    Electromagnetism,
    Semiconductors,
    
    // Tier 4: Nuclear & Space
    Radioactivity,
    NuclearFission,
    NuclearFusion,
    OrbitalMechanics,
    
    // Tier 5: Relativistic
    TimeDilation,
    LengthContraction,
    MassEnergy,
    
    // Tier 6: Quantum
    Superposition,
    Entanglement,
    Tunneling,
    
    // Tier 7+: Speculative
    ExoticMatter,
    WarpField,
    BlackHolePhysics,
    Multiverse,
}

/// A phenomenon that can be discovered
#[derive(Clone, Debug)]
pub struct Phenomenon {
    pub id: PhenomenonId,
    pub name: String,
    pub description: String,
    pub category: PhenomenonCategory,
    pub tier: u32,
    /// Prerequisites (other phenomena that must be discovered first)
    pub prerequisites: Vec<PhenomenonId>,
    /// How the phenomenon can be discovered
    pub discovery_method: DiscoveryMethod,
    /// How difficult to discover (multiplier on conditions)
    pub discovery_difficulty: f64,
}

/// How a phenomenon can be discovered
#[derive(Clone, Debug)]
pub enum DiscoveryMethod {
    /// Observe something happen naturally
    Observation { condition: String },
    /// Perform a specific experiment
    Experiment { procedure: String },
    /// Use instruments to measure
    Measurement { instrument: String, threshold: f64 },
    /// Combine discovered phenomena
    Synthesis { required: Vec<PhenomenonId> },
    /// Taught by another player or NPC
    Teaching,
}

/// A discovered piece of knowledge
#[derive(Clone, Debug)]
pub struct Discovery {
    pub phenomenon: PhenomenonId,
    pub discoverer: u64, // Player ID (0 for NPC/system)
    pub timestamp: f64,  // Simulation time
    pub method: DiscoveryMethod,
    pub notes: String,   // Player-written notes
    pub reproducibility: f64, // How well-documented (0-1)
}

/// Global knowledge base shared by all players
#[derive(Resource, Default)]
pub struct GlobalKnowledgeBase {
    /// All phenomena that have been discovered by anyone
    pub discoveries: HashMap<PhenomenonId, Discovery>,
}

impl GlobalKnowledgeBase {
    /// Check if a phenomenon has been discovered globally
    pub fn is_discovered(&self, id: PhenomenonId) -> bool {
        self.discoveries.contains_key(&id)
    }
    
    /// Get the discovery record for a phenomenon
    pub fn get_discovery(&self, id: PhenomenonId) -> Option<&Discovery> {
        self.discoveries.get(&id)
    }
    
    /// Record a new discovery
    pub fn record_discovery(&mut self, discovery: Discovery) {
        self.discoveries.insert(discovery.phenomenon, discovery);
    }
}

/// Component for player's personal knowledge
#[derive(Component, Clone, Debug, Default)]
pub struct PlayerKnowledge {
    /// Phenomena this player has personally discovered
    pub discovered: HashSet<PhenomenonId>,
    /// Phenomena this player knows about (from teaching/reading)
    pub known: HashSet<PhenomenonId>,
    /// Technologies this player has unlocked
    pub unlocked_tech: HashSet<TechId>,
    /// Current research focus
    pub research_focus: Option<PhenomenonId>,
    /// Accumulated research points
    pub research_progress: HashMap<PhenomenonId, f64>,
    /// Personal notes
    pub notes: HashMap<PhenomenonId, String>,
}

impl PlayerKnowledge {
    /// Check if player can discover a phenomenon
    pub fn can_discover(&self, phenomenon: &Phenomenon) -> bool {
        // Check prerequisites
        for prereq in &phenomenon.prerequisites {
            if !self.discovered.contains(prereq) && !self.known.contains(prereq) {
                return false;
            }
        }
        true
    }
    
    /// Add research progress toward a phenomenon
    pub fn add_research_progress(&mut self, id: PhenomenonId, amount: f64) {
        let current = self.research_progress.entry(id).or_insert(0.0);
        *current += amount;
    }
    
    /// Check if research is complete
    pub fn is_research_complete(&self, id: PhenomenonId, required: f64) -> bool {
        self.research_progress.get(&id).map(|p| *p >= required).unwrap_or(false)
    }
}

/// The infinite tech tree
#[derive(Resource)]
pub struct TechTree {
    /// All technologies
    pub technologies: HashMap<TechId, Technology>,
    /// Phenomena registry
    pub phenomena: HashMap<PhenomenonId, Phenomenon>,
    /// Next available IDs
    next_tech_id: u32,
    next_phenomenon_id: u32,
}

impl Default for TechTree {
    fn default() -> Self {
        let mut tree = Self {
            technologies: HashMap::new(),
            phenomena: HashMap::new(),
            next_tech_id: 0,
            next_phenomenon_id: 0,
        };
        
        // Initialize with base phenomena and technologies
        tree.initialize_base_tree();
        tree
    }
}

impl TechTree {
    fn initialize_base_tree(&mut self) {
        // Tier 0: Basic Survival
        let fire = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Fire".to_string(),
            description: "Combustion releases heat and light".to_string(),
            category: PhenomenonCategory::Fire,
            tier: 0,
            prerequisites: vec![],
            discovery_method: DiscoveryMethod::Observation { 
                condition: "Strike flint and steel, or rub sticks together".to_string() 
            },
            discovery_difficulty: 1.0,
        });
        
        let gravity = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Gravity".to_string(),
            description: "Objects fall toward massive bodies".to_string(),
            category: PhenomenonCategory::Gravity,
            tier: 0,
            prerequisites: vec![],
            discovery_method: DiscoveryMethod::Observation { 
                condition: "Drop an object and observe it fall".to_string() 
            },
            discovery_difficulty: 0.5,
        });
        
        let leverage = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Leverage".to_string(),
            description: "Mechanical advantage through levers".to_string(),
            category: PhenomenonCategory::Leverage,
            tier: 0,
            prerequisites: vec![],
            discovery_method: DiscoveryMethod::Experiment { 
                procedure: "Use a stick as a lever to move a heavy object".to_string() 
            },
            discovery_difficulty: 1.0,
        });
        
        // Add basic technologies
        self.add_technology(Technology {
            id: TechId(0),
            name: "Fire Starting".to_string(),
            description: "Create and control fire".to_string(),
            tier: 0,
            required_knowledge: vec![fire],
            unlocks: vec![
                TechUnlock::Ability("Cook Food".to_string()),
                TechUnlock::Ability("Stay Warm".to_string()),
                TechUnlock::Recipe("Torch".to_string()),
            ],
        });
        
        self.add_technology(Technology {
            id: TechId(0),
            name: "Simple Tools".to_string(),
            description: "Basic hand tools for survival".to_string(),
            tier: 0,
            required_knowledge: vec![leverage],
            unlocks: vec![
                TechUnlock::Recipe("Stone Knife".to_string()),
                TechUnlock::Recipe("Wooden Club".to_string()),
                TechUnlock::Recipe("Simple Lever".to_string()),
            ],
        });
        
        // Tier 1: Agriculture & Metallurgy
        let plant_growth = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Plant Growth".to_string(),
            description: "Plants grow from seeds with water and sunlight".to_string(),
            category: PhenomenonCategory::PlantGrowth,
            tier: 1,
            prerequisites: vec![],
            discovery_method: DiscoveryMethod::Observation {
                condition: "Plant a seed and watch it grow over time".to_string()
            },
            discovery_difficulty: 2.0,
        });
        
        let smelting = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Smelting".to_string(),
            description: "Metal can be extracted from ore with heat".to_string(),
            category: PhenomenonCategory::Smelting,
            tier: 1,
            prerequisites: vec![fire],
            discovery_method: DiscoveryMethod::Experiment {
                procedure: "Heat ore to extreme temperatures in a furnace".to_string()
            },
            discovery_difficulty: 3.0,
        });
        
        // Tier 3: Electrical
        let electricity = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Electricity".to_string(),
            description: "Flow of electric charge through conductors".to_string(),
            category: PhenomenonCategory::Electricity,
            tier: 3,
            prerequisites: vec![],
            discovery_method: DiscoveryMethod::Experiment {
                procedure: "Create a potential difference between two materials".to_string()
            },
            discovery_difficulty: 5.0,
        });
        
        let magnetism = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Magnetism".to_string(),
            description: "Magnetic fields attract and repel".to_string(),
            category: PhenomenonCategory::Magnetism,
            tier: 3,
            prerequisites: vec![],
            discovery_method: DiscoveryMethod::Observation {
                condition: "Observe lodestone attracting iron".to_string()
            },
            discovery_difficulty: 3.0,
        });
        
        let electromagnetism = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Electromagnetism".to_string(),
            description: "Electric current creates magnetic fields".to_string(),
            category: PhenomenonCategory::Electromagnetism,
            tier: 3,
            prerequisites: vec![electricity, magnetism],
            discovery_method: DiscoveryMethod::Experiment {
                procedure: "Pass current through a wire near a compass".to_string()
            },
            discovery_difficulty: 6.0,
        });
        
        let semiconductors = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Semiconductors".to_string(),
            description: "Materials that conduct electricity under certain conditions".to_string(),
            category: PhenomenonCategory::Semiconductors,
            tier: 3,
            prerequisites: vec![electricity],
            discovery_method: DiscoveryMethod::Measurement {
                instrument: "Resistance Meter".to_string(),
                threshold: 1e-6,
            },
            discovery_difficulty: 8.0,
        });
        
        // Tier 5: Relativistic
        let time_dilation = self.add_phenomenon(Phenomenon {
            id: PhenomenonId(0),
            name: "Time Dilation".to_string(),
            description: "Time passes slower at high velocities".to_string(),
            category: PhenomenonCategory::TimeDilation,
            tier: 5,
            prerequisites: vec![electricity, gravity],
            discovery_method: DiscoveryMethod::Measurement {
                instrument: "Precise Atomic Clock".to_string(),
                threshold: 1e-15,
            },
            discovery_difficulty: 20.0,
        });
        
        // Technologies for electrical tier
        self.add_technology(Technology {
            id: TechId(0),
            name: "Basic Circuits".to_string(),
            description: "Create simple electrical circuits".to_string(),
            tier: 3,
            required_knowledge: vec![electricity],
            unlocks: vec![
                TechUnlock::Recipe("Wire".to_string()),
                TechUnlock::Recipe("Switch".to_string()),
                TechUnlock::Recipe("Resistor".to_string()),
                TechUnlock::Blueprint("Simple Circuit".to_string()),
            ],
        });
        
        self.add_technology(Technology {
            id: TechId(0),
            name: "Electromagnetic Devices".to_string(),
            description: "Devices using electromagnetism".to_string(),
            tier: 3,
            required_knowledge: vec![electromagnetism],
            unlocks: vec![
                TechUnlock::Recipe("Electromagnet".to_string()),
                TechUnlock::Recipe("Electric Motor".to_string()),
                TechUnlock::Recipe("Generator".to_string()),
            ],
        });
        
        self.add_technology(Technology {
            id: TechId(0),
            name: "Transistors".to_string(),
            description: "Semiconductor switches for computing".to_string(),
            tier: 3,
            required_knowledge: vec![semiconductors],
            unlocks: vec![
                TechUnlock::Recipe("Transistor".to_string()),
                TechUnlock::Blueprint("Logic Gate".to_string()),
                TechUnlock::Blueprint("Amplifier".to_string()),
            ],
        });
    }
    
    fn add_phenomenon(&mut self, mut phenomenon: Phenomenon) -> PhenomenonId {
        let id = PhenomenonId(self.next_phenomenon_id);
        self.next_phenomenon_id += 1;
        phenomenon.id = id;
        self.phenomena.insert(id, phenomenon);
        id
    }
    
    fn add_technology(&mut self, mut tech: Technology) -> TechId {
        let id = TechId(self.next_tech_id);
        self.next_tech_id += 1;
        tech.id = id;
        self.technologies.insert(id, tech);
        id
    }
    
    /// Get all technologies available given current knowledge
    pub fn available_technologies(&self, knowledge: &PlayerKnowledge) -> Vec<&Technology> {
        self.technologies.values()
            .filter(|tech| {
                !knowledge.unlocked_tech.contains(&tech.id) &&
                tech.required_knowledge.iter().all(|req| {
                    knowledge.discovered.contains(req) || knowledge.known.contains(req)
                })
            })
            .collect()
    }
    
    /// Get all phenomena that can currently be researched
    pub fn researchable_phenomena(&self, knowledge: &PlayerKnowledge) -> Vec<&Phenomenon> {
        self.phenomena.values()
            .filter(|p| {
                !knowledge.discovered.contains(&p.id) &&
                !knowledge.known.contains(&p.id) &&
                p.prerequisites.iter().all(|req| {
                    knowledge.discovered.contains(req) || knowledge.known.contains(req)
                })
            })
            .collect()
    }
}

/// A technology that can be unlocked
#[derive(Clone, Debug)]
pub struct Technology {
    pub id: TechId,
    pub name: String,
    pub description: String,
    pub tier: u32,
    /// Phenomena that must be known to unlock this tech
    pub required_knowledge: Vec<PhenomenonId>,
    /// What this technology unlocks
    pub unlocks: Vec<TechUnlock>,
}

/// What a technology unlocks
#[derive(Clone, Debug)]
pub enum TechUnlock {
    /// A new ability (e.g., "Cook Food")
    Ability(String),
    /// A crafting recipe
    Recipe(String),
    /// A buildable blueprint
    Blueprint(String),
    /// Access to a new phenomenon to research
    Phenomenon(PhenomenonId),
}

/// Event when something is discovered
#[derive(Message)]
pub struct DiscoveryEvent {
    pub player: u64,
    pub phenomenon: PhenomenonId,
    pub method: DiscoveryMethod,
    pub is_first: bool, // First person to discover this
}

/// Event when a technology is unlocked
#[derive(Message)]
pub struct TechUnlockEvent {
    pub player: u64,
    pub technology: TechId,
}

/// System to process discoveries
fn process_discovery_events(
    mut events: MessageReader<DiscoveryEvent>,
    mut knowledge_base: ResMut<GlobalKnowledgeBase>,
    mut player_query: Query<(&crate::plugins::observation::PlayerId, &mut PlayerKnowledge)>,
    tech_tree: Res<TechTree>,
) {
    for event in events.read() {
        // Record in global knowledge base
        if event.is_first {
            if let Some(phenomenon) = tech_tree.phenomena.get(&event.phenomenon) {
                info!("[DISCOVERY] First discovery of '{}' by player {}",
                      phenomenon.name, event.player);
            }

            knowledge_base.record_discovery(Discovery {
                phenomenon: event.phenomenon,
                discoverer: event.player,
                timestamp: 0.0, // Would be simulation time
                method: event.method.clone(),
                notes: String::new(),
                reproducibility: 1.0,
            });
        }

        // Update player knowledge — find player by ID
        for (player_id, mut knowledge) in player_query.iter_mut() {
            if player_id.0 == event.player {
                knowledge.discovered.insert(event.phenomenon);
                if let Some(phenomenon) = tech_tree.phenomena.get(&event.phenomenon) {
                    info!("[KNOWLEDGE] Player {} discovered '{}'", event.player, phenomenon.name);
                }
                break;
            }
        }
    }
}

/// System to check and unlock technologies
fn check_tech_unlocks(
    mut tech_events: MessageWriter<TechUnlockEvent>,
    tech_tree: Res<TechTree>,
    mut player_query: Query<(Entity, &mut PlayerKnowledge)>,
) {
    for (entity, mut knowledge) in player_query.iter_mut() {
        // Check each technology
        for tech in tech_tree.available_technologies(&knowledge) {
            // Auto-unlock technologies when requirements are met
            let can_unlock = tech.required_knowledge.iter().all(|req| {
                knowledge.discovered.contains(req) || knowledge.known.contains(req)
            });
            
            if can_unlock && !knowledge.unlocked_tech.contains(&tech.id) {
                knowledge.unlocked_tech.insert(tech.id);
                
                tech_events.write(TechUnlockEvent {
                    player: 0, // Would be player ID
                    technology: tech.id,
                });
                
                info!("[TECH] Unlocked '{}'", tech.name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tech_tree_initialization() {
        let tree = TechTree::default();
        
        // Should have some base phenomena
        assert!(!tree.phenomena.is_empty());
        
        // Should have some base technologies
        assert!(!tree.technologies.is_empty());
    }
    
    #[test]
    fn test_player_knowledge() {
        let mut knowledge = PlayerKnowledge::default();
        let fire_id = PhenomenonId(0);
        
        // Initially nothing discovered
        assert!(knowledge.discovered.is_empty());
        
        // Discover fire
        knowledge.discovered.insert(fire_id);
        assert!(knowledge.discovered.contains(&fire_id));
    }
}

