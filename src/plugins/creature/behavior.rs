// Minimal Behavior Tree Engine
// No external crate — simple recursive tree evaluation

/// Result of evaluating a behavior node
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorStatus {
    Running,
    Success,
    Failure,
}

/// Actions a creature can perform
#[derive(Clone, Debug)]
pub enum CreatureAction {
    Wander,
    Forage,
    FleeFrom,
    HuntPrey,
    Rest,
    Socialize,
    Migrate,
    PlayBehavior,
    MarkTerritory,
    CallOut,
}

/// Conditions a creature can check
#[derive(Clone, Debug)]
pub enum CreatureCondition {
    Hungry(f32),        // threshold
    Threatened(f32),    // threat level threshold
    NearPrey(f32),      // detection range
    Tired(f32),         // energy threshold
    NearFlock(f32),     // flock detection range
    TimeOfDay(f32, f32), // (start_hour, end_hour) — not used yet
}

/// A node in the behavior tree
#[derive(Clone, Debug)]
pub enum BehaviorNode {
    /// Run children in sequence; fail on first failure
    Sequence(Vec<BehaviorNode>),
    /// Try children in order; succeed on first success
    Selector(Vec<BehaviorNode>),
    /// Repeat a child N times (0 = forever)
    Repeat(u32, Box<BehaviorNode>),
    /// Invert child result
    Invert(Box<BehaviorNode>),
    /// Only run child if cooldown has elapsed
    Cooldown(f32, Box<BehaviorNode>),
    /// Leaf: perform an action
    Action(CreatureAction),
    /// Leaf: check a condition
    Condition(CreatureCondition),
}

/// Context passed to behavior evaluation
pub struct BehaviorContext {
    pub hunger: f32,
    pub energy: f32,
    pub threat_level: f32,
    pub near_prey: bool,
    pub near_flock: bool,
    pub prey_distance: f32,
    pub flock_distance: f32,
}

/// Evaluate a behavior tree node, returning status and the chosen action (if any)
pub fn evaluate(node: &BehaviorNode, ctx: &BehaviorContext) -> (BehaviorStatus, Option<CreatureAction>) {
    match node {
        BehaviorNode::Sequence(children) => {
            for child in children {
                let (status, action) = evaluate(child, ctx);
                match status {
                    BehaviorStatus::Failure => return (BehaviorStatus::Failure, None),
                    BehaviorStatus::Running => return (BehaviorStatus::Running, action),
                    BehaviorStatus::Success => continue,
                }
            }
            (BehaviorStatus::Success, None)
        }
        BehaviorNode::Selector(children) => {
            for child in children {
                let (status, action) = evaluate(child, ctx);
                match status {
                    BehaviorStatus::Success => return (BehaviorStatus::Success, action),
                    BehaviorStatus::Running => return (BehaviorStatus::Running, action),
                    BehaviorStatus::Failure => continue,
                }
            }
            (BehaviorStatus::Failure, None)
        }
        BehaviorNode::Repeat(_, child) => {
            // Simplified: just run once per tick
            evaluate(child, ctx)
        }
        BehaviorNode::Invert(child) => {
            let (status, action) = evaluate(child, ctx);
            let inverted = match status {
                BehaviorStatus::Success => BehaviorStatus::Failure,
                BehaviorStatus::Failure => BehaviorStatus::Success,
                BehaviorStatus::Running => BehaviorStatus::Running,
            };
            (inverted, action)
        }
        BehaviorNode::Cooldown(_, child) => {
            // Simplified: cooldown tracking handled externally
            evaluate(child, ctx)
        }
        BehaviorNode::Action(action) => {
            (BehaviorStatus::Running, Some(action.clone()))
        }
        BehaviorNode::Condition(condition) => {
            let result = evaluate_condition(condition, ctx);
            (
                if result { BehaviorStatus::Success } else { BehaviorStatus::Failure },
                None,
            )
        }
    }
}

fn evaluate_condition(condition: &CreatureCondition, ctx: &BehaviorContext) -> bool {
    match condition {
        CreatureCondition::Hungry(threshold) => ctx.hunger > *threshold,
        CreatureCondition::Threatened(threshold) => ctx.threat_level > *threshold,
        CreatureCondition::NearPrey(range) => ctx.near_prey && ctx.prey_distance < *range,
        CreatureCondition::Tired(threshold) => ctx.energy < *threshold,
        CreatureCondition::NearFlock(range) => ctx.near_flock && ctx.flock_distance < *range,
        CreatureCondition::TimeOfDay(_, _) => true, // Not implemented yet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_picks_first_success() {
        let tree = BehaviorNode::Selector(vec![
            BehaviorNode::Sequence(vec![
                BehaviorNode::Condition(CreatureCondition::Hungry(0.5)),
                BehaviorNode::Action(CreatureAction::Forage),
            ]),
            BehaviorNode::Action(CreatureAction::Wander),
        ]);

        let ctx = BehaviorContext {
            hunger: 0.8,
            energy: 1.0,
            threat_level: 0.0,
            near_prey: false,
            near_flock: false,
            prey_distance: 100.0,
            flock_distance: 100.0,
        };

        let (status, action) = evaluate(&tree, &ctx);
        assert_eq!(status, BehaviorStatus::Running);
        assert!(matches!(action, Some(CreatureAction::Forage)));
    }
}
