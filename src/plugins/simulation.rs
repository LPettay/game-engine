// Simulation Time Quanta System
// Infinite Universe Engine - Core time management for physics simulation
//
// Supports two modes:
// 1. Pre-bake: Accelerated simulation for generating planetary history (billions of years in seconds)
// 2. Runtime: Compressed time for gameplay (1 real hour = 1 in-game day)
//
// All physics systems tick at discrete quanta to ensure deterministic simulation

use bevy::prelude::*;
use std::time::Duration;

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationConfig>()
           .init_resource::<SimulationTime>()
           .init_resource::<DifficultyConfig>()
           .add_message::<SimulationEvent>()
           .add_systems(Update, (
               update_simulation_time,
               process_simulation_events,
           ).chain());
    }
}

/// Configuration for the simulation system
#[derive(Resource, Clone)]
pub struct SimulationConfig {
    /// Current simulation mode
    pub mode: SimulationMode,
    /// Whether simulation is paused
    pub paused: bool,
    /// Discrete time steps per real second (physics update rate)
    pub quanta_per_second: u32,
    /// Target physics tick rate in Hz
    pub physics_tick_rate: f64,
    /// Accumulated time since last physics tick
    pub accumulated_time: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            mode: SimulationMode::Runtime {
                time_compression: 24.0, // 1 real hour = 1 in-game day
            },
            paused: false,
            quanta_per_second: 60, // 60 physics ticks per second
            physics_tick_rate: 60.0,
            accumulated_time: 0.0,
        }
    }
}

/// Simulation mode determines how time flows
#[derive(Clone, Debug, PartialEq)]
pub enum SimulationMode {
    /// Pre-bake mode: Accelerated simulation for generating history
    PreBake {
        /// Target universe age to simulate until (in seconds since Big Bang)
        target_age: f64,
        /// How many universe-years pass per real second
        years_per_second: f64,
        /// Current progress (0.0 to 1.0)
        progress: f64,
    },
    /// Runtime mode: Normal gameplay with time compression
    Runtime {
        /// Time compression factor (default: 24.0 = 1 real hour becomes 1 in-game day)
        time_compression: f64,
    },
    /// Creative mode: Instant everything, no time pressure
    Creative,
}

impl SimulationMode {
    /// Create a pre-bake mode for generating 4.5 billion years of history
    pub fn earth_like_prebake() -> Self {
        Self::PreBake {
            target_age: 4.5e9 * 365.25 * 24.0 * 3600.0, // 4.5 billion years in seconds
            years_per_second: 1e9, // 1 billion years per real second
            progress: 0.0,
        }
    }
    
    /// Standard runtime mode with 24x time compression
    pub fn standard_runtime() -> Self {
        Self::Runtime {
            time_compression: 24.0,
        }
    }
    
    /// Accelerated runtime for faster progression
    pub fn accelerated_runtime() -> Self {
        Self::Runtime {
            time_compression: 168.0, // 1 real hour = 1 in-game week
        }
    }
    
    /// Get the time multiplier for this mode
    pub fn time_multiplier(&self) -> f64 {
        match self {
            SimulationMode::PreBake { years_per_second, .. } => {
                years_per_second * 365.25 * 24.0 * 3600.0 // Convert years/sec to seconds/sec
            }
            SimulationMode::Runtime { time_compression } => *time_compression,
            SimulationMode::Creative => 1.0,
        }
    }
}

/// Tracks the current time in the simulation
#[derive(Resource, Clone, Debug)]
pub struct SimulationTime {
    /// Universe time in seconds since the Big Bang (simulation origin)
    /// For a Earth-like planet, this might start at ~4.5 billion years
    pub universe_age: f64,
    /// Time elapsed since simulation started (in universe time)
    pub simulation_elapsed: f64,
    /// Real-world time since simulation started
    pub real_elapsed: Duration,
    /// Current in-game date/time (for display)
    pub calendar: GameCalendar,
    /// Number of physics ticks completed
    pub tick_count: u64,
}

impl Default for SimulationTime {
    fn default() -> Self {
        Self {
            universe_age: 0.0,
            simulation_elapsed: 0.0,
            real_elapsed: Duration::ZERO,
            calendar: GameCalendar::default(),
            tick_count: 0,
        }
    }
}

impl SimulationTime {
    /// Create simulation time starting at a specific universe age
    pub fn at_age(universe_age_years: f64) -> Self {
        let universe_age = universe_age_years * 365.25 * 24.0 * 3600.0;
        Self {
            universe_age,
            simulation_elapsed: 0.0,
            real_elapsed: Duration::ZERO,
            calendar: GameCalendar::from_simulation_seconds(0.0),
            tick_count: 0,
        }
    }
    
    /// Get universe age in years
    pub fn universe_age_years(&self) -> f64 {
        self.universe_age / (365.25 * 24.0 * 3600.0)
    }
    
    /// Get simulation elapsed time in days
    pub fn elapsed_days(&self) -> f64 {
        self.simulation_elapsed / (24.0 * 3600.0)
    }
}

/// In-game calendar for player-facing time display
#[derive(Clone, Debug, Default)]
pub struct GameCalendar {
    /// Year (starting from year 1)
    pub year: u32,
    /// Day of year (1-365)
    pub day_of_year: u16,
    /// Hour of day (0-23)
    pub hour: u8,
    /// Minute of hour (0-59)
    pub minute: u8,
    /// Season based on day of year
    pub season: Season,
}

impl GameCalendar {
    /// Create calendar from simulation seconds (since player spawn)
    pub fn from_simulation_seconds(seconds: f64) -> Self {
        let days = seconds / (24.0 * 3600.0);
        let year = (days / 365.25).floor() as u32 + 1;
        let day_of_year = ((days % 365.25).floor() as u16).max(1);
        let hours = (seconds % (24.0 * 3600.0)) / 3600.0;
        let hour = hours.floor() as u8;
        let minute = ((hours - hour as f64) * 60.0).floor() as u8;
        
        let season = match day_of_year {
            1..=91 => Season::Spring,
            92..=182 => Season::Summer,
            183..=273 => Season::Autumn,
            _ => Season::Winter,
        };
        
        Self {
            year,
            day_of_year,
            hour,
            minute,
            season,
        }
    }
    
    /// Format as readable string
    pub fn format(&self) -> String {
        format!(
            "Year {}, Day {} ({:?}) {:02}:{:02}",
            self.year, self.day_of_year, self.season, self.hour, self.minute
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Season {
    #[default]
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// Difficulty modifiers that affect gameplay speed
#[derive(Resource, Clone)]
pub struct DifficultyConfig {
    /// Multiplier for resource gathering speed (> 1.0 = faster/easier)
    pub resource_multiplier: f64,
    /// Multiplier for survival needs decay rate (< 1.0 = slower/easier)
    pub survival_leniency: f64,
    /// Multiplier for discovery/learning speed (> 1.0 = faster)
    pub learning_rate: f64,
    /// Multiplier for crafting time (< 1.0 = faster)
    pub crafting_speed: f64,
    /// Enable/disable permadeath
    pub permadeath: bool,
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        Self {
            resource_multiplier: 2.0,   // Resources gather 2x faster than reality
            survival_leniency: 0.5,     // Hunger/thirst decay at half speed
            learning_rate: 2.0,         // Discoveries happen 2x faster
            crafting_speed: 0.25,       // Crafting is 4x faster than reality
            permadeath: false,
        }
    }
}

impl DifficultyConfig {
    pub fn easy() -> Self {
        Self {
            resource_multiplier: 4.0,
            survival_leniency: 0.25,
            learning_rate: 4.0,
            crafting_speed: 0.1,
            permadeath: false,
        }
    }
    
    pub fn normal() -> Self {
        Self::default()
    }
    
    pub fn hard() -> Self {
        Self {
            resource_multiplier: 1.0,
            survival_leniency: 1.0,
            learning_rate: 1.0,
            crafting_speed: 1.0,
            permadeath: false,
        }
    }
    
    pub fn hardcore() -> Self {
        Self {
            resource_multiplier: 0.5,
            survival_leniency: 1.5,
            learning_rate: 0.5,
            crafting_speed: 2.0,
            permadeath: true,
        }
    }
}

/// Events that affect simulation state
#[derive(Message)]
pub enum SimulationEvent {
    /// Pause the simulation
    Pause,
    /// Resume the simulation
    Resume,
    /// Toggle pause state
    TogglePause,
    /// Change simulation mode
    SetMode(SimulationMode),
    /// Pre-bake phase completed
    PreBakeComplete {
        final_age: f64,
        duration: Duration,
    },
    /// New day started (for day/night cycle)
    NewDay { day_number: u32 },
    /// New year started
    NewYear { year_number: u32 },
    /// Season changed
    SeasonChanged { new_season: Season },
}

/// System to update simulation time each frame
fn update_simulation_time(
    time: Res<Time>,
    mut sim_config: ResMut<SimulationConfig>,
    mut sim_time: ResMut<SimulationTime>,
    mut events: MessageWriter<SimulationEvent>,
) {
    if sim_config.paused {
        return;
    }
    
    let real_dt = time.delta_secs_f64();
    sim_time.real_elapsed += Duration::from_secs_f64(real_dt);
    
    match &mut sim_config.mode {
        SimulationMode::PreBake { target_age, years_per_second, progress } => {
            // In pre-bake mode, advance time rapidly
            let universe_dt = real_dt * *years_per_second * 365.25 * 24.0 * 3600.0;
            sim_time.universe_age += universe_dt;
            sim_time.simulation_elapsed += universe_dt;
            
            // Update progress
            *progress = (sim_time.universe_age / *target_age).min(1.0);
            
            // Check if pre-bake is complete
            if sim_time.universe_age >= *target_age {
                events.write(SimulationEvent::PreBakeComplete {
                    final_age: sim_time.universe_age,
                    duration: sim_time.real_elapsed,
                });
                // Switch to runtime mode
                sim_config.mode = SimulationMode::standard_runtime();
            }
        }
        SimulationMode::Runtime { time_compression } => {
            // In runtime mode, advance time with compression
            let universe_dt = real_dt * *time_compression;
            sim_time.universe_age += universe_dt;
            sim_time.simulation_elapsed += universe_dt;
            
            // Update calendar and check for day/year changes
            let old_calendar = sim_time.calendar.clone();
            sim_time.calendar = GameCalendar::from_simulation_seconds(sim_time.simulation_elapsed);
            
            // Send events for calendar changes
            if sim_time.calendar.day_of_year != old_calendar.day_of_year {
                let day_number = (sim_time.simulation_elapsed / (24.0 * 3600.0)).floor() as u32;
                events.write(SimulationEvent::NewDay { day_number });
            }
            
            if sim_time.calendar.year != old_calendar.year {
                events.write(SimulationEvent::NewYear { year_number: sim_time.calendar.year });
            }
            
            if sim_time.calendar.season != old_calendar.season {
                events.write(SimulationEvent::SeasonChanged { new_season: sim_time.calendar.season.clone() });
            }
        }
        SimulationMode::Creative => {
            // In creative mode, time passes at normal rate
            sim_time.universe_age += real_dt;
            sim_time.simulation_elapsed += real_dt;
            sim_time.calendar = GameCalendar::from_simulation_seconds(sim_time.simulation_elapsed);
        }
    }
    
    // Accumulate time for physics ticks
    sim_config.accumulated_time += real_dt;
    
    // Process physics ticks at fixed rate
    let tick_dt = 1.0 / sim_config.physics_tick_rate;
    while sim_config.accumulated_time >= tick_dt {
        sim_config.accumulated_time -= tick_dt;
        sim_time.tick_count += 1;
    }
}

/// System to process simulation events
fn process_simulation_events(
    mut events: MessageReader<SimulationEvent>,
    mut sim_config: ResMut<SimulationConfig>,
) {
    for event in events.read() {
        match event {
            SimulationEvent::Pause => {
                sim_config.paused = true;
                info!("[Simulation] Paused");
            }
            SimulationEvent::Resume => {
                sim_config.paused = false;
                info!("[Simulation] Resumed");
            }
            SimulationEvent::TogglePause => {
                sim_config.paused = !sim_config.paused;
                info!("[Simulation] {}", if sim_config.paused { "Paused" } else { "Resumed" });
            }
            SimulationEvent::SetMode(mode) => {
                sim_config.mode = mode.clone();
                info!("[Simulation] Mode changed to {:?}", mode);
            }
            SimulationEvent::PreBakeComplete { final_age, duration } => {
                let years = final_age / (365.25 * 24.0 * 3600.0);
                info!("[Simulation] Pre-bake complete: {:.2} billion years in {:?}", 
                      years / 1e9, duration);
            }
            SimulationEvent::NewDay { day_number } => {
                // Could trigger day-based events here
                if *day_number % 30 == 0 {
                    info!("[Simulation] Day {}", day_number);
                }
            }
            SimulationEvent::NewYear { year_number } => {
                info!("[Simulation] Year {} begins", year_number);
            }
            SimulationEvent::SeasonChanged { new_season } => {
                info!("[Simulation] Season changed to {:?}", new_season);
            }
        }
    }
}

/// Helper trait for applying difficulty modifiers
pub trait DifficultyAdjusted {
    fn apply_resource_modifier(&self, base_value: f64, difficulty: &DifficultyConfig) -> f64;
    fn apply_survival_modifier(&self, base_decay: f64, difficulty: &DifficultyConfig) -> f64;
    fn apply_learning_modifier(&self, base_rate: f64, difficulty: &DifficultyConfig) -> f64;
    fn apply_crafting_modifier(&self, base_time: f64, difficulty: &DifficultyConfig) -> f64;
}

impl DifficultyAdjusted for f64 {
    fn apply_resource_modifier(&self, base_value: f64, difficulty: &DifficultyConfig) -> f64 {
        base_value * difficulty.resource_multiplier
    }
    
    fn apply_survival_modifier(&self, base_decay: f64, difficulty: &DifficultyConfig) -> f64 {
        base_decay * difficulty.survival_leniency
    }
    
    fn apply_learning_modifier(&self, base_rate: f64, difficulty: &DifficultyConfig) -> f64 {
        base_rate * difficulty.learning_rate
    }
    
    fn apply_crafting_modifier(&self, base_time: f64, difficulty: &DifficultyConfig) -> f64 {
        base_time * difficulty.crafting_speed
    }
}

/// Component for entities that need time-based updates
#[derive(Component)]
pub struct TimeDependent {
    /// Last update tick
    pub last_tick: u64,
    /// Update frequency (ticks between updates)
    pub update_frequency: u64,
}

impl Default for TimeDependent {
    fn default() -> Self {
        Self {
            last_tick: 0,
            update_frequency: 1, // Update every tick
        }
    }
}

impl TimeDependent {
    pub fn every_n_ticks(n: u64) -> Self {
        Self {
            last_tick: 0,
            update_frequency: n,
        }
    }
    
    /// Check if this entity should update this tick
    pub fn should_update(&mut self, current_tick: u64) -> bool {
        if current_tick >= self.last_tick + self.update_frequency {
            self.last_tick = current_tick;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calendar_conversion() {
        // One day = 86400 seconds
        let cal = GameCalendar::from_simulation_seconds(86400.0);
        assert_eq!(cal.year, 1);
        assert_eq!(cal.day_of_year, 2); // Day 1 is first day, after 1 day we're on day 2
        
        // One year
        let cal = GameCalendar::from_simulation_seconds(365.25 * 24.0 * 3600.0);
        assert_eq!(cal.year, 2);
    }
    
    #[test]
    fn test_time_compression() {
        let runtime = SimulationMode::Runtime { time_compression: 24.0 };
        assert_eq!(runtime.time_multiplier(), 24.0);
        
        // 1 real hour = 24 simulation hours = 1 day
        // In 1 real hour (3600 real seconds), 86400 sim seconds pass
        let sim_seconds = 3600.0 * 24.0;
        assert_eq!(sim_seconds, 86400.0);
    }
}

