// Pre-built behavior trees for different species types

use crate::plugins::discovery::PhenomenonId;
use crate::plugins::ecosystem::SpeciesType;
use super::behavior::*;

/// Behavior tree for herbivores: flee > forage > rest > socialize > wander
pub fn herbivore_behavior() -> BehaviorNode {
    BehaviorNode::Selector(vec![
        // Priority 1: Flee if threatened
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Threatened(0.3)),
            BehaviorNode::Action(CreatureAction::FleeFrom),
        ]),
        // Priority 2: Forage if hungry
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Hungry(0.4)),
            BehaviorNode::Action(CreatureAction::Forage),
        ]),
        // Priority 3: Rest if tired
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Tired(0.3)),
            BehaviorNode::Action(CreatureAction::Rest),
        ]),
        // Priority 4: Socialize if near flock
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::NearFlock(30.0)),
            BehaviorNode::Action(CreatureAction::Socialize),
        ]),
        // Default: Wander
        BehaviorNode::Action(CreatureAction::Wander),
    ])
}

/// Behavior tree for carnivores: hunt > forage > rest > mark territory > wander
pub fn carnivore_behavior() -> BehaviorNode {
    BehaviorNode::Selector(vec![
        // Priority 1: Hunt if near prey and hungry
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Hungry(0.3)),
            BehaviorNode::Condition(CreatureCondition::NearPrey(50.0)),
            BehaviorNode::Action(CreatureAction::HuntPrey),
        ]),
        // Priority 2: Forage if hungry (fallback)
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Hungry(0.5)),
            BehaviorNode::Action(CreatureAction::Forage),
        ]),
        // Priority 3: Rest if tired
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Tired(0.3)),
            BehaviorNode::Action(CreatureAction::Rest),
        ]),
        // Priority 4: Mark territory
        BehaviorNode::Action(CreatureAction::MarkTerritory),
        // Default: Wander
        BehaviorNode::Action(CreatureAction::Wander),
    ])
}

/// Behavior tree for decomposers: forage > rest > wander (simple)
pub fn decomposer_behavior() -> BehaviorNode {
    BehaviorNode::Selector(vec![
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Hungry(0.3)),
            BehaviorNode::Action(CreatureAction::Forage),
        ]),
        BehaviorNode::Sequence(vec![
            BehaviorNode::Condition(CreatureCondition::Tired(0.4)),
            BehaviorNode::Action(CreatureAction::Rest),
        ]),
        BehaviorNode::Action(CreatureAction::Wander),
    ])
}

/// Get the behavior tree for a species type
pub fn behavior_for_species(species_type: &SpeciesType) -> BehaviorNode {
    match species_type {
        SpeciesType::Herbivore => herbivore_behavior(),
        SpeciesType::Carnivore => carnivore_behavior(),
        SpeciesType::Omnivore => herbivore_behavior(), // Uses herbivore tree with hunt capability
        SpeciesType::Decomposer => decomposer_behavior(),
        SpeciesType::Producer => decomposer_behavior(), // Plants don't move much
    }
}

/// Map species type to the phenomenon that observing it teaches
pub fn observable_phenomenon(species_type: &SpeciesType) -> Option<PhenomenonId> {
    match species_type {
        SpeciesType::Herbivore => Some(PhenomenonId(3)),  // PlantGrowth (watching grazing)
        SpeciesType::Carnivore => Some(PhenomenonId(1)),  // Gravity (watching predator-prey chase)
        SpeciesType::Omnivore => Some(PhenomenonId(3)),
        SpeciesType::Producer => Some(PhenomenonId(3)),   // PlantGrowth
        SpeciesType::Decomposer => None,                  // Not observable for discoveries
    }
}

/// Get observation threshold for a species type (higher = harder to discover)
pub fn observation_threshold(species_type: &SpeciesType) -> f32 {
    match species_type {
        SpeciesType::Herbivore => 10.0,
        SpeciesType::Carnivore => 15.0,
        SpeciesType::Omnivore => 12.0,
        SpeciesType::Producer => 8.0,
        SpeciesType::Decomposer => 20.0,
    }
}
