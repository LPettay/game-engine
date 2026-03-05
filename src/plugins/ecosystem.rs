// Ecosystem Simulation Module
// Infinite Universe Engine - Biomes and life simulation
//
// Simulates:
// - Biome distribution based on temperature and precipitation
// - Vegetation growth and succession
// - Simple food chain dynamics
// - Population dynamics

use bevy::prelude::*;
use std::collections::HashMap;

pub struct EcosystemPlugin;

impl Plugin for EcosystemPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EcosystemState>()
           .add_systems(Update, update_ecosystem);
    }
}

/// Global ecosystem state
#[derive(Resource, Default)]
pub struct EcosystemState {
    /// Biome grid (mapped to planet surface)
    pub biome_grid: BiomeGrid,
    /// Species populations
    pub species: HashMap<SpeciesId, Species>,
    /// Food chain relationships
    pub food_chain: FoodChain,
    /// Simulation settings
    pub settings: EcosystemSettings,
}

/// Unique identifier for a species
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpeciesId(pub u32);

/// Biome types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeType {
    // Aquatic
    Ocean,
    DeepOcean,
    CoralReef,
    Freshwater,
    
    // Cold biomes
    Ice,
    Tundra,
    Taiga,
    
    // Temperate biomes
    TemperateForest,
    TemperateGrassland,
    Mediterranean,
    
    // Warm biomes
    TropicalRainforest,
    TropicalSeasonal,
    Savanna,
    
    // Dry biomes
    Desert,
    Shrubland,
    
    // Altitude-based
    Alpine,
    Mountain,
    
    // Special
    Volcanic,
    Cave,
}

impl BiomeType {
    /// Get biome from temperature and precipitation
    pub fn from_climate(temp: f64, precip: f64, elevation: f64) -> Self {
        // Temperature in Celsius, precipitation in mm/year, elevation in meters
        
        // High altitude overrides
        if elevation > 4000.0 {
            return BiomeType::Alpine;
        }
        if elevation > 2500.0 {
            return BiomeType::Mountain;
        }
        
        // Temperature-based classification
        if temp < -15.0 {
            return BiomeType::Ice;
        }
        if temp < 0.0 {
            if precip < 300.0 {
                return BiomeType::Tundra;
            }
            return BiomeType::Taiga;
        }
        
        // Temperate zone (0-20°C)
        if temp < 20.0 {
            if precip < 250.0 {
                return BiomeType::Desert;
            }
            if precip < 500.0 {
                return BiomeType::Shrubland;
            }
            if precip < 1000.0 {
                return BiomeType::TemperateGrassland;
            }
            return BiomeType::TemperateForest;
        }
        
        // Tropical zone (>20°C)
        if precip < 250.0 {
            return BiomeType::Desert;
        }
        if precip < 500.0 {
            return BiomeType::Savanna;
        }
        if precip < 1500.0 {
            return BiomeType::TropicalSeasonal;
        }
        BiomeType::TropicalRainforest
    }
    
    /// Get the base productivity of this biome (kg biomass / m² / year)
    pub fn productivity(&self) -> f64 {
        match self {
            BiomeType::TropicalRainforest => 2.2,
            BiomeType::TemperateForest => 1.2,
            BiomeType::Taiga => 0.8,
            BiomeType::TropicalSeasonal => 1.5,
            BiomeType::TemperateGrassland => 0.6,
            BiomeType::Savanna => 0.9,
            BiomeType::Shrubland => 0.4,
            BiomeType::Tundra => 0.14,
            BiomeType::Desert => 0.03,
            BiomeType::Alpine => 0.1,
            BiomeType::Ocean => 0.125,
            BiomeType::DeepOcean => 0.003,
            BiomeType::CoralReef => 2.5,
            BiomeType::Freshwater => 0.8,
            _ => 0.1,
        }
    }
    
    /// Get biodiversity index (species richness multiplier)
    pub fn biodiversity(&self) -> f64 {
        match self {
            BiomeType::TropicalRainforest => 1.0,
            BiomeType::CoralReef => 0.9,
            BiomeType::TemperateForest => 0.5,
            BiomeType::TropicalSeasonal => 0.6,
            BiomeType::Ocean => 0.4,
            BiomeType::Savanna => 0.4,
            BiomeType::TemperateGrassland => 0.3,
            BiomeType::Taiga => 0.2,
            BiomeType::Desert => 0.1,
            BiomeType::Tundra => 0.05,
            BiomeType::Ice => 0.02,
            _ => 0.2,
        }
    }
}

/// Grid of biomes covering the planet
#[derive(Clone)]
pub struct BiomeGrid {
    pub resolution: (usize, usize),
    pub biomes: Vec<BiomeType>,
    pub vegetation_density: Vec<f64>,
    pub soil_fertility: Vec<f64>,
}

impl Default for BiomeGrid {
    fn default() -> Self {
        let res = (72, 36); // 5-degree resolution
        let size = res.0 * res.1;
        Self {
            resolution: res,
            biomes: vec![BiomeType::Ocean; size],
            vegetation_density: vec![0.0; size],
            soil_fertility: vec![0.5; size],
        }
    }
}

impl BiomeGrid {
    pub fn index(&self, x: usize, y: usize) -> usize {
        y * self.resolution.0 + x
    }
    
    /// Get biome at a position (spherical coordinates)
    pub fn get_biome(&self, lon: f64, lat: f64) -> BiomeType {
        let (nx, ny) = self.resolution;
        let gx = ((lon / (2.0 * std::f64::consts::PI) + 0.5) * nx as f64) as usize % nx;
        let gy = ((lat / std::f64::consts::PI + 0.5) * ny as f64).clamp(0.0, (ny - 1) as f64) as usize;
        self.biomes[self.index(gx, gy)]
    }
    
    /// Update biomes based on climate
    pub fn update_from_climate(&mut self, temp_grid: &[f64], precip_grid: &[f64], elevation_grid: &[f64]) {
        for i in 0..self.biomes.len() {
            let temp = temp_grid.get(i).copied().unwrap_or(288.0) - 273.15; // Convert K to C
            let precip = precip_grid.get(i).copied().unwrap_or(1000.0);
            let elevation = elevation_grid.get(i).copied().unwrap_or(0.0);
            
            self.biomes[i] = BiomeType::from_climate(temp, precip, elevation);
            
            // Vegetation density based on biome productivity
            self.vegetation_density[i] = self.biomes[i].productivity() / 2.5;
        }
    }
}

/// A species in the ecosystem
#[derive(Clone, Debug)]
pub struct Species {
    pub id: SpeciesId,
    pub name: String,
    pub species_type: SpeciesType,
    /// Population count
    pub population: f64,
    /// Preferred biomes
    pub preferred_biomes: Vec<BiomeType>,
    /// Energy requirements (kg biomass / individual / year)
    pub energy_requirement: f64,
    /// Reproduction rate (offspring / individual / year)
    pub reproduction_rate: f64,
    /// Mortality rate (deaths / individual / year)
    pub mortality_rate: f64,
    /// Carrying capacity multiplier
    pub carrying_capacity_mult: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeciesType {
    Producer,   // Plants, algae
    Herbivore,  // Plant eaters
    Carnivore,  // Meat eaters
    Omnivore,   // Both
    Decomposer, // Break down dead matter
}

/// Food chain relationships
#[derive(Clone, Default)]
pub struct FoodChain {
    /// Predator-prey relationships (predator -> list of prey)
    pub predation: HashMap<SpeciesId, Vec<SpeciesId>>,
    /// Competition relationships (species -> competitors)
    pub competition: HashMap<SpeciesId, Vec<SpeciesId>>,
}

impl FoodChain {
    pub fn add_predation(&mut self, predator: SpeciesId, prey: SpeciesId) {
        self.predation.entry(predator).or_default().push(prey);
    }
    
    pub fn add_competition(&mut self, species1: SpeciesId, species2: SpeciesId) {
        self.competition.entry(species1).or_default().push(species2);
        self.competition.entry(species2).or_default().push(species1);
    }
}

/// Ecosystem simulation settings
#[derive(Clone, Debug)]
pub struct EcosystemSettings {
    /// Time step (years)
    pub dt_years: f64,
    /// Global productivity multiplier
    pub productivity_mult: f64,
    /// Enable extinctions
    pub extinctions_enabled: bool,
    /// Enable speciation (new species evolution)
    pub speciation_enabled: bool,
}

impl Default for EcosystemSettings {
    fn default() -> Self {
        Self {
            dt_years: 1.0,
            productivity_mult: 1.0,
            extinctions_enabled: true,
            speciation_enabled: true,
        }
    }
}

/// Update ecosystem simulation
fn update_ecosystem(
    mut state: ResMut<EcosystemState>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    let dt_years = dt * state.settings.dt_years;
    
    // Collect species IDs to iterate
    let species_ids: Vec<SpeciesId> = state.species.keys().cloned().collect();
    
    // Update each species population
    for id in species_ids {
        // Get data needed for calculation
        let (energy_req, repro_rate, mort_rate, pop, species_type, cap_mult) = {
            let species = match state.species.get(&id) {
                Some(s) => s,
                None => continue,
            };
            (
                species.energy_requirement,
                species.reproduction_rate,
                species.mortality_rate,
                species.population,
                species.species_type.clone(),
                species.carrying_capacity_mult,
            )
        };
        
        // Calculate carrying capacity
        let k = calculate_carrying_capacity(&state.biome_grid, cap_mult);
        
        // Lotka-Volterra population dynamics
        let growth_rate = match species_type {
            SpeciesType::Producer => repro_rate * (1.0 - pop / k),
            SpeciesType::Herbivore => {
                // Depends on plant availability
                let plants = get_producer_biomass(&state);
                repro_rate * (plants / (plants + energy_req)) - mort_rate
            },
            SpeciesType::Carnivore => {
                // Depends on prey availability
                let prey = get_prey_biomass(&state, &state.food_chain, id);
                repro_rate * (prey / (prey + energy_req)) - mort_rate
            },
            SpeciesType::Omnivore => {
                let plants = get_producer_biomass(&state);
                let prey = get_prey_biomass(&state, &state.food_chain, id);
                let food = plants + prey;
                repro_rate * (food / (food + energy_req)) - mort_rate
            },
            SpeciesType::Decomposer => {
                // Always has food (dead matter)
                repro_rate * (1.0 - pop / k)
            },
        };
        
        // Update population
        let extinctions_enabled = state.settings.extinctions_enabled;
        if let Some(species) = state.species.get_mut(&id) {
            species.population = (pop + pop * growth_rate * dt_years).max(0.0);
            
            // Check for extinction
            if extinctions_enabled && species.population < 1.0 {
                species.population = 0.0;
            }
        }
    }
}

fn calculate_carrying_capacity(biome_grid: &BiomeGrid, mult: f64) -> f64 {
    // Sum up productivity across all biomes
    let total_productivity: f64 = biome_grid.biomes.iter()
        .map(|b| b.productivity())
        .sum();
    
    total_productivity * 1e6 * mult // Scale to reasonable numbers
}

fn get_producer_biomass(state: &EcosystemState) -> f64 {
    state.species.values()
        .filter(|s| s.species_type == SpeciesType::Producer)
        .map(|s| s.population)
        .sum()
}

fn get_prey_biomass(state: &EcosystemState, food_chain: &FoodChain, predator: SpeciesId) -> f64 {
    let prey_ids = food_chain.predation.get(&predator);
    match prey_ids {
        Some(ids) => {
            ids.iter()
                .filter_map(|id| state.species.get(id))
                .map(|s| s.population)
                .sum()
        },
        None => 0.0,
    }
}

/// Initialize a simple ecosystem
pub fn create_simple_ecosystem() -> EcosystemState {
    let mut state = EcosystemState::default();
    
    // Add producers (plants)
    let plants = SpeciesId(1);
    state.species.insert(plants, Species {
        id: plants,
        name: "Vegetation".to_string(),
        species_type: SpeciesType::Producer,
        population: 1e9,
        preferred_biomes: vec![BiomeType::TemperateForest, BiomeType::TropicalRainforest],
        energy_requirement: 0.0,
        reproduction_rate: 0.5,
        mortality_rate: 0.1,
        carrying_capacity_mult: 1.0,
    });
    
    // Add herbivores
    let herbivores = SpeciesId(2);
    state.species.insert(herbivores, Species {
        id: herbivores,
        name: "Herbivores".to_string(),
        species_type: SpeciesType::Herbivore,
        population: 1e7,
        preferred_biomes: vec![BiomeType::TemperateGrassland, BiomeType::Savanna],
        energy_requirement: 100.0,
        reproduction_rate: 0.3,
        mortality_rate: 0.2,
        carrying_capacity_mult: 0.1,
    });
    
    // Add carnivores
    let carnivores = SpeciesId(3);
    state.species.insert(carnivores, Species {
        id: carnivores,
        name: "Carnivores".to_string(),
        species_type: SpeciesType::Carnivore,
        population: 1e5,
        preferred_biomes: vec![BiomeType::TemperateForest, BiomeType::Savanna],
        energy_requirement: 500.0,
        reproduction_rate: 0.15,
        mortality_rate: 0.1,
        carrying_capacity_mult: 0.01,
    });
    
    // Set up food chain
    state.food_chain.add_predation(herbivores, plants);
    state.food_chain.add_predation(carnivores, herbivores);
    
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_biome_classification() {
        // Tropical rainforest: hot and wet
        assert_eq!(BiomeType::from_climate(25.0, 3000.0, 100.0), BiomeType::TropicalRainforest);
        
        // Desert: hot and dry
        assert_eq!(BiomeType::from_climate(30.0, 100.0, 100.0), BiomeType::Desert);
        
        // Tundra: cold and dry
        assert_eq!(BiomeType::from_climate(-5.0, 200.0, 100.0), BiomeType::Tundra);
        
        // Alpine: high elevation
        assert_eq!(BiomeType::from_climate(5.0, 1000.0, 5000.0), BiomeType::Alpine);
    }
    
    #[test]
    fn test_simple_ecosystem() {
        let ecosystem = create_simple_ecosystem();
        assert_eq!(ecosystem.species.len(), 3);
        assert!(!ecosystem.food_chain.predation.is_empty());
    }
}

