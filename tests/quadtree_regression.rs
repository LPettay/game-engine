//! Quadtree regression tests
//!
//! These tests verify that the subdivision budget prevents frame freezes
//! when the camera jumps from orbital to surface altitude.

use bevy::prelude::*;
use genesis_engine::plugins::quadtree::{QuadtreeManager, QuadtreeAddress};

/// Helper: create an initialized manager with default 20km planet radius
fn make_manager() -> QuadtreeManager {
    let mut m = QuadtreeManager::new(20000.0);
    m.initialize();
    m
}

/// THE regression test: simulates an Orbital → Surface camera switch.
/// With budget 8, each frame should produce at most 32 spawns (8 × 4 children).
/// Without budget, this used to freeze for seconds (910 → 13,006 nodes in one frame).
#[test]
fn test_orbital_to_surface_bounded() {
    let mut m = make_manager();

    // Let the tree stabilize at orbital altitude first
    let orbital = Vec3::Y * 100_000.0;
    for _ in 0..50 {
        m.update(orbital, 8);
    }
    let orbital_nodes = m.nodes.len();

    // Teleport to just above the surface
    let surface = Vec3::Y * 20_001.0;
    let (spawned, _) = m.update(surface, 8);

    // Budget 8 = at most 8 subdivisions × 4 children = 32 new nodes per frame
    assert!(
        spawned.len() <= 32,
        "Surface frame should spawn at most 32 nodes with budget 8, got {}",
        spawned.len()
    );

    // Compare: without budget, unlimited subdivisions happen
    let mut m_unlimited = make_manager();
    for _ in 0..50 {
        m_unlimited.update(orbital, 0);
    }
    let (spawned_unlimited, _) = m_unlimited.update(surface, 0);

    // The unlimited version should produce significantly more spawns
    assert!(
        spawned_unlimited.len() > spawned.len(),
        "Unlimited should produce more spawns ({}) than budgeted ({})",
        spawned_unlimited.len(),
        spawned.len()
    );
}

/// Budget of 2 should produce at most 8 spawns per frame
#[test]
fn test_subdivision_budget_limits_growth() {
    let mut m = make_manager();
    let surface = Vec3::Y * 20_001.0;
    let (spawned, _) = m.update(surface, 2);
    assert!(
        spawned.len() <= 8,
        "Budget 2 should produce at most 8 spawns, got {}",
        spawned.len()
    );
}

/// After enough budgeted updates, the tree should stabilize (converge)
/// — no more spawns or despawns once the observer is stationary.
#[test]
fn test_progressive_deepening_converges() {
    let mut m = make_manager();
    let surface = Vec3::Y * 20_001.0;

    let mut total_spawned = 0;
    let mut zero_count = 0;

    // Run up to 1000 frames — aggressive thresholds need more iterations
    for _ in 0..1000 {
        let (spawned, _) = m.update(surface, 8);
        total_spawned += spawned.len();

        if spawned.is_empty() {
            zero_count += 1;
            // 5 consecutive idle frames = converged
            if zero_count >= 5 {
                break;
            }
        } else {
            zero_count = 0;
        }
    }

    assert!(
        zero_count >= 5,
        "Quadtree should converge (5 idle frames) within 1000 updates (total spawned: {})",
        total_spawned
    );
}

/// Nodes should not oscillate between subdivide and merge when the
/// observer is stationary at a boundary. The hysteresis gap between
/// subdivision_threshold (8.0) and merge_threshold (12.0) prevents this.
#[test]
fn test_merge_hysteresis() {
    let mut m = make_manager();

    // Get the tree to a stable state near the surface (allow plenty of iterations)
    let surface = Vec3::Y * 20_001.0;
    for _ in 0..1000 {
        let (spawned, _) = m.update(surface, 8);
        if spawned.is_empty() {
            break;
        }
    }

    let node_count_before = m.nodes.len();

    // Run 10 more frames at the exact same position
    let mut any_change = false;
    for _ in 0..10 {
        let (spawned, despawned) = m.update(surface, 8);
        if !spawned.is_empty() || !despawned.is_empty() {
            any_change = true;
        }
    }

    assert!(
        !any_change,
        "Stable observer should not cause oscillation (nodes before: {}, after: {})",
        node_count_before,
        m.nodes.len()
    );
}

/// Verifies that the tree starts with exactly 6 root nodes (one per cube face)
#[test]
fn test_initial_state() {
    let m = make_manager();
    assert_eq!(m.nodes.len(), 6, "Should start with 6 root nodes (one per face)");

    for face in 0..6 {
        let root = QuadtreeAddress::root(face);
        assert!(m.nodes.contains_key(&root), "Missing root for face {}", face);
    }
}

/// Smaller budget produces strictly fewer or equal spawns per frame
#[test]
fn test_budget_ordering() {
    let surface = Vec3::Y * 20_001.0;

    let mut m2 = make_manager();
    let (s2, _) = m2.update(surface, 2);

    let mut m8 = make_manager();
    let (s8, _) = m8.update(surface, 8);

    assert!(
        s2.len() <= s8.len(),
        "Budget 2 ({}) should produce <= spawns than budget 8 ({})",
        s2.len(),
        s8.len()
    );
}

/// With budget 1, only the closest candidate subdivides.
/// Its children should be on the face nearest the observer.
#[test]
fn test_closest_face_subdivides_first() {
    let mut m = make_manager();
    // Observer near the +Y face surface (face 0)
    let surface = Vec3::Y * 20_001.0;
    let (spawned, _) = m.update(surface, 1);

    // Budget 1 = exactly 1 subdivision = 4 children
    assert_eq!(spawned.len(), 4, "Budget 1 should produce exactly 4 spawns");

    // All children should be on face 0 (+Y), the closest face
    for addr in &spawned {
        assert_eq!(
            addr.face, 0,
            "Near +Y surface, budget 1 should subdivide face 0, got face {}",
            addr.face
        );
    }
}

/// Priority ordering: spawned addresses should be sorted closest-first
#[test]
fn test_spawned_sorted_by_distance() {
    let mut m = make_manager();
    let surface = Vec3::Y * 20_001.0;
    let (spawned, _) = m.update(surface, 4);

    // Compute distances for spawned addresses
    let distances: Vec<f32> = spawned
        .iter()
        .map(|addr| {
            let center = m.nodes.get(addr).unwrap().world_center;
            (surface - center).length()
        })
        .collect();

    // Verify sorted (each distance <= next)
    for i in 1..distances.len() {
        assert!(
            distances[i - 1] <= distances[i] + 0.01, // small epsilon for float comparison
            "Spawned list not sorted by distance: [{:.1}] at {} > [{:.1}] at {}",
            distances[i - 1], i - 1, distances[i], i
        );
    }
}
