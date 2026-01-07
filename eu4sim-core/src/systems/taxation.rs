#[cfg(test)]
use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::simd::tax32::{calculate_taxes_batch32, TaxInput32, TaxOutput32};
use crate::state::{TagId, WorldState};
use tracing::instrument;

/// Chunk size for SIMD batch processing.
/// Chosen to balance SIMD efficiency with trace-level visibility.
/// 256 provinces = ~32 AVX2 iterations, good cache locality.
const CHUNK_SIZE: usize = 256;

/// Runs monthly taxation calculations.
///
/// Formula: (Base Tax) * (1 + National Mod + Local Mod) * (1 - Autonomy) / 12
///
/// Uses SIMD-accelerated batch processing with a single flat batch for all provinces,
/// then aggregates results by owner. This minimizes per-country overhead while
/// maintaining efficient SIMD utilization.
///
/// # Performance
///
/// This function uses tag interning to avoid string cloning:
/// - Phase 1: Build local TagId interner from owner strings (O(1) per province after first)
/// - Phase 2: SIMD batch computation (unchanged)
/// - Phase 3: Aggregate using TagId keys (u16 hash is trivial)
/// - Phase 4: Resolve TagId back to strings only for country lookup
#[instrument(skip_all, name = "taxation")]
pub fn run_taxation_tick(state: &mut WorldState) {
    // PHASE 1: Collect all province inputs into a single flat batch
    // Uses owned_provinces cache (Vec) for O(1) iteration without tree traversal.
    // Cache is lazily rebuilt on first access if invalidated.
    // PHASE 1: Collect all province inputs into a single flat batch
    // Uses SoA cache for SIMD-friendly linear iteration.
    // Cache is lazily rebuilt on first access if invalidated.
    let province_data: Vec<(TagId, TaxInput32, Mod32)> = {
        let _span = tracing::info_span!("taxation_prepare").entered();

        // 1. Ensure cache is valid (rebuilds and sorts by owner if dirty)
        state.ensure_owned_provinces_valid();

        // 2. Split borrows to access fields simultaneously without cloning
        let cache = &state.owned_provinces_cache;
        let count = cache.ids.len();

        let mut data = Vec::with_capacity(count);

        // Hoist references
        let prov_tax_mod = &state.modifiers.province_tax_modifier;
        let country_tax_mod = &state.modifiers.country_tax_modifier;
        let prov_autonomy = &state.modifiers.province_autonomy;

        // Optimization: Cache is sorted by owner, so we can hoist country modifier lookups
        let mut current_owner_id = TagId(u16::MAX); // Sentinel
        let mut current_national_mod = Mod32::ZERO;

        for i in 0..count {
            let id = cache.ids[i];
            let owner_id = cache.owners[i];
            let base_tax = cache.base_tax[i];
            let auto_floor = cache.autonomy_floor[i];

            // Update national modifier only when owner changes (cache is sorted!)
            if owner_id != current_owner_id {
                let tag_str = state.tags.resolve(owner_id);
                current_national_mod = country_tax_mod.get(tag_str).copied().unwrap_or(Mod32::ZERO);
                current_owner_id = owner_id;
            }

            // O(1) array lookups for province modifiers
            let local_mod = prov_tax_mod.get(id);
            let base_autonomy = prov_autonomy.get(id);

            // Effective autonomy using pre-calculated floor from cache
            let effective_autonomy = base_autonomy.max(auto_floor);

            let input = TaxInput32::new(
                base_tax,
                current_national_mod,
                local_mod,
                effective_autonomy,
            );

            data.push((owner_id, input, base_tax));
        }

        data
    };

    let province_count = province_data.len();
    if province_count == 0 {
        return;
    }

    // PHASE 2: Sequential SIMD batch processing
    // Note: Rayon parallelism was tested but overhead dominated for ~10 chunks (2450/256).
    // For modded games with 10k+ provinces, consider par_chunks.
    let outputs: Vec<TaxOutput32> = {
        let _span = tracing::info_span!("taxation_compute", provinces = province_count).entered();

        // Extract just the inputs for SIMD
        let inputs: Vec<TaxInput32> = province_data.iter().map(|(_, input, _)| *input).collect();
        let mut outputs = vec![TaxOutput32::default(); province_count];

        // Process in chunks with trace-level spans for optional visibility
        for (chunk_idx, (in_chunk, out_chunk)) in inputs
            .chunks(CHUNK_SIZE)
            .zip(outputs.chunks_mut(CHUNK_SIZE))
            .enumerate()
        {
            let _chunk_span =
                tracing::trace_span!("tax_chunk", n = chunk_idx, size = in_chunk.len()).entered();
            calculate_taxes_batch32(in_chunk, out_chunk);
        }

        outputs
    };

    // PHASE 3: Aggregate results by owner
    // Grouping by TagId allows using an integer key (fast aggregation)
    let mut country_totals: std::collections::HashMap<TagId, Mod32> =
        std::collections::HashMap::with_capacity(state.countries.len());

    {
        let _span = tracing::info_span!("taxation_aggregate").entered();
        for ((owner_id, _, _), output) in province_data.iter().zip(outputs.iter()) {
            *country_totals.entry(*owner_id).or_insert(Mod32::ZERO) +=
                Mod32::from_raw(output.monthly_income);
        }
    }

    // PHASE 4: Update World State (Treasury)
    // Resolve TagId back to string just once per country
    {
        let _span = tracing::info_span!("taxation_apply").entered();
        let base_monthly_income = Mod32::ONE;

        for (owner_id, total_tax) in country_totals {
            let tag_str = state.tags.resolve(owner_id);
            if let Some(country) = state.countries.get_mut(tag_str) {
                // Add base income (every country with provinces gets 1 ducat/month)
                let total_income = total_tax + base_monthly_income;

                // Tax income is added to treasury (converted to Fixed)
                country.treasury += total_income.to_fixed();
                country.income.taxation += total_income.to_fixed();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ProvinceState;
    use crate::testing::WorldStateBuilder;
    use proptest::prelude::*;

    #[test]
    fn test_taxation_basic() {
        // Setup: 1 province, base tax 12.0
        // Provincial Monthly: 12.0 / 12 = 1.0
        // Base Income: 1.0 (all countries get 1 ducat/month)
        // Expected Total: 2.0
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_tax: Mod32::from_f32(12.0),
            owner: Some("SWE".to_string()),
            cores,
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(1, province)
            .build();

        // Reset treasury to 0 for clear assertion
        state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

        run_taxation_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(2.0)); // 1.0 provincial + 1.0 base
    }

    #[test]
    fn test_taxation_modifiers() {
        // Setup: Base 12, +50% National, -50% Autonomy
        // Yearly: 12 * 1.5 * 0.5 = 9.0
        // Provincial Monthly: 0.75
        // Base Income: 1.0 (all countries get 1 ducat/month)
        // Total: 1.75
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_tax: Mod32::from_f32(12.0),
            owner: Some("SWE".to_string()),
            cores,
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(1, province)
            .build();

        // Reset treasury
        state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

        // Modifiers
        state
            .modifiers
            .country_tax_modifier
            .insert("SWE".to_string(), Mod32::from_f32(0.5));
        state
            .modifiers
            .province_autonomy
            .insert(1, Mod32::from_f32(0.5));

        run_taxation_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(1.75)); // 0.75 provincial + 1.0 base
    }

    proptest! {
        #[test]
        fn prop_taxation_never_negative(
            autonomy in -2.0..2.0f32,
            efficiency_mod in -2.0..2.0f32
        ) {
            let province = ProvinceState {
                base_tax: Mod32::from_f32(12.0), // Base 12 = 1.0 monthly base
                owner: Some("SWE".to_string()),
                ..Default::default()
            };

            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .with_province_state(1, province)
                .build();

            state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

            state.modifiers.province_autonomy.insert(1, Mod32::from_f32(autonomy));
            state.modifiers.country_tax_modifier.insert("SWE".to_string(), Mod32::from_f32(efficiency_mod));

            run_taxation_tick(&mut state);

            let swe = state.countries.get("SWE").unwrap();
            // Income should never be negative
            prop_assert!(swe.treasury >= Fixed::ZERO);
        }
    }
}
