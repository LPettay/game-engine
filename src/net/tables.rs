// SpacetimeDB Table Schemas
// Defines the authoritative data model for networked state.
// These types mirror SpacetimeDB tables. When the SDK is available,
// they serve as the client-side cache. Without it, they're local-only.

use std::collections::{HashMap, HashSet};

/// Network-safe player identity (will become SpacetimeDB Identity when SDK available)
pub type NetworkPlayerId = u64;

// ============================================================================
// TABLE SCHEMAS
// ============================================================================

/// Player presence and profile
#[derive(Clone, Debug)]
pub struct PlayerTable {
    pub identity: NetworkPlayerId,
    pub name: String,
    pub playtime_secs: u64,
    pub last_login: u64,
    pub online: bool,
}

/// Player's discovered knowledge
#[derive(Clone, Debug)]
pub struct PlayerKnowledgeTable {
    pub identity: NetworkPlayerId,
    pub discovered: HashSet<u32>,    // PhenomenonId values
    pub known: HashSet<u32>,         // Learned from others
    pub unlocked_tech: HashSet<u32>, // TechId values
    pub research_progress: HashMap<u32, f64>,
}

/// A recorded discovery
#[derive(Clone, Debug)]
pub struct DiscoveryTable {
    pub phenomenon_id: u32,
    pub discoverer: NetworkPlayerId,
    pub timestamp: u64,
    pub is_first: bool,
    pub notes: String,
}

/// Player inventory
#[derive(Clone, Debug)]
pub struct InventoryTable {
    pub identity: NetworkPlayerId,
    pub goods: HashMap<u32, f64>, // GoodId -> quantity
    pub capacity: f64,
}

/// Active trade offer
#[derive(Clone, Debug)]
pub struct TradeOfferTable {
    pub id: u64,
    pub seller: NetworkPlayerId,
    pub good_id: u32,
    pub amount: f64,
    pub price: f64,
    pub created_at: u64,
    pub expires_at: u64,
}

/// Historical milestone
#[derive(Clone, Debug)]
pub struct MilestoneTable {
    pub milestone_type: String,
    pub timestamp: u64,
    pub participants: Vec<NetworkPlayerId>,
    pub documentation: String,
}

/// Server-wide singleton state
#[derive(Clone, Debug)]
pub struct ServerStateTable {
    pub uptime_secs: u64,
    pub online_count: u32,
    pub server_type: String,
    pub time_compression: f64,
}

impl Default for ServerStateTable {
    fn default() -> Self {
        Self {
            uptime_secs: 0,
            online_count: 0,
            server_type: "Standard".to_string(),
            time_compression: 24.0,
        }
    }
}
