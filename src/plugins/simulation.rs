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
    /// Runtime mode: Normal gameplay with time compression
    Runtime {
        /// Time compression factor (default: 24.0 = 1 real hour becomes 1 in-game day)
        time_compression: f64,
    },
    /// Creative mode: Instant everything, no time pressure
    Creative,
}

impl SimulationMode {
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

