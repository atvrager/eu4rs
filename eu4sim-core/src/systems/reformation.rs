//! Reformation spread system.
//!
//! Handles the Protestant and Reformed Reformations:
//! - Fires at historical dates (1517, 1536)
//! - Creates Centers of Reformation
//! - Spreads religion to adjacent provinces

use crate::state::{ProvinceId, WorldState};
use eu4data::adjacency::AdjacencyGraph;

/// German HRE province IDs for Protestant spawn
const GERMAN_PROVINCES: &[ProvinceId] = &[
    50, // Brandenburg
    61, // Magdeburg
    65, // Saxony
    67, // Thuringia
    57, // Brunswick
    52, // Mecklenburg
];

/// Swiss/French province IDs for Reformed spawn
const SWISS_PROVINCES: &[ProvinceId] = &[
    165, // Bern
    166, // Zurich
    196, // Geneva
    193, // Vaud
];

/// Run the reformation system (called monthly).
pub fn run_reformation_tick(state: &mut WorldState, adjacency: Option<&AdjacencyGraph>) {
    // Only run on first of month
    if state.date.day != 1 {
        return;
    }

    check_protestant_reformation(state);
    check_reformed_reformation(state);
    process_centers(state, adjacency);
    expire_centers(state);
}

fn check_protestant_reformation(state: &mut WorldState) {
    if state.global.reformation.protestant_reformation_fired {
        return;
    }

    // Fire on October 31, 1517 (95 Theses)
    if state.date.year >= 1517 && state.date.month >= 10 {
        log::info!("The Protestant Reformation has begun!");
        state.global.reformation.protestant_reformation_fired = true;
        spawn_centers(state, "protestant", GERMAN_PROVINCES, 3);
    }
}

fn check_reformed_reformation(state: &mut WorldState) {
    if state.global.reformation.reformed_reformation_fired {
        return;
    }

    // Fire in 1536 (Calvin's Institutes)
    if state.date.year >= 1536 {
        log::info!("The Reformed movement has begun!");
        state.global.reformation.reformed_reformation_fired = true;
        spawn_centers(state, "reformed", SWISS_PROVINCES, 3);
    }
}

fn spawn_centers(state: &mut WorldState, religion: &str, candidates: &[ProvinceId], count: usize) {
    // Find Catholic provinces from candidates, sorted by dev
    let mut catholic_provinces: Vec<_> = candidates
        .iter()
        .filter_map(|&id| {
            let prov = state.provinces.get(&id)?;
            if prov.religion.as_deref() == Some("catholic") && prov.owner.is_some() {
                let dev = prov.base_tax + prov.base_production + prov.base_manpower;
                Some((id, dev))
            } else {
                None
            }
        })
        .collect();

    // Sort by development (highest first)
    catholic_provinces.sort_by(|a, b| b.1.cmp(&a.1));

    // Create centers in top provinces
    for (id, _) in catholic_provinces.into_iter().take(count) {
        if let Some(prov) = state.provinces.get_mut(&id) {
            prov.religion = Some(religion.to_string());
            state
                .global
                .reformation
                .centers_of_reformation
                .insert(id, religion.to_string());
            state
                .global
                .reformation
                .center_creation_dates
                .insert(id, state.date);
            log::info!(
                "Center of Reformation ({}) created in province {}",
                religion,
                id
            );
        }
    }
}

fn process_centers(state: &mut WorldState, adjacency: Option<&AdjacencyGraph>) {
    let Some(adj) = adjacency else { return };

    // Collect candidates: (neighbor_id, religion, threshold)
    let mut candidates: Vec<(ProvinceId, String, f32)> = Vec::new();

    for (&center_id, religion) in &state.global.reformation.centers_of_reformation {
        // Get adjacent provinces
        let neighbors = adj.neighbors(center_id);

        for neighbor_id in neighbors {
            let Some(neighbor) = state.provinces.get(&neighbor_id) else {
                continue;
            };

            // Only convert Catholic provinces
            if neighbor.religion.as_deref() != Some("catholic") {
                continue;
            }

            // Skip unowned (wasteland)
            if neighbor.owner.is_none() {
                continue;
            }

            // Calculate conversion chance
            // Mission objective: determine probability based on development
            // Higher development = more resistance to religious change
            let dev = neighbor.base_tax + neighbor.base_production + neighbor.base_manpower;
            let dev_f32 = dev.to_f32();
            let base_chance = 0.02; // 2% per month
            let dev_modifier = 1.0 / (1.0 + dev_f32 / 10.0);
            let threshold = base_chance * dev_modifier;

            candidates.push((neighbor_id, religion.clone(), threshold));
        }
    }

    // Now roll RNG for each candidate and collect conversions
    let mut conversions: Vec<(ProvinceId, String)> = Vec::new();
    for (province_id, religion, threshold) in candidates {
        let roll = state.random_f32();
        if roll < threshold {
            conversions.push((province_id, religion));
        }
    }

    // Apply conversions
    for (province_id, religion) in conversions {
        if let Some(prov) = state.provinces.get_mut(&province_id) {
            log::debug!(
                "Province {} converted to {} (Reformation spread)",
                province_id,
                religion
            );
            prov.religion = Some(religion);
        }
    }
}

fn expire_centers(state: &mut WorldState) {
    let current_date = state.date;

    // Find expired centers (100 years old)
    let expired: Vec<ProvinceId> = state
        .global
        .reformation
        .center_creation_dates
        .iter()
        .filter_map(|(&id, &created)| {
            if current_date.year >= created.year + 100 {
                Some(id)
            } else {
                None
            }
        })
        .collect();

    // Also check if center province changed religion
    let changed_religion: Vec<ProvinceId> = state
        .global
        .reformation
        .centers_of_reformation
        .iter()
        .filter_map(|(&id, religion)| {
            state
                .provinces
                .get(&id)
                .filter(|p| p.religion.as_deref() != Some(religion))
                .map(|_| id)
        })
        .collect();

    // Remove expired centers
    for id in expired.into_iter().chain(changed_religion) {
        state.global.reformation.centers_of_reformation.remove(&id);
        state.global.reformation.center_creation_dates.remove(&id);
        log::info!("Center of Reformation in province {} has been removed", id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Date;

    #[test]
    fn test_reformation_does_not_fire_before_1517() {
        let mut state = WorldState {
            date: Date::new(1500, 1, 1),
            ..Default::default()
        };

        run_reformation_tick(&mut state, None);

        assert!(!state.global.reformation.protestant_reformation_fired);
        assert!(state.global.reformation.centers_of_reformation.is_empty());
    }

    #[test]
    fn test_reformation_fires_in_1517() {
        let mut state = WorldState {
            date: Date::new(1517, 10, 1),
            ..Default::default()
        };

        // Add a Catholic German province
        let prov = crate::state::ProvinceState {
            religion: Some("catholic".to_string()),
            owner: Some("BRA".to_string()),
            base_tax: crate::fixed::Fixed::from_int(5),
            ..Default::default()
        };
        state.provinces.insert(50, prov);

        run_reformation_tick(&mut state, None);

        assert!(state.global.reformation.protestant_reformation_fired);
    }
}
