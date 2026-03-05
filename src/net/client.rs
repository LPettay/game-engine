// SpacetimeDB Connection Manager
// Handles connect/disconnect/reconnect and table subscription callbacks.
// When SpacetimeDB SDK is unavailable, operates in offline-only mode.

use bevy::prelude::*;
use super::tables::*;
use super::{SpacetimeClient, ConnectionState, NetEvent};

/// Manage connection lifecycle
pub fn manage_connection(
    mut client: ResMut<SpacetimeClient>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    match client.state {
        ConnectionState::Disconnected => {
            // Attempt connection on first frame
            if !client.connection_attempted {
                info!("[NET] Attempting connection to SpacetimeDB...");
                client.state = ConnectionState::Connecting;
                client.connection_attempted = true;
            }
        }
        ConnectionState::Connecting => {
            // Timeout after 5 seconds → go to offline mode
            client.connect_timer += dt;
            if client.connect_timer > 5.0 {
                info!("[NET] Connection timed out — running in offline mode");
                client.state = ConnectionState::Offline;
            }
            // When SpacetimeDB SDK is available, actual connection happens here.
            // For now, immediately go offline.
            client.state = ConnectionState::Offline;
        }
        ConnectionState::Connected => {
            // Poll for updates — placeholder for SDK subscription polling
            client.uptime += dt;
        }
        ConnectionState::Reconnecting => {
            client.reconnect_timer += dt;
            if client.reconnect_timer > 10.0 {
                client.state = ConnectionState::Offline;
                info!("[NET] Reconnection failed — offline mode");
            }
        }
        ConnectionState::Offline => {
            // Operating without server — all state is local
        }
        ConnectionState::Error(ref _msg) => {
            // Wait before retry
            client.reconnect_timer += dt;
            if client.reconnect_timer > 30.0 {
                client.reconnect_timer = 0.0;
                client.state = ConnectionState::Reconnecting;
            }
        }
    }
}

/// Poll SpacetimeDB for table updates
pub fn poll_spacetimedb(
    client: Res<SpacetimeClient>,
    mut net_events: MessageWriter<NetEvent>,
) {
    if client.state != ConnectionState::Connected {
        return;
    }

    // When SDK is available:
    // - Poll subscription updates
    // - Emit NetEvent for each table change
    // - Sync remote state to local resources
}

/// Sync local Bevy resources from network state
pub fn sync_local_state(
    client: Res<SpacetimeClient>,
    // Would sync CollectiveKnowledge, ServerState, etc. from SpacetimeDB tables
) {
    if client.state != ConnectionState::Connected {
        return;
    }

    // Placeholder: when connected, read cached table data
    // and update local Bevy resources
}

// ============================================================================
// TABLE SUBSCRIPTION CALLBACKS (for when SDK is integrated)
// ============================================================================

/// Called when a player row is inserted/updated
pub fn on_player_update(player: &PlayerTable) {
    info!("[NET] Player update: {} (online: {})", player.name, player.online);
}

/// Called when a knowledge row changes
pub fn on_knowledge_update(knowledge: &PlayerKnowledgeTable) {
    info!(
        "[NET] Knowledge update for player {}: {} discoveries",
        knowledge.identity,
        knowledge.discovered.len()
    );
}

/// Called when a new discovery is inserted
pub fn on_discovery_insert(discovery: &DiscoveryTable) {
    info!(
        "[NET] New discovery: phenomenon {} by player {} (first: {})",
        discovery.phenomenon_id, discovery.discoverer, discovery.is_first
    );
}
