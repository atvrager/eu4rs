use anyhow::Result;
use eu4sim_core::modifiers::TradegoodId;
use eu4sim_core::state::{CountryState, Date, ProvinceState};
use eu4sim_core::{Fixed, WorldState};
use std::collections::HashMap;
use std::path::Path;

pub fn load_initial_state(
    game_path: &Path,
    start_date: Date,
    _rng_seed: u64,
) -> Result<WorldState> {
    // 1. Load Trade Goods
    log::info!("Loading trade goods from {:?}", game_path);
    let tradegoods = eu4data::tradegoods::load_tradegoods(game_path).unwrap_or_default();

    // Sort for deterministic ID assignment
    let mut sorted_goods: Vec<_> = tradegoods.iter().collect();
    sorted_goods.sort_by_key(|(k, _)| *k);

    let mut base_prices = HashMap::new();
    let mut name_to_id = HashMap::new();

    for (idx, (name, data)) in sorted_goods.iter().enumerate() {
        let id = TradegoodId(idx as u16);
        let price = Fixed::from_f32(data.base_price.unwrap_or(0.0));
        base_prices.insert(id, price);
        name_to_id.insert(name.to_string(), id);
        log::debug!("Tradegood {}: {} -> {}", id.0, name, price);
    }
    log::info!("Loaded {} trade goods", base_prices.len());

    // 2. Load Provinces
    log::info!("Loading province history...");
    let (province_history, _) = eu4data::history::load_province_history(game_path)
        .map_err(|e| anyhow::anyhow!("Failed to load provinces: {}", e))?;

    let mut provinces = HashMap::new();
    let mut countries = HashMap::new();

    // First pass: Create provinces and identify countries
    for (id, hist) in province_history {
        // Map trade good
        let goods_id = hist
            .trade_goods
            .and_then(|name| name_to_id.get(&name))
            .copied();

        // Create ProvinceState
        let p = ProvinceState {
            owner: hist.owner.clone(),
            religion: hist.religion.clone(),
            culture: hist.culture.clone(),
            trade_goods_id: goods_id,
            base_tax: Fixed::from_f32(hist.base_tax.unwrap_or(0.0)),
            base_production: Fixed::from_f32(hist.base_production.unwrap_or(0.0)),
            base_manpower: Fixed::from_f32(hist.base_manpower.unwrap_or(0.0)),
        };
        provinces.insert(id, p.clone());

        // Init country if needed
        if let Some(tag) = p.owner {
            countries.entry(tag).or_insert_with(|| CountryState {
                treasury: Fixed::ZERO,
                manpower: Fixed::ZERO,
                prestige: Fixed::ZERO,
                stability: 0,
            });
        }
    }

    log::info!(
        "Loaded {} provinces, {} countries",
        provinces.len(),
        countries.len()
    );

    // 3. Assemble State
    Ok(WorldState {
        date: start_date,
        rng_seed: _rng_seed,
        provinces,
        countries,
        base_goods_prices: base_prices,
        modifiers: Default::default(),
        diplomacy: Default::default(),
        global: Default::default(),
    })
}
