// Networking Module — SpacetimeDB v2.0 Integration
// Phase 1: Client-only with table definitions.
// Server WASM module is a separate deployment.
//
// Graceful offline mode: the game is fully playable without a server.

pub mod tables;
pub mod client;

use bevy::prelude::*;
use crate::GameState;

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpacetimeClient>()
            .add_message::<NetEvent>()
            .add_systems(
                Update,
                (
                    client::manage_connection,
                    client::poll_spacetimedb,
                    client::sync_local_state,
                    transition_from_connecting,
                )
                    .chain(),
            );
    }
}

// ============================================================================
// RESOURCES
// ============================================================================

/// SpacetimeDB client connection state
#[derive(Resource)]
pub struct SpacetimeClient {
    pub state: ConnectionState,
    pub identity: Option<tables::NetworkPlayerId>,
    pub server_url: String,
    pub connection_attempted: bool,
    pub connect_timer: f32,
    pub reconnect_timer: f32,
    pub uptime: f32,
}

impl Default for SpacetimeClient {
    fn default() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            identity: None,
            server_url: "http://localhost:3000".to_string(),
            connection_attempted: false,
            connect_timer: 0.0,
            reconnect_timer: 0.0,
            uptime: 0.0,
        }
    }
}

/// Connection lifecycle states
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Offline,
    Error(String),
}

// ============================================================================
// EVENTS
// ============================================================================

/// Network events for the rest of the game to react to
#[derive(Message)]
pub enum NetEvent {
    Connected { identity: tables::NetworkPlayerId },
    Disconnected,
    PlayerJoined { identity: tables::NetworkPlayerId, name: String },
    PlayerLeft { identity: tables::NetworkPlayerId },
    DiscoverySync { phenomenon_id: u32, discoverer: tables::NetworkPlayerId },
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Transition from Connecting to Playing once connection resolves
fn transition_from_connecting(
    client: Res<SpacetimeClient>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if *state.get() == GameState::Connecting {
        match client.state {
            ConnectionState::Connected | ConnectionState::Offline => {
                next_state.set(GameState::Playing);
            }
            ConnectionState::Error(_) => {
                // Still transition to playing in offline mode
                next_state.set(GameState::Playing);
            }
            _ => {}
        }
    }
}
