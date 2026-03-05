// Materials Properties Module
// Continuous material properties for emergent technology
//
// Unlike block-based games, materials have real physical properties
// that determine behavior at all scales.

use bevy::prelude::*;
use std::collections::HashMap;

/// Unique identifier for a material type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialId(pub u32);

/// Complete physical properties of a material
/// These drive all emergent behavior in the simulation
#[derive(Clone, Debug)]
pub struct MaterialProperties {
    /// Human-readable name
    pub name: String,
    
    // Electrical properties
    /// Electrical conductivity (S/m - Siemens per meter)
    /// Copper: 5.96e7, Iron: 1.0e7, Glass: 1e-12
    pub electrical_conductivity: f64,
    /// Dielectric constant (relative permittivity)
    /// Vacuum: 1.0, Water: 80, Glass: 4-10
    pub dielectric_constant: f64,
    /// Band gap in eV (None = conductor, Some = semiconductor/insulator)
    /// Silicon: 1.1, Diamond: 5.5
    pub band_gap: Option<f64>,
    
    // Thermal properties
    /// Thermal conductivity (W/(m·K))
    /// Copper: 400, Iron: 80, Wood: 0.1
    pub thermal_conductivity: f64,
    /// Specific heat capacity (J/(kg·K))
    /// Water: 4186, Iron: 449
    pub specific_heat: f64,
    /// Melting point (K)
    pub melting_point: f64,
    /// Boiling point (K)
    pub boiling_point: f64,
    /// Heat of fusion (J/kg)
    pub heat_of_fusion: f64,
    /// Heat of vaporization (J/kg)
    pub heat_of_vaporization: f64,
    
    // Mechanical properties
    /// Density (kg/m³)
    pub density: f64,
    /// Young's modulus / Elastic modulus (Pa)
    /// Steel: 200e9, Rubber: 0.01e9
    pub elastic_modulus: f64,
    /// Tensile strength (Pa)
    /// Steel: 400e6, Concrete: 3e6
    pub tensile_strength: f64,
    /// Compressive strength (Pa)
    pub compressive_strength: f64,
    /// Hardness (Mohs scale 1-10 or Vickers)
    pub hardness: f64,
    /// Poisson's ratio (dimensionless, typically 0.2-0.5)
    pub poisson_ratio: f64,
    
    // Chemical properties
    /// Electronegativity (Pauling scale)
    pub electronegativity: f64,
    /// Oxidation states (common valences)
    pub oxidation_states: Vec<i8>,
    /// Is this material reactive with water?
    pub water_reactive: bool,
    /// Is this material reactive with oxygen?
    pub oxygen_reactive: bool,
    /// Corrosion resistance (0-1, 1 = completely resistant)
    pub corrosion_resistance: f64,
    
    // Optical properties
    /// Refractive index
    /// Glass: 1.5, Diamond: 2.4, Water: 1.33
    pub refractive_index: f64,
    /// Transparency (0-1)
    pub transparency: f64,
    /// Reflectivity (0-1)
    pub reflectivity: f64,
    /// Emissivity (0-1) for thermal radiation
    pub emissivity: f64,
    /// Color (as RGB wavelength absorption)
    pub color: Vec3,
    
    // Magnetic properties
    /// Relative magnetic permeability
    /// Iron: ~5000, Copper: 1.0
    pub magnetic_permeability: f64,
    /// Is ferromagnetic?
    pub ferromagnetic: bool,
    
    // Nuclear properties (for advanced gameplay)
    /// Atomic number (0 for compounds)
    pub atomic_number: u8,
    /// Atomic mass (g/mol)
    pub atomic_mass: f64,
    /// Is radioactive?
    pub radioactive: bool,
    /// Half-life in seconds (if radioactive)
    pub half_life: Option<f64>,
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            electrical_conductivity: 1e-12,
            dielectric_constant: 1.0,
            band_gap: Some(5.0),
            thermal_conductivity: 1.0,
            specific_heat: 1000.0,
            melting_point: 1000.0,
            boiling_point: 2000.0,
            heat_of_fusion: 100000.0,
            heat_of_vaporization: 500000.0,
            density: 2000.0,
            elastic_modulus: 50e9,
            tensile_strength: 100e6,
            compressive_strength: 100e6,
            hardness: 5.0,
            poisson_ratio: 0.3,
            electronegativity: 2.0,
            oxidation_states: vec![0],
            water_reactive: false,
            oxygen_reactive: false,
            corrosion_resistance: 0.5,
            refractive_index: 1.5,
            transparency: 0.0,
            reflectivity: 0.5,
            emissivity: 0.5,
            color: Vec3::new(0.5, 0.5, 0.5),
            magnetic_permeability: 1.0,
            ferromagnetic: false,
            atomic_number: 0,
            atomic_mass: 50.0,
            radioactive: false,
            half_life: None,
        }
    }
}

impl MaterialProperties {
    /// Iron
    pub fn iron() -> Self {
        Self {
            name: "Iron".to_string(),
            electrical_conductivity: 1.0e7,
            dielectric_constant: 1.0,
            band_gap: None,
            thermal_conductivity: 80.0,
            specific_heat: 449.0,
            melting_point: 1811.0,
            boiling_point: 3134.0,
            heat_of_fusion: 247000.0,
            heat_of_vaporization: 6090000.0,
            density: 7874.0,
            elastic_modulus: 211e9,
            tensile_strength: 540e6,
            compressive_strength: 540e6,
            hardness: 4.0,
            poisson_ratio: 0.29,
            electronegativity: 1.83,
            oxidation_states: vec![-2, 2, 3, 4, 6],
            water_reactive: false,
            oxygen_reactive: true,
            corrosion_resistance: 0.2,
            refractive_index: 2.95,
            transparency: 0.0,
            reflectivity: 0.65,
            emissivity: 0.7,
            color: Vec3::new(0.56, 0.57, 0.58),
            magnetic_permeability: 5000.0,
            ferromagnetic: true,
            atomic_number: 26,
            atomic_mass: 55.845,
            radioactive: false,
            half_life: None,
        }
    }
    
    /// Copper
    pub fn copper() -> Self {
        Self {
            name: "Copper".to_string(),
            electrical_conductivity: 5.96e7,
            dielectric_constant: 1.0,
            band_gap: None,
            thermal_conductivity: 401.0,
            specific_heat: 385.0,
            melting_point: 1358.0,
            boiling_point: 2835.0,
            heat_of_fusion: 205000.0,
            heat_of_vaporization: 4730000.0,
            density: 8960.0,
            elastic_modulus: 130e9,
            tensile_strength: 220e6,
            compressive_strength: 220e6,
            hardness: 3.0,
            poisson_ratio: 0.34,
            electronegativity: 1.90,
            oxidation_states: vec![1, 2],
            water_reactive: false,
            oxygen_reactive: true,
            corrosion_resistance: 0.6,
            refractive_index: 0.47,
            transparency: 0.0,
            reflectivity: 0.95,
            emissivity: 0.03,
            color: Vec3::new(0.72, 0.45, 0.20),
            magnetic_permeability: 1.0,
            ferromagnetic: false,
            atomic_number: 29,
            atomic_mass: 63.546,
            radioactive: false,
            half_life: None,
        }
    }
    
    /// Silicon
    pub fn silicon() -> Self {
        Self {
            name: "Silicon".to_string(),
            electrical_conductivity: 1.56e-3, // Intrinsic
            dielectric_constant: 11.7,
            band_gap: Some(1.12),
            thermal_conductivity: 149.0,
            specific_heat: 705.0,
            melting_point: 1687.0,
            boiling_point: 3538.0,
            heat_of_fusion: 1787000.0,
            heat_of_vaporization: 359000.0,
            density: 2329.0,
            elastic_modulus: 130e9,
            tensile_strength: 7000e6, // For perfect crystal
            compressive_strength: 7000e6,
            hardness: 6.5,
            poisson_ratio: 0.28,
            electronegativity: 1.90,
            oxidation_states: vec![-4, 4],
            water_reactive: false,
            oxygen_reactive: true,
            corrosion_resistance: 0.9,
            refractive_index: 3.42,
            transparency: 0.0, // Opaque to visible, transparent to IR
            reflectivity: 0.35,
            emissivity: 0.65,
            color: Vec3::new(0.4, 0.4, 0.45),
            magnetic_permeability: 1.0,
            ferromagnetic: false,
            atomic_number: 14,
            atomic_mass: 28.085,
            radioactive: false,
            half_life: None,
        }
    }
    
    /// Gold
    pub fn gold() -> Self {
        Self {
            name: "Gold".to_string(),
            electrical_conductivity: 4.1e7,
            dielectric_constant: 1.0,
            band_gap: None,
            thermal_conductivity: 318.0,
            specific_heat: 129.0,
            melting_point: 1337.0,
            boiling_point: 3129.0,
            heat_of_fusion: 64400.0,
            heat_of_vaporization: 324000.0,
            density: 19300.0,
            elastic_modulus: 78e9,
            tensile_strength: 120e6,
            compressive_strength: 120e6,
            hardness: 2.5,
            poisson_ratio: 0.44,
            electronegativity: 2.54,
            oxidation_states: vec![1, 3],
            water_reactive: false,
            oxygen_reactive: false,
            corrosion_resistance: 0.99,
            refractive_index: 0.47,
            transparency: 0.0,
            reflectivity: 0.95,
            emissivity: 0.02,
            color: Vec3::new(1.0, 0.84, 0.0),
            magnetic_permeability: 1.0,
            ferromagnetic: false,
            atomic_number: 79,
            atomic_mass: 196.967,
            radioactive: false,
            half_life: None,
        }
    }
    
    /// Water (liquid)
    pub fn water() -> Self {
        Self {
            name: "Water".to_string(),
            electrical_conductivity: 5e-6, // Pure water
            dielectric_constant: 80.0,
            band_gap: Some(6.5),
            thermal_conductivity: 0.606,
            specific_heat: 4186.0,
            melting_point: 273.15,
            boiling_point: 373.15,
            heat_of_fusion: 334000.0,
            heat_of_vaporization: 2260000.0,
            density: 1000.0,
            elastic_modulus: 2.2e9, // Bulk modulus
            tensile_strength: 0.0, // Liquids don't have tensile strength
            compressive_strength: 1e9, // Very high
            hardness: 0.0,
            poisson_ratio: 0.5, // Incompressible
            electronegativity: 0.0, // Compound
            oxidation_states: vec![],
            water_reactive: false,
            oxygen_reactive: false,
            corrosion_resistance: 1.0,
            refractive_index: 1.33,
            transparency: 0.99,
            reflectivity: 0.02,
            emissivity: 0.95,
            color: Vec3::new(0.0, 0.3, 0.8),
            magnetic_permeability: 1.0,
            ferromagnetic: false,
            atomic_number: 0, // Compound
            atomic_mass: 18.015,
            radioactive: false,
            half_life: None,
        }
    }
    
    /// Granite (rock)
    pub fn granite() -> Self {
        Self {
            name: "Granite".to_string(),
            electrical_conductivity: 1e-8,
            dielectric_constant: 6.0,
            band_gap: Some(8.0),
            thermal_conductivity: 2.5,
            specific_heat: 790.0,
            melting_point: 1260.0, // Softening point
            boiling_point: 2500.0, // Approximate
            heat_of_fusion: 400000.0,
            heat_of_vaporization: 2000000.0,
            density: 2750.0,
            elastic_modulus: 70e9,
            tensile_strength: 10e6,
            compressive_strength: 200e6,
            hardness: 6.5,
            poisson_ratio: 0.25,
            electronegativity: 0.0,
            oxidation_states: vec![],
            water_reactive: false,
            oxygen_reactive: false,
            corrosion_resistance: 0.95,
            refractive_index: 1.55,
            transparency: 0.0,
            reflectivity: 0.15,
            emissivity: 0.9,
            color: Vec3::new(0.5, 0.5, 0.5),
            magnetic_permeability: 1.0,
            ferromagnetic: false,
            atomic_number: 0,
            atomic_mass: 70.0,
            radioactive: false,
            half_life: None,
        }
    }
    
    /// Calculate electrical resistance for a wire of this material
    pub fn wire_resistance(&self, length: f64, cross_section_area: f64) -> f64 {
        if self.electrical_conductivity > 0.0 {
            length / (self.electrical_conductivity * cross_section_area)
        } else {
            f64::INFINITY
        }
    }
    
    /// Calculate thermal resistance for a slab of this material
    pub fn thermal_resistance(&self, thickness: f64, area: f64) -> f64 {
        if self.thermal_conductivity > 0.0 && area > 0.0 {
            thickness / (self.thermal_conductivity * area)
        } else {
            f64::INFINITY
        }
    }
    
    /// Check if material is a conductor
    pub fn is_conductor(&self) -> bool {
        self.band_gap.is_none() && self.electrical_conductivity > 1e4
    }
    
    /// Check if material is a semiconductor
    pub fn is_semiconductor(&self) -> bool {
        match self.band_gap {
            Some(gap) => gap < 4.0 && gap > 0.0,
            None => false,
        }
    }
    
    /// Check if material is an insulator
    pub fn is_insulator(&self) -> bool {
        match self.band_gap {
            Some(gap) => gap >= 4.0,
            None => false,
        }
    }
}

/// Resource to store all known materials
#[derive(Resource, Default)]
pub struct MaterialRegistry {
    materials: HashMap<MaterialId, MaterialProperties>,
    next_id: u32,
}

impl MaterialRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        
        // Register common materials
        registry.register(MaterialProperties::iron());
        registry.register(MaterialProperties::copper());
        registry.register(MaterialProperties::silicon());
        registry.register(MaterialProperties::gold());
        registry.register(MaterialProperties::water());
        registry.register(MaterialProperties::granite());
        
        registry
    }
    
    pub fn register(&mut self, properties: MaterialProperties) -> MaterialId {
        let id = MaterialId(self.next_id);
        self.next_id += 1;
        self.materials.insert(id, properties);
        id
    }
    
    pub fn get(&self, id: MaterialId) -> Option<&MaterialProperties> {
        self.materials.get(&id)
    }
    
    pub fn find_by_name(&self, name: &str) -> Option<(MaterialId, &MaterialProperties)> {
        self.materials.iter()
            .find(|(_, m)| m.name.eq_ignore_ascii_case(name))
            .map(|(id, m)| (*id, m))
    }
}

/// Calculate alloy properties from constituent materials
pub fn alloy_properties(
    materials: &[(MaterialId, f64)], // (material, mass fraction)
    registry: &MaterialRegistry,
) -> Option<MaterialProperties> {
    if materials.is_empty() {
        return None;
    }
    
    let mut result = MaterialProperties::default();
    result.name = "Alloy".to_string();
    
    let total_fraction: f64 = materials.iter().map(|(_, f)| f).sum();
    if total_fraction < 0.99 || total_fraction > 1.01 {
        return None; // Fractions must sum to 1
    }
    
    // Properties that mix linearly (rule of mixtures)
    for (id, fraction) in materials {
        let mat = registry.get(*id)?;
        
        result.density += mat.density * fraction;
        result.specific_heat += mat.specific_heat * fraction;
        result.thermal_conductivity += mat.thermal_conductivity * fraction;
        result.elastic_modulus += mat.elastic_modulus * fraction;
        result.tensile_strength += mat.tensile_strength * fraction;
        result.color += mat.color * (*fraction as f32);
    }
    
    // Electrical conductivity uses harmonic mean for series connection
    let mut sum_inv_conductivity = 0.0;
    for (id, fraction) in materials {
        let mat = registry.get(*id)?;
        if mat.electrical_conductivity > 0.0 {
            sum_inv_conductivity += fraction / mat.electrical_conductivity;
        }
    }
    if sum_inv_conductivity > 0.0 {
        result.electrical_conductivity = 1.0 / sum_inv_conductivity;
    }
    
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_material_conductor_classification() {
        let copper = MaterialProperties::copper();
        let silicon = MaterialProperties::silicon();
        let granite = MaterialProperties::granite();
        
        assert!(copper.is_conductor());
        assert!(!copper.is_semiconductor());
        
        assert!(silicon.is_semiconductor());
        assert!(!silicon.is_conductor());
        
        assert!(granite.is_insulator());
    }
    
    #[test]
    fn test_wire_resistance() {
        let copper = MaterialProperties::copper();
        
        // 1m wire, 1mm² cross-section
        let r = copper.wire_resistance(1.0, 1e-6);
        // Copper: about 0.017 ohms/m for 1mm² wire
        assert!((r - 0.017).abs() < 0.01);
    }
}

