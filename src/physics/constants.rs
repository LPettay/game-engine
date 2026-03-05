// Physical Constants Module
// Standard physical constants and unit conversions

/// Astronomical distances
pub mod distances {
    /// Astronomical Unit in meters (Earth-Sun distance)
    pub const AU: f64 = 1.496e11;
    /// Light year in meters
    pub const LIGHT_YEAR: f64 = 9.461e15;
    /// Parsec in meters
    pub const PARSEC: f64 = 3.086e16;
    /// Solar radius in meters
    pub const SOLAR_RADIUS: f64 = 6.96e8;
    /// Earth radius in meters
    pub const EARTH_RADIUS: f64 = 6.371e6;
    /// Moon's orbital radius in meters
    pub const LUNAR_DISTANCE: f64 = 3.844e8;
}

/// Masses of celestial bodies
pub mod masses {
    /// Sun's mass in kg
    pub const SOLAR_MASS: f64 = 1.989e30;
    /// Earth's mass in kg
    pub const EARTH_MASS: f64 = 5.972e24;
    /// Moon's mass in kg
    pub const LUNAR_MASS: f64 = 7.342e22;
    /// Jupiter's mass in kg
    pub const JUPITER_MASS: f64 = 1.898e27;
    /// Proton mass in kg
    pub const PROTON_MASS: f64 = 1.673e-27;
    /// Electron mass in kg
    pub const ELECTRON_MASS: f64 = 9.109e-31;
}

/// Temperature scales
pub mod temperatures {
    /// Absolute zero in Kelvin
    pub const ABSOLUTE_ZERO: f64 = 0.0;
    /// Water freezing point (K)
    pub const WATER_FREEZING: f64 = 273.15;
    /// Water boiling point at 1 atm (K)
    pub const WATER_BOILING: f64 = 373.15;
    /// Room temperature (K)
    pub const ROOM_TEMPERATURE: f64 = 293.15;
    /// Sun's surface temperature (K)
    pub const SOLAR_SURFACE: f64 = 5778.0;
    /// Sun's core temperature (K)
    pub const SOLAR_CORE: f64 = 1.57e7;
    /// Cosmic microwave background (K)
    pub const CMB_TEMPERATURE: f64 = 2.725;
}

/// Time units
pub mod time {
    /// Seconds in a minute
    pub const MINUTE: f64 = 60.0;
    /// Seconds in an hour
    pub const HOUR: f64 = 3600.0;
    /// Seconds in a day
    pub const DAY: f64 = 86400.0;
    /// Seconds in a year (365.25 days)
    pub const YEAR: f64 = 31_557_600.0;
    /// Age of the universe in seconds (~13.8 billion years)
    pub const UNIVERSE_AGE: f64 = 4.35e17;
}

/// Common material densities (kg/m³)
pub mod densities {
    /// Air at sea level
    pub const AIR: f64 = 1.225;
    /// Pure water at 4°C
    pub const WATER: f64 = 1000.0;
    /// Ice
    pub const ICE: f64 = 917.0;
    /// Iron
    pub const IRON: f64 = 7874.0;
    /// Steel (average)
    pub const STEEL: f64 = 7850.0;
    /// Gold
    pub const GOLD: f64 = 19300.0;
    /// Granite
    pub const GRANITE: f64 = 2750.0;
    /// Earth average density
    pub const EARTH_AVERAGE: f64 = 5515.0;
    /// Sun average density
    pub const SUN_AVERAGE: f64 = 1408.0;
}

/// Pressure units and references
pub mod pressure {
    /// Standard atmospheric pressure (Pa)
    pub const ATMOSPHERE: f64 = 101325.0;
    /// Pressure at Earth's core (Pa, approximate)
    pub const EARTH_CORE: f64 = 3.6e11;
    /// Pressure at Sun's core (Pa, approximate)
    pub const SOLAR_CORE: f64 = 2.5e16;
}

/// Unit conversion helpers
pub mod conversions {
    /// Convert Celsius to Kelvin
    pub fn celsius_to_kelvin(c: f64) -> f64 {
        c + 273.15
    }
    
    /// Convert Kelvin to Celsius
    pub fn kelvin_to_celsius(k: f64) -> f64 {
        k - 273.15
    }
    
    /// Convert kilometers to meters
    pub fn km_to_m(km: f64) -> f64 {
        km * 1000.0
    }
    
    /// Convert meters to kilometers
    pub fn m_to_km(m: f64) -> f64 {
        m / 1000.0
    }
    
    /// Convert years to seconds
    pub fn years_to_seconds(years: f64) -> f64 {
        years * super::time::YEAR
    }
    
    /// Convert seconds to years
    pub fn seconds_to_years(seconds: f64) -> f64 {
        seconds / super::time::YEAR
    }
    
    /// Convert AU to meters
    pub fn au_to_m(au: f64) -> f64 {
        au * super::distances::AU
    }
    
    /// Convert meters to AU
    pub fn m_to_au(m: f64) -> f64 {
        m / super::distances::AU
    }
}

