// Simple procedural meshes for creatures
// Follows patterns from vegetation.rs — scaled primitives per species type

use bevy::prelude::*;
use crate::plugins::ecosystem::SpeciesType;

/// Create a mesh for a creature based on species type
pub fn creature_mesh(species_type: &SpeciesType) -> Mesh {
    match species_type {
        SpeciesType::Herbivore => herbivore_mesh(),
        SpeciesType::Carnivore => carnivore_mesh(),
        SpeciesType::Omnivore => herbivore_mesh(),
        SpeciesType::Decomposer => decomposer_mesh(),
        SpeciesType::Producer => decomposer_mesh(), // small blob
    }
}

/// Get the material color for a species type
pub fn creature_color(species_type: &SpeciesType) -> Color {
    match species_type {
        SpeciesType::Herbivore => Color::srgb(0.6, 0.45, 0.3),  // brown
        SpeciesType::Carnivore => Color::srgb(0.7, 0.25, 0.15), // reddish
        SpeciesType::Omnivore => Color::srgb(0.5, 0.4, 0.35),   // tan
        SpeciesType::Decomposer => Color::srgb(0.35, 0.3, 0.2), // dark brown
        SpeciesType::Producer => Color::srgb(0.2, 0.5, 0.2),    // green
    }
}

/// Get the scale for a species type
pub fn creature_scale(species_type: &SpeciesType) -> Vec3 {
    match species_type {
        SpeciesType::Herbivore => Vec3::new(1.2, 1.0, 2.0),   // wide, long body
        SpeciesType::Carnivore => Vec3::new(0.8, 0.9, 2.5),   // sleek, longer
        SpeciesType::Omnivore => Vec3::new(1.0, 1.0, 1.5),    // medium
        SpeciesType::Decomposer => Vec3::splat(0.4),           // small
        SpeciesType::Producer => Vec3::splat(0.3),             // tiny
    }
}

/// Herbivore: capsule body (like a deer/cow silhouette)
fn herbivore_mesh() -> Mesh {
    Capsule3d::new(0.5, 1.0).into()
}

/// Carnivore: capsule body, slightly different proportions
fn carnivore_mesh() -> Mesh {
    Capsule3d::new(0.4, 1.2).into()
}

/// Decomposer: small sphere
fn decomposer_mesh() -> Mesh {
    Sphere::new(0.5).into()
}
