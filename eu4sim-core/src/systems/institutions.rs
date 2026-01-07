use crate::fixed_generic::Mod32;
use crate::state::{InstitutionId, Tag, WorldState};
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use tracing::instrument;

/// Executes the monthly institution spread tick.
///
/// Simplified institution spread:
/// 1. Institutions grow in high-dev provinces adjacent to embraced provinces.
/// 2. If a province has 100% presence, it becomes "present" in that province.
#[instrument(skip_all, name = "institutions")]
pub fn tick_institution_spread(state: &mut WorldState) {
    // For the mid-term goal, we'll implement a very simple spread mechanism.
    // In a real EU4 simulation, this would depend on many factors (trade, adjacency, etc.)

    let mut presence_updates: Vec<(u32, InstitutionId, f32)> = Vec::new();

    // Collect all provinces where institutions have already reached 100% and the owner has embraced it.
    let mut embraced_by_tag: HashSet<(Tag, InstitutionId)> = HashSet::new();
    for (tag, country) in &state.countries {
        for inst in &country.embraced_institutions {
            embraced_by_tag.insert((tag.clone(), inst.clone()));
        }
    }

    for (&province_id, province) in &state.provinces {
        let total_dev =
            (province.base_tax + province.base_production + province.base_manpower).to_f32();

        // Spread chance: 1% per 100 dev per month (very slow)
        // Simplified: if neighbor has embraced, grow by dev/1000 per month.
        let spread_rate = (total_dev / 1000.0).max(0.1); // minimum 0.1% per month

        // We'll hardcode some starting institutions if they don't exist yet
        // In a real sim, these would fire via events.
        if state.date.year >= 1450 && !province.institution_presence.contains_key("renaissance") {
            // Renaissance origin simulation (e.g. province 1 is Florence in a real map)
            if province_id == 112 {
                // Randomly picked "origin" for now
                presence_updates.push((province_id, "renaissance".to_string(), spread_rate * 10.0));
            }
        }

        // Logic for spreading from embraced neighbors would go here if we had an adjacency graph in the state.
        // For now, let's just make it grow linearly in high-dev provinces to simulate "innovation".
        for (inst, presence) in &province.institution_presence {
            if *presence < 100.0 {
                presence_updates.push((province_id, inst.clone(), spread_rate));
            }
        }
    }

    for (pid, inst, delta) in presence_updates {
        if let Some(province) = state.provinces.get_mut(&pid) {
            let entry = province.institution_presence.entry(inst).or_insert(0.0);
            *entry = (*entry + delta).min(100.0);
        }
    }
}

/// Executes the EmbraceInstitution command.
///
/// Costs gold based on development of non-present provinces.
pub fn embrace_institution(
    state: &mut WorldState,
    country: Tag,
    institution: InstitutionId,
) -> Result<()> {
    let country_state = state
        .countries
        .get_mut(&country)
        .ok_or_else(|| anyhow!("Country {} not found", country))?;

    if country_state.embraced_institutions.contains(&institution) {
        return Err(anyhow!("Institution {} already embraced", institution));
    }

    // Check if at least 10% of development has the institution present
    let mut total_dev = Mod32::ZERO;
    let mut present_dev = Mod32::ZERO;

    for province in state.provinces.values() {
        if province.owner.as_ref() == Some(&country) {
            let dev = province.base_tax + province.base_production + province.base_manpower;
            total_dev += dev;
            if province
                .institution_presence
                .get(&institution)
                .copied()
                .unwrap_or(0.0)
                >= 100.0
            {
                present_dev += dev;
            }
        }
    }

    if total_dev > Mod32::ZERO && (present_dev / total_dev) < Mod32::from_raw(1000) {
        // 0.10 in SCALE=10000
        return Err(anyhow!(
            "Less than 10% of development has present institution {}",
            institution
        ));
    }

    // Cost: 2.0 gold per non-present development point
    let non_present_dev = total_dev - present_dev;
    let cost = (non_present_dev * Mod32::from_int(2)).to_fixed();

    if country_state.treasury < cost {
        return Err(anyhow!(
            "Not enough gold to embrace institution {} (cost: {})",
            institution,
            cost
        ));
    }

    country_state.treasury -= cost;
    country_state.embraced_institutions.insert(institution);

    Ok(())
}
