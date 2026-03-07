pub mod plugins;
pub mod physics;
pub mod net;

// Re-export GameState so integration tests and main.rs can both use it
use bevy::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum GameState {
    #[default]
    Loading,
    Connecting,
    Playing,
}
