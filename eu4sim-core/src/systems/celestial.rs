//! Celestial Empire (Emperor of China) system.
//!
//! Handles yearly Mandate of Heaven calculations and Celestial Empire mechanics.
//!
//! ## Mandate Formula (Yearly)
//!
//! ```text
//! Base yearly change = 0
//! + (stability × 0.4)                    # Per positive stability
//! + (prosperous_states × 0.04)           # Per prosperous state
//! + (tributary_dev / 100 × 0.15)         # Per 100 tributary dev
//! - (devastation_dev × 0.12)             # Per 100 devastated dev (scaled)
//! - (loans / 5 × 0.60)                   # Per 5 loans
//! ```

use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::state::{CelestialReformId, WorldState};

/// Celestial Empire constants from defines.
pub mod defines {
    use crate::fixed::Fixed;

    /// Mandate cost to pass a reform.
    pub const REFORM_MANDATE_COST: Fixed = Fixed::from_int(70);

    /// Stability cost to pass a reform.
    pub const REFORM_STABILITY_COST: i32 = 1;

    /// Minimum mandate required to pass a reform.
    pub const REFORM_MIN_MANDATE: Fixed = Fixed::from_int(80);

    /// Default mandate for new emperors.
    pub const DEFAULT_MANDATE: Fixed = Fixed::from_int(80);

    /// Mandate threshold for positive/negative modifiers (50).
    pub const MODIFIER_THRESHOLD: Fixed = Fixed::from_int(50);

    /// Maximum mandate (cap).
    pub const MAX_MANDATE: Fixed = Fixed::from_int(100);

    /// Minimum mandate (floor).
    pub const MIN_MANDATE: Fixed = Fixed::ZERO;

    // Yearly mandate changes
    /// Mandate per point of positive stability.
    pub const MANDATE_PER_STABILITY: Fixed = Fixed::from_raw(4000); // 0.4

    /// Mandate per prosperous state.
    pub const MANDATE_PER_PROSPEROUS_STATE: Fixed = Fixed::from_raw(400); // 0.04

    /// Mandate per 100 tributary development.
    pub const MANDATE_PER_100_TRIBUTARY_DEV: Fixed = Fixed::from_raw(1500); // 0.15

    /// Mandate loss per 100 devastated development.
    pub const MANDATE_PER_100_DEVASTATION: Fixed = Fixed::from_raw(120_000); // 12.0

    /// Mandate loss per 5 loans.
    pub const MANDATE_PER_5_LOANS: Fixed = Fixed::from_raw(6000); // 0.60

    /// Mandate gained when successfully defending the title.
    pub const MANDATE_DEFENDING_SUCCESS: Fixed = Fixed::from_int(5);

    /// Mandate lost when refusing a tributary's call to arms.
    pub const MANDATE_REFUSED_TRIBUTARY_CTA: Fixed = Fixed::from_int(10);

    // Meritocracy
    /// Meritocracy gained from Strengthen Government action.
    pub const STRENGTHEN_GOVERNMENT_MERITOCRACY: Fixed = Fixed::from_int(10);

    /// Military power cost for Strengthen Government.
    pub const STRENGTHEN_GOVERNMENT_MIL_COST: Fixed = Fixed::from_int(100);

    /// Meritocracy cost to issue a decree.
    pub const DECREE_MERITOCRACY_COST: i32 = 20;

    /// Duration of a decree in years.
    pub const DECREE_DURATION_YEARS: i32 = 10;

    /// Yearly meritocracy gain per advisor level.
    /// Each advisor contributes skill_level * 0.5 meritocracy per year.
    pub const MERITOCRACY_PER_ADVISOR_LEVEL: Fixed = Fixed::from_raw(5000); // 0.5

    /// Maximum meritocracy value.
    pub const MAX_MERITOCRACY: Fixed = Fixed::from_int(100);

    /// Minimum meritocracy value.
    pub const MIN_MERITOCRACY: Fixed = Fixed::from_int(-100);

    // Meritocracy effects (linear interpolation from 0 to 100)
    /// Advisor cost modifier at 0 meritocracy: +25%
    pub const ADVISOR_COST_AT_ZERO: Fixed = Fixed::from_raw(2500); // 0.25 = +25%

    /// Advisor cost modifier at 100 meritocracy: -25%
    pub const ADVISOR_COST_AT_MAX: Fixed = Fixed::from_raw(-2500); // -0.25 = -25%

    /// Yearly corruption reduction at 100 meritocracy.
    pub const CORRUPTION_REDUCTION_AT_MAX: Fixed = Fixed::from_raw(2000); // 0.2

    /// Number of reforms required for vassalize tributaries capstone.
    pub const VASSALIZE_TRIBUTARIES_REQUIRED_REFORMS: usize = 8;
}

/// Well-known celestial reform IDs.
///
/// These IDs match the reforms in common/imperial_reforms/01_china.txt.
/// Unlike HRE reforms, celestial reforms can be passed in any order
/// (with some having prerequisites).
pub mod reforms {
    use crate::state::CelestialReformId;

    // Core reforms
    /// Seaban Decision - +1 diplomat, +5% trade efficiency
    pub const SEABAN: CelestialReformId = CelestialReformId(1);
    /// Establish Gaituguiliu - +0.5 meritocracy, +1 advisor pool (members)
    pub const GAITUGUILIU: CelestialReformId = CelestialReformId(2);
    /// Reform Land Tax - -5% autonomy, -25% state maintenance (members)
    pub const REFORM_LAND_TAX: CelestialReformId = CelestialReformId(3);
    /// Military Governors - +10% nobles loyalty, -10% core creation
    pub const MILITARY_GOVERNORS: CelestialReformId = CelestialReformId(4);
    /// Centralizing Top Government - +1 ADM/month, -5% estate influence (members)
    pub const CENTRALIZING_GOVERNMENT: CelestialReformId = CelestialReformId(5);

    // Capstone reform (requires 8 others)
    /// Vassalize Tributaries - +0.05 mandate, -33% liberty desire
    pub const VASSALIZE_TRIBUTARIES: CelestialReformId = CelestialReformId(6);

    // 1.35+ reforms
    /// Codify Single Whip Law - requires REFORM_LAND_TAX
    pub const CODIFY_SINGLE_WHIP_LAW: CelestialReformId = CelestialReformId(7);
    /// Establish Silver Standard - requires CODIFY_SINGLE_WHIP_LAW
    pub const ESTABLISH_SILVER_STANDARD: CelestialReformId = CelestialReformId(8);
    /// Kanhe Certificate - trade efficiency + merchants
    pub const KANHE_CERTIFICATE: CelestialReformId = CelestialReformId(9);
    /// New Keju Formats - governing capacity + reform progress
    pub const NEW_KEJU_FORMATS: CelestialReformId = CelestialReformId(10);
    /// Inclusive Monarchy - tolerance of heathens
    pub const INCLUSIVE_MONARCHY: CelestialReformId = CelestialReformId(11);
    /// Promote Bureaucratic Faction - mutually exclusive with PROMOTE_MILITARY
    pub const PROMOTE_BUREAUCRATIC: CelestialReformId = CelestialReformId(12);
    /// Promote Military Faction - mutually exclusive with PROMOTE_BUREAUCRATIC
    pub const PROMOTE_MILITARY: CelestialReformId = CelestialReformId(13);
    /// Unified Trade Market - requires SEABAN + KANHE_CERTIFICATE
    pub const UNIFIED_TRADE_MARKET: CelestialReformId = CelestialReformId(14);
    /// Reform the Military Branch - army professionalism + movement
    pub const REFORM_MILITARY_BRANCH: CelestialReformId = CelestialReformId(15);
    /// Modernize the Banners - cavalry cost + power
    pub const MODERNIZE_BANNERS: CelestialReformId = CelestialReformId(16);
    /// Study Foreign Ship Designs - ship cost + heavy ship power
    pub const STUDY_FOREIGN_SHIPS: CelestialReformId = CelestialReformId(17);
    /// Tributary Embassies - diplomatic upkeep + favor
    pub const TRIBUTARY_EMBASSIES: CelestialReformId = CelestialReformId(18);
    /// New World Discovery - colonial growth + colonist
    pub const NEW_WORLD_DISCOVERY: CelestialReformId = CelestialReformId(19);
    /// Reign in Estates - absolutism + admin efficiency
    pub const REIGN_IN_ESTATES: CelestialReformId = CelestialReformId(20);
    /// Reform Civil Registration - tax + dev cost
    pub const REFORM_CIVIL_REGISTRATION: CelestialReformId = CelestialReformId(21);
}

impl crate::state::CelestialEmpireState {
    /// Check if the vassalize tributaries reform has been passed.
    pub fn has_vassalize_tributaries(&self) -> bool {
        self.has_reform(reforms::VASSALIZE_TRIBUTARIES)
    }

    /// Check if a reform's prerequisites are met.
    pub fn can_pass_reform(&self, reform: CelestialReformId) -> bool {
        // Check prerequisites
        match reform {
            r if r == reforms::CODIFY_SINGLE_WHIP_LAW => self.has_reform(reforms::REFORM_LAND_TAX),
            r if r == reforms::ESTABLISH_SILVER_STANDARD => {
                self.has_reform(reforms::CODIFY_SINGLE_WHIP_LAW)
            }
            r if r == reforms::UNIFIED_TRADE_MARKET => {
                self.has_reform(reforms::SEABAN) && self.has_reform(reforms::KANHE_CERTIFICATE)
            }
            r if r == reforms::VASSALIZE_TRIBUTARIES => {
                self.reform_count() >= defines::VASSALIZE_TRIBUTARIES_REQUIRED_REFORMS
            }
            r if r == reforms::PROMOTE_BUREAUCRATIC => !self.has_reform(reforms::PROMOTE_MILITARY),
            r if r == reforms::PROMOTE_MILITARY => !self.has_reform(reforms::PROMOTE_BUREAUCRATIC),
            _ => true,
        }
    }
}

/// Run yearly mandate tick for the Celestial Empire.
///
/// Called on January 1st of each year. Updates mandate based on
/// stability, tributaries, devastation, and loans.
pub fn run_celestial_tick(state: &mut WorldState) {
    // Skip if dismantled or no emperor
    if state.global.celestial_empire.dismantled {
        return;
    }
    let emperor_tag = match &state.global.celestial_empire.emperor {
        Some(tag) => tag.clone(),
        None => return,
    };

    // Get emperor's country state
    let emperor = match state.countries.get(&emperor_tag) {
        Some(c) => c,
        None => return,
    };

    let mut mandate_delta = Fixed::ZERO;

    // Stability bonus (+0.4 per positive stability)
    let stability = emperor.stability.get();
    if stability > 0 {
        mandate_delta += defines::MANDATE_PER_STABILITY.mul(Fixed::from_int(stability as i64));
    }

    // Tributary development bonus (+0.15 per 100 dev)
    let tributary_dev = calculate_tributary_development(state, &emperor_tag);
    let dev_bonus = tributary_dev
        .div(Fixed::from_int(100))
        .mul(defines::MANDATE_PER_100_TRIBUTARY_DEV);
    mandate_delta += dev_bonus;

    // TODO: Prosperous states bonus (requires prosperity tracking)

    // Devastation penalty (-12.0 per 100 devastated dev)
    // Calculate dev-weighted devastation: sum(dev * devastation%) / 100
    let devastated_dev = calculate_devastated_development(state, &emperor_tag);
    let devastation_penalty = devastated_dev
        .div(Fixed::from_int(100))
        .mul(defines::MANDATE_PER_100_DEVASTATION);
    mandate_delta -= devastation_penalty;

    // Loan penalty (-0.6 per 5 loans)
    let loan_penalty =
        Fixed::from_int((emperor.loans / 5) as i64).mul(defines::MANDATE_PER_5_LOANS);
    mandate_delta -= loan_penalty;

    // Apply mandate change with clamping
    let current = state.global.celestial_empire.mandate;
    let new_mandate = (current + mandate_delta)
        .max(defines::MIN_MANDATE)
        .min(defines::MAX_MANDATE);
    state.global.celestial_empire.mandate = new_mandate;

    log::trace!(
        "Celestial Empire tick: emperor={}, mandate={:.2} -> {:.2} (delta={:.2}, trib_dev={:.0}, dev_dev={:.0}, loans={})",
        emperor_tag,
        current.to_f32(),
        new_mandate.to_f32(),
        mandate_delta.to_f32(),
        tributary_dev.to_f32(),
        devastated_dev.to_f32(),
        emperor.loans
    );
}

/// Calculate total development of all tributaries of the emperor.
fn calculate_tributary_development(state: &WorldState, emperor_tag: &str) -> Fixed {
    let mut total_dev = Mod32::ZERO;

    for (subject_tag, relationship) in &state.diplomacy.subjects {
        // Check if this is a tributary of the emperor
        if relationship.overlord != emperor_tag {
            continue;
        }

        // Check if it's a tributary type
        if let Some(_subject_type) = state.subject_types.get(relationship.subject_type) {
            if !state.subject_types.is_tributary(relationship.subject_type) {
                continue;
            }
        }

        // Sum development of all provinces owned by this tributary
        for province in state.provinces.values() {
            if province.owner.as_deref() == Some(subject_tag) {
                total_dev += province.base_tax + province.base_production + province.base_manpower;
            }
        }
    }

    total_dev.to_fixed()
}

/// Calculate development-weighted devastation for the emperor's provinces.
///
/// Returns the sum of (province_dev * province_devastation%) for all emperor-owned provinces.
/// This represents the "devastated development" used in mandate calculation.
fn calculate_devastated_development(state: &WorldState, emperor_tag: &str) -> Fixed {
    let mut devastated_dev = Mod32::ZERO;

    for province in state.provinces.values() {
        if province.owner.as_deref() == Some(emperor_tag) {
            let dev = province.base_tax + province.base_production + province.base_manpower;
            // Devastation is 0-100, divide by 100 to get percentage
            let devastation_pct = province.devastation / Mod32::from_int(100);
            devastated_dev += dev * devastation_pct;
        }
    }

    devastated_dev.to_fixed()
}

/// Run yearly meritocracy tick for the Celestial Empire.
///
/// Called on January 1st of each year (same as mandate tick).
/// Meritocracy increases based on advisor skill levels.
pub fn run_meritocracy_tick(state: &mut WorldState) {
    // Skip if dismantled or no emperor
    if state.global.celestial_empire.dismantled {
        return;
    }
    let emperor_tag = match &state.global.celestial_empire.emperor {
        Some(tag) => tag.clone(),
        None => return,
    };

    // Get emperor's country state
    let emperor = match state.countries.get(&emperor_tag) {
        Some(c) => c,
        None => return,
    };

    // Calculate meritocracy gain from advisors
    // Each advisor contributes skill_level * 0.5 per year
    let mut advisor_bonus = Fixed::ZERO;
    for advisor in &emperor.advisors {
        advisor_bonus +=
            Fixed::from_int(advisor.skill as i64).mul(defines::MERITOCRACY_PER_ADVISOR_LEVEL);
    }

    // Apply meritocracy change
    let emperor = state.countries.get_mut(&emperor_tag).unwrap();
    let current = emperor.meritocracy.get();
    let new_meritocracy = (current + advisor_bonus)
        .max(defines::MIN_MERITOCRACY)
        .min(defines::MAX_MERITOCRACY);
    emperor.meritocracy.set(new_meritocracy);

    log::trace!(
        "Meritocracy tick: emperor={}, meritocracy={:.2} -> {:.2} (advisor_bonus={:.2})",
        emperor_tag,
        current.to_f32(),
        new_meritocracy.to_f32(),
        advisor_bonus.to_f32()
    );
}

/// Calculate the advisor cost modifier based on meritocracy.
///
/// Returns a modifier from +0.25 (at 0 meritocracy) to -0.25 (at 100 meritocracy).
/// Linearly interpolated based on meritocracy value.
pub fn calculate_advisor_cost_modifier(meritocracy: Fixed) -> Fixed {
    // Linear interpolation: at 0 = +25%, at 100 = -25%
    // modifier = 0.25 - (meritocracy / 100) * 0.50
    let ratio = meritocracy.div(Fixed::from_int(100));
    let range = defines::ADVISOR_COST_AT_ZERO - defines::ADVISOR_COST_AT_MAX; // 0.50
    defines::ADVISOR_COST_AT_ZERO - ratio.mul(range)
}

/// Calculate the yearly corruption reduction based on meritocracy.
///
/// Returns a value from 0 (at 0 meritocracy) to 0.2 (at 100 meritocracy).
pub fn calculate_corruption_reduction(meritocracy: Fixed) -> Fixed {
    // Linear interpolation: at 0 = 0, at 100 = 0.2
    if meritocracy <= Fixed::ZERO {
        return Fixed::ZERO;
    }
    let ratio = meritocracy.div(Fixed::from_int(100));
    ratio.mul(defines::CORRUPTION_REDUCTION_AT_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Advisor, AdvisorType, CountryState, Date, ProvinceState};

    fn setup_celestial_test() -> WorldState {
        let mut state = WorldState {
            date: Date::new(1445, 1, 1), // January 1st for yearly tick
            ..Default::default()
        };

        // Set up Ming as Emperor of China
        state.global.celestial_empire.emperor = Some("MNG".to_string());
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        // Add Ming with positive stability
        let mut ming = CountryState::default();
        ming.stability.set(3);
        state.countries.insert("MNG".to_string(), ming);

        state
    }

    #[test]
    fn test_mandate_no_change_when_dismantled() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.dismantled = true;
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        run_celestial_tick(&mut state);

        assert_eq!(state.global.celestial_empire.mandate, Fixed::from_int(50));
    }

    #[test]
    fn test_mandate_no_change_when_no_emperor() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.emperor = None;
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        run_celestial_tick(&mut state);

        assert_eq!(state.global.celestial_empire.mandate, Fixed::from_int(50));
    }

    #[test]
    fn test_mandate_stability_bonus() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        // Ming has stability 3, should gain 3 * 0.4 = 1.2 mandate
        run_celestial_tick(&mut state);

        // Should be around 51.2 (50 + 1.2)
        let new_mandate = state.global.celestial_empire.mandate.to_f32();
        assert!(
            new_mandate > 51.0 && new_mandate < 52.0,
            "mandate={}",
            new_mandate
        );
    }

    #[test]
    fn test_mandate_capped_at_100() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(99);

        run_celestial_tick(&mut state);

        assert!(state.global.celestial_empire.mandate <= Fixed::from_int(100));
    }

    #[test]
    fn test_is_emperor() {
        let state = setup_celestial_test();

        assert!(state.global.celestial_empire.is_emperor(&"MNG".to_string()));
        assert!(!state.global.celestial_empire.is_emperor(&"QNG".to_string()));
    }

    #[test]
    fn test_reform_prerequisites() {
        let mut ce = crate::state::CelestialEmpireState::default();

        // Can't pass codify_single_whip without reform_land_tax
        assert!(!ce.can_pass_reform(reforms::CODIFY_SINGLE_WHIP_LAW));

        // Can pass after prerequisite
        ce.reforms_passed.insert(reforms::REFORM_LAND_TAX);
        assert!(ce.can_pass_reform(reforms::CODIFY_SINGLE_WHIP_LAW));

        // Mutually exclusive reforms
        assert!(ce.can_pass_reform(reforms::PROMOTE_BUREAUCRATIC));
        assert!(ce.can_pass_reform(reforms::PROMOTE_MILITARY));

        ce.reforms_passed.insert(reforms::PROMOTE_BUREAUCRATIC);
        assert!(!ce.can_pass_reform(reforms::PROMOTE_MILITARY));
    }

    #[test]
    fn test_vassalize_tributaries_requires_8_reforms() {
        let mut ce = crate::state::CelestialEmpireState::default();

        // Need 8 reforms first
        assert!(!ce.can_pass_reform(reforms::VASSALIZE_TRIBUTARIES));

        // Add 8 reforms
        ce.reforms_passed.insert(reforms::SEABAN);
        ce.reforms_passed.insert(reforms::GAITUGUILIU);
        ce.reforms_passed.insert(reforms::REFORM_LAND_TAX);
        ce.reforms_passed.insert(reforms::MILITARY_GOVERNORS);
        ce.reforms_passed.insert(reforms::CENTRALIZING_GOVERNMENT);
        ce.reforms_passed.insert(reforms::KANHE_CERTIFICATE);
        ce.reforms_passed.insert(reforms::NEW_KEJU_FORMATS);
        ce.reforms_passed.insert(reforms::INCLUSIVE_MONARCHY);

        assert!(ce.can_pass_reform(reforms::VASSALIZE_TRIBUTARIES));
    }

    #[test]
    fn test_mandate_loan_penalty() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        // Set stability to 0 so we only see loan effects
        state.countries.get_mut("MNG").unwrap().stability.set(0);

        // Add 10 loans (should lose 0.6 * 2 = 1.2 mandate)
        state.countries.get_mut("MNG").unwrap().loans = 10;

        run_celestial_tick(&mut state);

        // Should be around 48.8 (50 - 1.2)
        let new_mandate = state.global.celestial_empire.mandate.to_f32();
        assert!(
            new_mandate > 48.0 && new_mandate < 50.0,
            "mandate={}, expected ~48.8",
            new_mandate
        );
    }

    #[test]
    fn test_mandate_devastation_penalty() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        // Set stability to 0 so we only see devastation effects
        state.countries.get_mut("MNG").unwrap().stability.set(0);

        // Add a province with 100 dev and 50% devastation
        // Devastated dev = 100 * 0.5 = 50
        // Penalty = (50 / 100) * 12.0 = 6.0 mandate loss
        state.provinces.insert(
            1,
            ProvinceState {
                owner: Some("MNG".to_string()),
                base_tax: Mod32::from_int(33),
                base_production: Mod32::from_int(34),
                base_manpower: Mod32::from_int(33),
                devastation: Mod32::from_int(50), // 50% devastation
                ..Default::default()
            },
        );

        run_celestial_tick(&mut state);

        // Should be around 44.0 (50 - 6.0)
        let new_mandate = state.global.celestial_empire.mandate.to_f32();
        assert!(
            new_mandate > 43.0 && new_mandate < 45.0,
            "mandate={}, expected ~44.0",
            new_mandate
        );
    }

    #[test]
    fn test_mandate_combined_effects() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        // Stability 3: +1.2
        state.countries.get_mut("MNG").unwrap().stability.set(3);

        // 5 loans: -0.6
        state.countries.get_mut("MNG").unwrap().loans = 5;

        // 30 dev province with 100% devastation: -3.6 (30/100 * 12)
        state.provinces.insert(
            1,
            ProvinceState {
                owner: Some("MNG".to_string()),
                base_tax: Mod32::from_int(10),
                base_production: Mod32::from_int(10),
                base_manpower: Mod32::from_int(10),
                devastation: Mod32::from_int(100), // 100% devastation
                ..Default::default()
            },
        );

        // Net: 50 + 1.2 - 0.6 - 3.6 = 47.0
        run_celestial_tick(&mut state);

        let new_mandate = state.global.celestial_empire.mandate.to_f32();
        assert!(
            new_mandate > 46.0 && new_mandate < 48.0,
            "mandate={}, expected ~47.0",
            new_mandate
        );
    }

    #[test]
    fn test_mandate_floors_at_zero() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(5);

        // Set stability to 0
        state.countries.get_mut("MNG").unwrap().stability.set(0);

        // 50 loans: -6.0 mandate (should hit floor at 0)
        state.countries.get_mut("MNG").unwrap().loans = 50;

        run_celestial_tick(&mut state);

        assert!(
            state.global.celestial_empire.mandate >= Fixed::ZERO,
            "mandate should not go below 0"
        );
    }

    // ===== MERITOCRACY TESTS =====

    #[test]
    fn test_meritocracy_from_advisors() {
        let mut state = setup_celestial_test();

        // Add advisors with skill levels 3, 3, 3 (total 9 levels)
        // Expected gain: 9 * 0.5 = 4.5 per year
        let ming = state.countries.get_mut("MNG").unwrap();
        ming.meritocracy.set(Fixed::from_int(50));
        ming.advisors = vec![
            Advisor {
                name: "Test ADM".to_string(),
                skill: 3,
                advisor_type: AdvisorType::Administrative,
                monthly_cost: Fixed::from_int(5),
            },
            Advisor {
                name: "Test DIP".to_string(),
                skill: 3,
                advisor_type: AdvisorType::Diplomatic,
                monthly_cost: Fixed::from_int(5),
            },
            Advisor {
                name: "Test MIL".to_string(),
                skill: 3,
                advisor_type: AdvisorType::Military,
                monthly_cost: Fixed::from_int(5),
            },
        ];

        run_meritocracy_tick(&mut state);

        let new_meritocracy = state
            .countries
            .get("MNG")
            .unwrap()
            .meritocracy
            .get()
            .to_f32();
        // Expected: 50 + 4.5 = 54.5
        assert!(
            new_meritocracy > 54.0 && new_meritocracy < 55.0,
            "meritocracy={}, expected ~54.5",
            new_meritocracy
        );
    }

    #[test]
    fn test_meritocracy_capped_at_100() {
        let mut state = setup_celestial_test();

        // Set meritocracy near cap and add high-skill advisors
        let ming = state.countries.get_mut("MNG").unwrap();
        ming.meritocracy.set(Fixed::from_int(98));
        ming.advisors = vec![Advisor {
            name: "Test".to_string(),
            skill: 5,
            advisor_type: AdvisorType::Administrative,
            monthly_cost: Fixed::from_int(10),
        }];

        run_meritocracy_tick(&mut state);

        let new_meritocracy = state.countries.get("MNG").unwrap().meritocracy.get();
        assert!(
            new_meritocracy <= Fixed::from_int(100),
            "meritocracy should not exceed 100"
        );
    }

    #[test]
    fn test_meritocracy_no_change_without_advisors() {
        let mut state = setup_celestial_test();

        // No advisors, meritocracy should not change
        let ming = state.countries.get_mut("MNG").unwrap();
        ming.meritocracy.set(Fixed::from_int(50));
        ming.advisors.clear();

        run_meritocracy_tick(&mut state);

        let new_meritocracy = state.countries.get("MNG").unwrap().meritocracy.get();
        assert_eq!(new_meritocracy, Fixed::from_int(50));
    }

    #[test]
    fn test_advisor_cost_modifier_at_zero() {
        // At 0 meritocracy: +25% advisor cost
        let modifier = calculate_advisor_cost_modifier(Fixed::ZERO);
        let modifier_pct = modifier.to_f32();
        assert!(
            (modifier_pct - 0.25).abs() < 0.01,
            "modifier={}, expected 0.25",
            modifier_pct
        );
    }

    #[test]
    fn test_advisor_cost_modifier_at_100() {
        // At 100 meritocracy: -25% advisor cost
        let modifier = calculate_advisor_cost_modifier(Fixed::from_int(100));
        let modifier_pct = modifier.to_f32();
        assert!(
            (modifier_pct - (-0.25)).abs() < 0.01,
            "modifier={}, expected -0.25",
            modifier_pct
        );
    }

    #[test]
    fn test_advisor_cost_modifier_at_50() {
        // At 50 meritocracy: 0% (midpoint)
        let modifier = calculate_advisor_cost_modifier(Fixed::from_int(50));
        let modifier_pct = modifier.to_f32();
        assert!(
            modifier_pct.abs() < 0.01,
            "modifier={}, expected ~0.0",
            modifier_pct
        );
    }

    #[test]
    fn test_corruption_reduction_at_zero() {
        let reduction = calculate_corruption_reduction(Fixed::ZERO);
        assert_eq!(reduction, Fixed::ZERO);
    }

    #[test]
    fn test_corruption_reduction_at_100() {
        let reduction = calculate_corruption_reduction(Fixed::from_int(100));
        let reduction_val = reduction.to_f32();
        assert!(
            (reduction_val - 0.2).abs() < 0.01,
            "reduction={}, expected 0.2",
            reduction_val
        );
    }

    #[test]
    fn test_corruption_reduction_at_50() {
        let reduction = calculate_corruption_reduction(Fixed::from_int(50));
        let reduction_val = reduction.to_f32();
        // At 50: 0.5 * 0.2 = 0.1
        assert!(
            (reduction_val - 0.1).abs() < 0.01,
            "reduction={}, expected 0.1",
            reduction_val
        );
    }

    // ===== INTEGRATION TESTS =====

    #[test]
    fn test_unified_trade_market_prerequisites() {
        let mut ce = crate::state::CelestialEmpireState::default();

        // Can't pass unified trade market without prerequisites
        assert!(!ce.can_pass_reform(reforms::UNIFIED_TRADE_MARKET));

        // Still can't with only seaban
        ce.reforms_passed.insert(reforms::SEABAN);
        assert!(!ce.can_pass_reform(reforms::UNIFIED_TRADE_MARKET));

        // Can pass with both seaban and kanhe_certificate
        ce.reforms_passed.insert(reforms::KANHE_CERTIFICATE);
        assert!(ce.can_pass_reform(reforms::UNIFIED_TRADE_MARKET));
    }

    #[test]
    fn test_silver_standard_chain() {
        let mut ce = crate::state::CelestialEmpireState::default();

        // Can't pass silver standard without prerequisite chain
        assert!(!ce.can_pass_reform(reforms::ESTABLISH_SILVER_STANDARD));

        // Need reform_land_tax first
        ce.reforms_passed.insert(reforms::REFORM_LAND_TAX);
        assert!(!ce.can_pass_reform(reforms::ESTABLISH_SILVER_STANDARD));

        // Need codify_single_whip_law (which requires reform_land_tax)
        ce.reforms_passed.insert(reforms::CODIFY_SINGLE_WHIP_LAW);
        assert!(ce.can_pass_reform(reforms::ESTABLISH_SILVER_STANDARD));
    }

    #[test]
    fn test_tributary_development_bonus() {
        let mut state = setup_celestial_test();
        state.global.celestial_empire.mandate = Fixed::from_int(50);

        // Set stability to 0 so we only see tributary effects
        state.countries.get_mut("MNG").unwrap().stability.set(0);

        // Add a tributary with 200 development
        // Need to set up subject types registry first
        let mut tributary_def = crate::subjects::SubjectTypeDef {
            name: "tributary_state".to_string(),
            joins_overlords_wars: false, // This makes it a tributary
            ..Default::default()
        };
        tributary_def.id = crate::subjects::SubjectTypeId(0);
        state.subject_types.add(tributary_def);

        // Add tributary country
        state
            .countries
            .insert("KOR".to_string(), CountryState::default());

        // Add subject relationship
        state.diplomacy.subjects.insert(
            "KOR".to_string(),
            crate::state::SubjectRelationship {
                overlord: "MNG".to_string(),
                subject: "KOR".to_string(),
                subject_type: crate::subjects::SubjectTypeId(0),
                start_date: state.date,
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            },
        );

        // Add 200 dev to the tributary
        state.provinces.insert(
            100,
            ProvinceState {
                owner: Some("KOR".to_string()),
                base_tax: Mod32::from_int(66),
                base_production: Mod32::from_int(67),
                base_manpower: Mod32::from_int(67),
                ..Default::default()
            },
        );

        run_celestial_tick(&mut state);

        // 200 dev / 100 * 0.15 = 0.30 mandate gain
        let new_mandate = state.global.celestial_empire.mandate.to_f32();
        assert!(
            new_mandate > 50.0 && new_mandate < 51.0,
            "mandate={}, expected ~50.3",
            new_mandate
        );
    }

    #[test]
    fn test_take_mandate_resets_reforms() {
        let mut ce = crate::state::CelestialEmpireState {
            emperor: Some("MNG".to_string()),
            mandate: Fixed::from_int(30),
            ..Default::default()
        };

        // Pass some reforms
        ce.reforms_passed.insert(reforms::SEABAN);
        ce.reforms_passed.insert(reforms::GAITUGUILIU);
        assert_eq!(ce.reform_count(), 2);

        // Take mandate resets everything
        ce.emperor = Some("QNG".to_string());
        ce.mandate = defines::DEFAULT_MANDATE;
        ce.reforms_passed.clear();

        assert_eq!(ce.emperor, Some("QNG".to_string()));
        assert_eq!(ce.mandate.to_f32(), 80.0);
        assert_eq!(ce.reform_count(), 0);
    }

    #[test]
    fn test_meritocracy_bounds() {
        let mut state = setup_celestial_test();

        // Set meritocracy to max
        let ming = state.countries.get_mut("MNG").unwrap();
        ming.meritocracy.set(Fixed::from_int(100));

        // Add level 5 advisor
        ming.advisors = vec![Advisor {
            name: "Test".to_string(),
            skill: 5,
            advisor_type: AdvisorType::Administrative,
            monthly_cost: Fixed::from_int(10),
        }];

        run_meritocracy_tick(&mut state);

        // Should cap at 100
        let meritocracy = state.countries.get("MNG").unwrap().meritocracy.get();
        assert!(
            meritocracy <= Fixed::from_int(100),
            "meritocracy should cap at 100"
        );
    }
}
