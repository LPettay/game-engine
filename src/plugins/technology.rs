// Emergent Technology Module
// Infinite Universe Engine - Scale-invariant technology from physics
//
// Unlike Minecraft's redstone, technology here emerges from actual physics:
// - Electricity flows through conductors based on material properties
// - Heat dissipates based on thermal conductivity
// - Miniaturization is possible through manufacturing precision
//
// Players design circuits and machines, not follow recipes.

use bevy::prelude::*;
use std::collections::HashMap;

pub struct TechnologyPlugin;

impl Plugin for TechnologyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TechnologyState>()
           .init_resource::<CircuitSimulator>()
           .add_systems(Update, (
               simulate_circuits,
               update_machines,
           ).chain());
    }
}

// ============================================================================
// TECHNOLOGY STATE
// ============================================================================

#[derive(Resource, Default)]
pub struct TechnologyState {
    /// All technology designs
    pub designs: HashMap<DesignId, TechnologyDesign>,
    /// All built instances
    pub instances: HashMap<InstanceId, TechnologyInstance>,
    /// Manufacturing capabilities
    pub manufacturing: ManufacturingCapabilities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DesignId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u32);

/// A player-designed technology
#[derive(Clone, Debug)]
pub struct TechnologyDesign {
    pub id: DesignId,
    pub name: String,
    pub creator: u64,
    pub design_type: DesignType,
    /// Components that make up this design
    pub components: Vec<ComponentSpec>,
    /// Connections between components
    pub connections: Vec<Connection>,
    /// Physical dimensions (meters)
    pub dimensions: Vec3,
    /// Required manufacturing precision (meters)
    pub required_precision: f64,
    /// Knowledge required to design this
    pub required_knowledge: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DesignType {
    Circuit,          // Electrical circuit
    Mechanism,        // Mechanical device
    Hybrid,           // Electromechanical
    Thermal,          // Heat-based device
    Optical,          // Light-based device
    Chemical,         // Chemical processing
    Quantum,          // Quantum device (advanced)
}

/// Specification for a component in a design
#[derive(Clone, Debug)]
pub struct ComponentSpec {
    pub component_type: ComponentType,
    pub position: Vec3, // Relative position in design
    pub orientation: Quat,
    pub material: u32, // Material ID
    pub dimensions: Vec3,
}

#[derive(Clone, Debug)]
pub enum ComponentType {
    // Electrical
    Wire { cross_section: f64 },
    Resistor { resistance: f64 },
    Capacitor { capacitance: f64 },
    Inductor { inductance: f64 },
    Transistor { gain: f64, threshold: f64 },
    Diode { forward_voltage: f64 },
    
    // Mechanical
    Gear { teeth: u32, module: f64 },
    Lever { fulcrum: f64 },
    Spring { stiffness: f64 },
    Shaft { diameter: f64 },
    Bearing { friction: f64 },
    
    // Other
    HeatSink { surface_area: f64 },
    Lens { focal_length: f64 },
    Mirror { reflectivity: f64 },
    Container { volume: f64 },
}

/// Connection between components
#[derive(Clone, Debug)]
pub struct Connection {
    pub from_component: usize,
    pub from_port: String,
    pub to_component: usize,
    pub to_port: String,
    pub connection_type: ConnectionType,
}

#[derive(Clone, Debug)]
pub enum ConnectionType {
    Electrical,
    Mechanical,
    Thermal,
    Optical,
    Fluid,
}

/// An instantiated piece of technology
#[derive(Clone, Debug)]
pub struct TechnologyInstance {
    pub id: InstanceId,
    pub design: DesignId,
    pub entity: Entity,
    /// Current state of the instance
    pub state: InstanceState,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Manufacturing quality (affects performance)
    pub quality: f64,
    /// Current operating conditions
    pub operating: OperatingState,
}

#[derive(Clone, Debug)]
pub struct InstanceState {
    /// Is the device powered on?
    pub powered: bool,
    /// Current mode of operation
    pub mode: u32,
    /// Component-specific states
    pub component_states: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct PerformanceMetrics {
    /// Power consumption (Watts)
    pub power_consumption: f64,
    /// Power output (Watts)
    pub power_output: f64,
    /// Efficiency (0-1)
    pub efficiency: f64,
    /// Heat generated (Watts)
    pub heat_generation: f64,
    /// Operating temperature (K)
    pub temperature: f64,
    /// Reliability (0-1)
    pub reliability: f64,
}

#[derive(Clone, Debug, Default)]
pub struct OperatingState {
    /// Input voltages
    pub input_voltages: Vec<f64>,
    /// Input currents
    pub input_currents: Vec<f64>,
    /// Output voltages
    pub output_voltages: Vec<f64>,
    /// Output currents
    pub output_currents: Vec<f64>,
}

// ============================================================================
// MANUFACTURING
// ============================================================================

#[derive(Clone, Debug)]
pub struct ManufacturingCapabilities {
    /// Minimum achievable precision (meters)
    pub precision: f64,
    /// Available manufacturing processes
    pub processes: Vec<ManufacturingProcess>,
    /// Material availability
    pub materials: Vec<u32>,
}

impl Default for ManufacturingCapabilities {
    fn default() -> Self {
        Self {
            precision: 0.001, // 1mm starting precision
            processes: vec![ManufacturingProcess::HandCrafting],
            materials: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ManufacturingProcess {
    pub name: String,
    pub tier: ManufacturingTier,
    /// Minimum feature size (meters)
    pub precision: f64,
    /// Cost multiplier
    pub cost_mult: f64,
    /// Time multiplier
    pub time_mult: f64,
    /// Required knowledge
    pub required_knowledge: Vec<u32>,
}

impl ManufacturingProcess {
    pub const HandCrafting: Self = Self {
        name: String::new(), // Will be set properly
        tier: ManufacturingTier::Primitive,
        precision: 0.01, // 1cm
        cost_mult: 1.0,
        time_mult: 10.0,
        required_knowledge: Vec::new(),
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManufacturingTier {
    /// Hand-forging, basic tools
    Primitive,   // ~1cm precision
    /// Lathes, mills
    Mechanical,  // ~0.1mm precision
    /// CNC, injection molding
    Precision,   // ~0.01mm precision
    /// Photolithography
    Micro,       // ~1μm precision
    /// EUV, molecular assembly
    Nano,        // ~10nm precision
    /// Atomic manipulation
    Quantum,     // ~0.1nm precision
}

// ============================================================================
// CIRCUIT SIMULATION
// ============================================================================

#[derive(Resource, Default)]
pub struct CircuitSimulator {
    /// Node voltages
    pub node_voltages: Vec<f64>,
    /// Branch currents
    pub branch_currents: Vec<f64>,
    /// Simulation settings
    pub settings: CircuitSettings,
}

#[derive(Clone, Debug)]
pub struct CircuitSettings {
    /// Simulation time step
    pub dt: f64,
    /// Maximum iterations for convergence
    pub max_iterations: usize,
    /// Convergence tolerance
    pub tolerance: f64,
}

impl Default for CircuitSettings {
    fn default() -> Self {
        Self {
            dt: 1e-6, // 1 microsecond
            max_iterations: 100,
            tolerance: 1e-9,
        }
    }
}

/// Simulate all active circuits
fn simulate_circuits(
    mut tech_state: ResMut<TechnologyState>,
    simulator: Res<CircuitSimulator>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // Get all powered instances with their design ids
    let instance_data: Vec<(InstanceId, DesignId)> = tech_state.instances
        .iter()
        .filter(|(_, inst)| inst.state.powered)
        .map(|(id, inst)| (*id, inst.design))
        .collect();
    
    for (instance_id, design_id) in instance_data {
        // First, get the design type (immutable borrow)
        let design_type = tech_state.designs.get(&design_id)
            .map(|d| d.design_type.clone());
        
        // Then, get the design and instance for simulation
        if let (Some(design), Some(instance)) = (
            tech_state.designs.get(&design_id).cloned(),
            tech_state.instances.get_mut(&instance_id)
        ) {
            // Simulate based on design type
            match design_type {
                Some(DesignType::Circuit) | Some(DesignType::Hybrid) => {
                    simulate_electrical(&design, instance, &simulator.settings, dt);
                },
                Some(DesignType::Mechanism) => {
                    simulate_mechanical(&design, instance, dt);
                },
                Some(DesignType::Thermal) => {
                    simulate_thermal(&design, instance, dt);
                },
                _ => {}
            }
        }
    }
}

fn simulate_electrical(
    design: &TechnologyDesign,
    instance: &mut TechnologyInstance,
    settings: &CircuitSettings,
    dt: f64,
) {
    // Simple electrical simulation
    // Real implementation would use nodal analysis (SPICE-like)
    
    let mut total_power = 0.0;
    let mut total_heat = 0.0;
    
    for (i, component) in design.components.iter().enumerate() {
        match &component.component_type {
            ComponentType::Resistor { resistance } => {
                // Get voltage across resistor (simplified)
                let voltage = instance.operating.input_voltages.get(0).copied().unwrap_or(0.0);
                let current = voltage / resistance;
                let power = voltage * current;
                
                total_power += power;
                total_heat += power; // All power becomes heat in resistor
                
                if let Some(state) = instance.state.component_states.get_mut(i) {
                    *state = current;
                }
            },
            ComponentType::Capacitor { capacitance } => {
                // I = C * dV/dt
                let voltage = instance.operating.input_voltages.get(0).copied().unwrap_or(0.0);
                let prev_voltage = instance.state.component_states.get(i).copied().unwrap_or(0.0);
                let current = capacitance * (voltage - prev_voltage) / dt;
                
                if let Some(state) = instance.state.component_states.get_mut(i) {
                    *state = voltage; // Store voltage for next step
                }
            },
            ComponentType::Transistor { gain, threshold } => {
                // Simple transistor model
                let base_voltage = instance.operating.input_voltages.get(1).copied().unwrap_or(0.0);
                let collector_voltage = instance.operating.input_voltages.get(0).copied().unwrap_or(0.0);
                
                if base_voltage > *threshold {
                    let base_current = (base_voltage - threshold) / 1000.0; // Simplified
                    let collector_current = base_current * gain;
                    
                    if let Some(state) = instance.state.component_states.get_mut(i) {
                        *state = collector_current;
                    }
                    
                    total_power += collector_voltage * collector_current;
                    total_heat += collector_voltage * collector_current * 0.1; // 10% loss
                }
            },
            _ => {}
        }
    }
    
    instance.performance.power_consumption = total_power;
    instance.performance.heat_generation = total_heat;
    instance.performance.temperature += total_heat * dt / 1000.0; // Simplified thermal model
}

fn simulate_mechanical(
    design: &TechnologyDesign,
    instance: &mut TechnologyInstance,
    dt: f64,
) {
    // Mechanical simulation
    // Calculate forces, torques, and motion
    
    let mut total_power = 0.0;
    
    for (i, component) in design.components.iter().enumerate() {
        match &component.component_type {
            ComponentType::Gear { teeth, module } => {
                // Calculate gear ratio and power transmission
                let radius = (*teeth as f64 * *module) / 2.0;
                // Simplified gear calculation
                if let Some(state) = instance.state.component_states.get_mut(i) {
                    *state = radius;
                }
            },
            ComponentType::Spring { stiffness } => {
                // F = k * x
                let displacement = instance.state.component_states.get(i).copied().unwrap_or(0.0);
                let force = stiffness * displacement;
                let energy = 0.5 * stiffness * displacement * displacement;
                // Store force
            },
            _ => {}
        }
    }
    
    instance.performance.power_output = total_power;
}

fn simulate_thermal(
    design: &TechnologyDesign,
    instance: &mut TechnologyInstance,
    dt: f64,
) {
    // Thermal simulation
    for (i, component) in design.components.iter().enumerate() {
        match &component.component_type {
            ComponentType::HeatSink { surface_area } => {
                // Calculate heat dissipation
                let temp = instance.performance.temperature;
                let ambient = 300.0; // K
                let h = 10.0; // Convection coefficient
                
                let heat_dissipated = h * surface_area * (temp - ambient);
                instance.performance.temperature -= heat_dissipated * dt / 1000.0;
            },
            _ => {}
        }
    }
}

/// Update all machines
fn update_machines(
    mut tech_state: ResMut<TechnologyState>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    // Update machine states
    for (_, instance) in tech_state.instances.iter_mut() {
        // Check reliability - devices can fail
        if instance.performance.temperature > 400.0 { // Overheating
            instance.performance.reliability *= 0.999;
        }
        
        // Calculate efficiency
        if instance.performance.power_consumption > 0.0 {
            instance.performance.efficiency = 
                instance.performance.power_output / instance.performance.power_consumption;
        }
    }
}

// ============================================================================
// DESIGN HELPERS
// ============================================================================

/// Create a simple logic gate design
pub fn design_logic_gate(gate_type: LogicGateType) -> TechnologyDesign {
    let (name, component_count) = match gate_type {
        LogicGateType::NOT => ("NOT Gate", 1),
        LogicGateType::AND => ("AND Gate", 2),
        LogicGateType::OR => ("OR Gate", 2),
        LogicGateType::NAND => ("NAND Gate", 3),
        LogicGateType::NOR => ("NOR Gate", 3),
        LogicGateType::XOR => ("XOR Gate", 4),
    };
    
    TechnologyDesign {
        id: DesignId(0),
        name: name.to_string(),
        creator: 0,
        design_type: DesignType::Circuit,
        components: vec![
            ComponentSpec {
                component_type: ComponentType::Transistor { gain: 100.0, threshold: 0.7 },
                position: Vec3::ZERO,
                orientation: Quat::IDENTITY,
                material: 0, // Silicon
                dimensions: Vec3::new(0.001, 0.001, 0.001),
            };
            component_count
        ],
        connections: vec![],
        dimensions: Vec3::new(0.01, 0.01, 0.005),
        required_precision: 1e-6, // 1 micrometer for IC
        required_knowledge: vec![],
    }
}

pub enum LogicGateType {
    NOT,
    AND,
    OR,
    NAND,
    NOR,
    XOR,
}

/// Calculate if a design can be manufactured
pub fn can_manufacture(design: &TechnologyDesign, capabilities: &ManufacturingCapabilities) -> bool {
    capabilities.precision <= design.required_precision
}

/// Estimate manufacturing cost
pub fn estimate_cost(design: &TechnologyDesign, capabilities: &ManufacturingCapabilities) -> f64 {
    let base_cost = design.components.len() as f64 * 10.0;
    let precision_factor = (capabilities.precision / design.required_precision).max(1.0);
    base_cost * precision_factor
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_logic_gate_design() {
        let gate = design_logic_gate(LogicGateType::AND);
        assert_eq!(gate.name, "AND Gate");
        assert_eq!(gate.components.len(), 2);
    }
    
    #[test]
    fn test_manufacturing_check() {
        let design = TechnologyDesign {
            id: DesignId(1),
            name: "Test".to_string(),
            creator: 0,
            design_type: DesignType::Circuit,
            components: vec![],
            connections: vec![],
            dimensions: Vec3::ZERO,
            required_precision: 0.0001, // 0.1mm
            required_knowledge: vec![],
        };
        
        let capabilities = ManufacturingCapabilities::default();
        // Default precision is 1mm, design needs 0.1mm
        assert!(!can_manufacture(&design, &capabilities));
        
        let better_caps = ManufacturingCapabilities {
            precision: 0.00001, // 10 micrometers
            ..default()
        };
        assert!(can_manufacture(&design, &better_caps));
    }
}

