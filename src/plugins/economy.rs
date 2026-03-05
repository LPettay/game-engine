// Emergent Economy System
// Infinite Universe Engine - Supply/demand economy that emerges from physics
//
// Economy arises naturally from:
// - Scarcity of resources
// - Specialization of players
// - Trade between players
// - Manufacturing capabilities
//
// No artificial currencies - value emerges from utility

use bevy::prelude::*;
use std::collections::HashMap;

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EconomyState>()
           .add_message::<TradeEvent>()
           .add_message::<MarketEvent>()
           .add_systems(Update, (
               update_markets,
               process_trades,
           ).chain());
    }
}

/// Unique identifier for a good/resource type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GoodId(pub u32);

/// Global economy state
#[derive(Resource, Default)]
pub struct EconomyState {
    /// All goods that can be traded
    pub goods: HashMap<GoodId, Good>,
    /// Market state for each good
    pub markets: HashMap<GoodId, Market>,
    /// Player inventories
    pub inventories: HashMap<u64, Inventory>,
    /// Active trade offers
    pub trade_offers: Vec<TradeOffer>,
    /// Economic statistics
    pub statistics: EconomicStats,
}

/// A tradeable good
#[derive(Clone, Debug)]
pub struct Good {
    pub id: GoodId,
    pub name: String,
    pub category: GoodCategory,
    /// Base resource cost to produce
    pub base_production_cost: f64,
    /// Physical properties (affects transport)
    pub mass_per_unit: f64,     // kg
    pub volume_per_unit: f64,   // m³
    /// Durability (does it decay?)
    pub decay_rate: f64,        // fraction per day
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoodCategory {
    RawMaterial,    // Ore, wood, stone
    ProcessedMaterial, // Refined metals, processed materials
    Component,      // Parts for technology
    Finished,       // Complete products
    Consumable,     // Food, fuel
    Service,        // Non-physical goods
}

/// Market state for a single good
#[derive(Clone, Debug, Default)]
pub struct Market {
    /// Current price (in trade value units)
    pub price: f64,
    /// Available supply
    pub supply: f64,
    /// Current demand
    pub demand: f64,
    /// Price history
    pub price_history: Vec<(f64, f64)>, // (time, price)
    /// Supply elasticity
    pub supply_elasticity: f64,
    /// Demand elasticity
    pub demand_elasticity: f64,
}

impl Market {
    pub fn new(initial_price: f64) -> Self {
        Self {
            price: initial_price,
            supply: 0.0,
            demand: 0.0,
            price_history: vec![(0.0, initial_price)],
            supply_elasticity: 1.0,
            demand_elasticity: 1.0,
        }
    }
    
    /// Update price based on supply and demand
    pub fn update_price(&mut self, dt: f64) {
        // Price increases when demand > supply
        // P' = P * (1 + k * (D - S) / max(S, 1))
        let imbalance = (self.demand - self.supply) / self.supply.max(1.0);
        let adjustment = 0.1 * imbalance * dt;
        
        self.price *= 1.0 + adjustment;
        self.price = self.price.max(0.01); // Minimum price
    }
}

/// A player's inventory
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    /// Goods owned
    pub goods: HashMap<GoodId, f64>,
    /// Storage capacity (m³)
    pub capacity: f64,
    /// Current usage (m³)
    pub used: f64,
}

impl Inventory {
    pub fn new(capacity: f64) -> Self {
        Self {
            capacity,
            ..default()
        }
    }
    
    pub fn add(&mut self, good_id: GoodId, amount: f64, volume_per_unit: f64) -> bool {
        let required_space = amount * volume_per_unit;
        if self.used + required_space > self.capacity {
            return false;
        }
        
        *self.goods.entry(good_id).or_insert(0.0) += amount;
        self.used += required_space;
        true
    }
    
    pub fn remove(&mut self, good_id: GoodId, amount: f64, volume_per_unit: f64) -> bool {
        if let Some(current) = self.goods.get_mut(&good_id) {
            if *current >= amount {
                *current -= amount;
                self.used -= amount * volume_per_unit;
                return true;
            }
        }
        false
    }
    
    pub fn get(&self, good_id: GoodId) -> f64 {
        self.goods.get(&good_id).copied().unwrap_or(0.0)
    }
}

/// A trade offer
#[derive(Clone, Debug)]
pub struct TradeOffer {
    pub id: u64,
    pub seller: u64,
    pub good: GoodId,
    pub amount: f64,
    pub asking_price: f64, // Per unit
    pub created_at: f64,
    pub expires_at: f64,
}

/// Trade event
#[derive(Message)]
pub struct TradeEvent {
    pub offer_id: u64,
    pub buyer: u64,
    pub seller: u64,
    pub good: GoodId,
    pub amount: f64,
    pub price: f64,
    pub timestamp: f64,
}

/// Market event
#[derive(Message)]
pub enum MarketEvent {
    PriceChange { good: GoodId, old_price: f64, new_price: f64 },
    ShortageWarning { good: GoodId, severity: f64 },
    SurplusWarning { good: GoodId, severity: f64 },
}

/// Economic statistics
#[derive(Clone, Debug, Default)]
pub struct EconomicStats {
    /// Total value of goods traded
    pub total_trade_volume: f64,
    /// Number of trades
    pub trade_count: u64,
    /// Average prices over time
    pub average_prices: HashMap<GoodId, f64>,
    /// GDP estimate (total production value)
    pub gdp: f64,
}

/// Update all markets
fn update_markets(
    mut state: ResMut<EconomyState>,
    mut market_events: MessageWriter<MarketEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    if dt <= 0.0 {
        return;
    }
    
    let current_time = 0.0; // Would be simulation time
    
    for (good_id, market) in state.markets.iter_mut() {
        let old_price = market.price;
        market.update_price(dt);
        
        // Record price history
        market.price_history.push((current_time, market.price));
        
        // Keep history bounded
        if market.price_history.len() > 1000 {
            market.price_history.remove(0);
        }
        
        // Emit events for significant changes
        if (market.price - old_price).abs() / old_price > 0.1 {
            market_events.write(MarketEvent::PriceChange {
                good: *good_id,
                old_price,
                new_price: market.price,
            });
        }
        
        // Shortage/surplus warnings
        if market.supply < market.demand * 0.5 {
            market_events.write(MarketEvent::ShortageWarning {
                good: *good_id,
                severity: 1.0 - (market.supply / market.demand),
            });
        } else if market.supply > market.demand * 2.0 {
            market_events.write(MarketEvent::SurplusWarning {
                good: *good_id,
                severity: market.supply / market.demand - 1.0,
            });
        }
    }
}

/// Process completed trades
fn process_trades(
    mut events: MessageReader<TradeEvent>,
    mut state: ResMut<EconomyState>,
) {
    for event in events.read() {
        // Update market statistics
        if let Some(market) = state.markets.get_mut(&event.good) {
            market.supply -= event.amount;
        }
        
        // Update economic stats
        state.statistics.total_trade_volume += event.amount * event.price;
        state.statistics.trade_count += 1;
        
        info!(
            "[Economy] Trade: {} units of {:?} sold for {} per unit",
            event.amount, event.good, event.price
        );
    }
}

/// Calculate the value of goods based on production chain
pub fn calculate_production_value(
    good: &Good,
    economy: &EconomyState,
    labor_cost: f64,
) -> f64 {
    // Value = raw material cost + labor + margin
    good.base_production_cost + labor_cost * 1.5
}

/// Find best trade opportunities (arbitrage)
pub fn find_arbitrage_opportunities(
    economy: &EconomyState,
    min_profit_margin: f64,
) -> Vec<ArbitrageOpportunity> {
    let mut opportunities = Vec::new();
    
    // This would compare prices across different locations/markets
    // In a single-market system, there's no arbitrage
    
    opportunities
}

#[derive(Clone, Debug)]
pub struct ArbitrageOpportunity {
    pub good: GoodId,
    pub buy_location: String,
    pub sell_location: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub profit_margin: f64,
}

/// Initialize basic economy with common goods
pub fn initialize_basic_economy() -> EconomyState {
    let mut state = EconomyState::default();
    
    // Raw materials
    let iron_ore = GoodId(1);
    state.goods.insert(iron_ore, Good {
        id: iron_ore,
        name: "Iron Ore".to_string(),
        category: GoodCategory::RawMaterial,
        base_production_cost: 1.0,
        mass_per_unit: 1.0,
        volume_per_unit: 0.001,
        decay_rate: 0.0,
    });
    state.markets.insert(iron_ore, Market::new(1.0));
    
    let copper_ore = GoodId(2);
    state.goods.insert(copper_ore, Good {
        id: copper_ore,
        name: "Copper Ore".to_string(),
        category: GoodCategory::RawMaterial,
        base_production_cost: 2.0,
        mass_per_unit: 1.0,
        volume_per_unit: 0.001,
        decay_rate: 0.0,
    });
    state.markets.insert(copper_ore, Market::new(2.0));
    
    // Processed materials
    let iron_ingot = GoodId(10);
    state.goods.insert(iron_ingot, Good {
        id: iron_ingot,
        name: "Iron Ingot".to_string(),
        category: GoodCategory::ProcessedMaterial,
        base_production_cost: 5.0,
        mass_per_unit: 1.0,
        volume_per_unit: 0.0001,
        decay_rate: 0.001, // Slight oxidation
    });
    state.markets.insert(iron_ingot, Market::new(5.0));
    
    let copper_wire = GoodId(11);
    state.goods.insert(copper_wire, Good {
        id: copper_wire,
        name: "Copper Wire".to_string(),
        category: GoodCategory::ProcessedMaterial,
        base_production_cost: 8.0,
        mass_per_unit: 0.1,
        volume_per_unit: 0.00001,
        decay_rate: 0.0,
    });
    state.markets.insert(copper_wire, Market::new(8.0));
    
    // Components
    let circuit_board = GoodId(100);
    state.goods.insert(circuit_board, Good {
        id: circuit_board,
        name: "Circuit Board".to_string(),
        category: GoodCategory::Component,
        base_production_cost: 50.0,
        mass_per_unit: 0.01,
        volume_per_unit: 0.0001,
        decay_rate: 0.0,
    });
    state.markets.insert(circuit_board, Market::new(50.0));
    
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_market_price_update() {
        let mut market = Market::new(10.0);
        
        market.supply = 100.0;
        market.demand = 150.0; // Demand exceeds supply
        
        market.update_price(1.0);
        
        // Price should increase
        assert!(market.price > 10.0);
    }
    
    #[test]
    fn test_inventory() {
        let mut inv = Inventory::new(10.0);
        let good = GoodId(1);
        
        // Add items
        assert!(inv.add(good, 5.0, 1.0));
        assert_eq!(inv.get(good), 5.0);
        
        // Try to add more than capacity
        assert!(!inv.add(good, 10.0, 1.0));
        
        // Remove items
        assert!(inv.remove(good, 3.0, 1.0));
        assert_eq!(inv.get(good), 2.0);
    }
}

