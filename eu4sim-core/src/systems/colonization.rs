use crate::state::WorldState;

/// Monthly colonization tick.
///
/// Progresses all active colonies.
pub fn run_colonization_tick(state: &mut WorldState) {
    let mut completed = Vec::new();

    let mut updates = Vec::new();

    // Progress each colony
    for colony in state.colonies.values() {
        let mut new_colony = colony.clone();
        new_colony.settlers += 83;

        if new_colony.settlers >= 1000 {
            completed.push(new_colony.province);
        } else {
            updates.push(new_colony);
        }
    }

    // Apply updates
    for colony in updates {
        state.colonies.insert(colony.province, colony);
    }

    // Convert completed colonies to owned provinces
    for province_id in completed {
        if let Some(colony) = state.colonies.remove(&province_id) {
            if let Some(province) = state.provinces.get_mut(&province_id) {
                // Wastelands cannot be colonized - remove invalid colony
                if province.is_wasteland {
                    log::warn!(
                        "Removed invalid colony in wasteland province {}",
                        province_id
                    );
                    continue;
                }
                province.owner = Some(colony.owner.clone());
                province.controller = Some(colony.owner.clone());
                log::info!(
                    "Colony in province {} completed! {} is the new owner.",
                    province_id,
                    colony.owner
                );
            }
        }
    }
}
