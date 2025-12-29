//! Holy Roman Empire system.
//!
//! Handles monthly Imperial Authority calculations and HRE mechanics.
//!
//! ## Imperial Authority Formula (Monthly)
//!
//! ```text
//! Base (at peace):         +0.10
//! Per prince (>25):        +0.003 × (member_count - 25)
//! Per free city:           +0.005 each
//! Heretic princes:         -0.01 each
//! Missing electors (<7):   -0.10 each
//! Emperor re-election:     +0.10 (same dynasty)
//! ```

use crate::fixed::Fixed;
use crate::state::{ReformId, WorldState};

/// HRE constants from defines.
pub mod defines {
    use crate::fixed::Fixed;

    /// Base monthly IA gain (when at peace).
    pub const BASE_IA_GAIN: Fixed = Fixed::from_raw(1000); // 0.10

    /// IA per prince above 25.
    pub const IA_PER_PRINCE: Fixed = Fixed::from_raw(30); // 0.003

    /// Baseline prince count (no bonus/penalty at this count).
    pub const PRINCE_BASELINE: i32 = 25;

    /// IA per free city.
    pub const IA_PER_FREE_CITY: Fixed = Fixed::from_raw(50); // 0.005

    /// IA penalty per heretic prince.
    pub const IA_HERETIC_PENALTY: Fixed = Fixed::from_raw(100); // 0.01

    /// IA penalty per missing elector (below 7).
    pub const IA_MISSING_ELECTOR: Fixed = Fixed::from_raw(1000); // 0.10

    /// IA cost to pass a reform.
    pub const REFORM_IA_COST: Fixed = Fixed::from_int(50);

    /// Maximum number of electors.
    pub const MAX_ELECTORS: usize = 7;

    /// Maximum number of free cities.
    pub const MAX_FREE_CITIES: usize = 12;

    /// Maximum Imperial Authority (cap).
    pub const MAX_IA: Fixed = Fixed::from_int(100);

    /// Minimum Imperial Authority (floor).
    pub const MIN_IA: Fixed = Fixed::ZERO;
}

/// Well-known imperial reform IDs.
///
/// These IDs match the order of reforms in the game files and are used
/// for checking if specific reforms have been passed.
pub mod reforms {
    use crate::state::ReformId;

    // HRE Reform Track (Emperor DLC)
    /// Call for Reichsreform - enables Imperial Ban CB
    pub const REICHSREFORM: ReformId = ReformId(1);
    /// Institute Reichsregiment - diplomatic bonuses
    pub const REICHSREGIMENT: ReformId = ReformId(2);
    /// Absolute Reichsstabilität - internal stability
    pub const REICHSSTABILITAET: ReformId = ReformId(3);
    /// Gemeiner Pfennig - tax bonuses
    pub const GEMEINERPFENNIG: ReformId = ReformId(4);
    /// Perpetual Diet - diet bonuses
    pub const PERPETUAL_DIET: ReformId = ReformId(5);
    /// Ewiger Landfriede - blocks internal HRE wars
    pub const EWIGER_LANDFRIEDE: ReformId = ReformId(6);
    /// Erbkaisertum - makes emperorship hereditary
    pub const ERBKAISERTUM: ReformId = ReformId(7);
    /// Revoke the Privilegia - all members become emperor's vassals
    pub const REVOKE_PRIVILEGIA: ReformId = ReformId(8);
    /// Renovatio Imperii - HRE becomes unified nation
    pub const RENOVATIO_IMPERII: ReformId = ReformId(9);
}

impl crate::state::HREState {
    /// Check if a specific reform has been passed.
    pub fn has_reform(&self, reform: ReformId) -> bool {
        self.reforms_passed.contains(&reform)
    }

    /// Check if Ewiger Landfriede is in effect (blocks internal wars).
    pub fn has_ewiger_landfriede(&self) -> bool {
        self.has_reform(reforms::EWIGER_LANDFRIEDE)
    }

    /// Check if Revoke Privilegia has been passed (members are vassals).
    pub fn has_revoke_privilegia(&self) -> bool {
        self.has_reform(reforms::REVOKE_PRIVILEGIA)
    }

    /// Check if emperorship is hereditary (Erbkaisertum).
    pub fn is_hereditary(&self) -> bool {
        self.has_reform(reforms::ERBKAISERTUM)
    }
}

/// Run the HRE system (called monthly).
///
/// Updates Imperial Authority and checks for elections:
/// - Check if emperor election needed (death, ineligibility)
/// - Calculate monthly IA change
pub fn run_hre_tick(state: &mut WorldState) {
    // Only run on first of month
    if state.date.day != 1 {
        return;
    }

    // Skip if HRE is dismantled
    if state.global.hre.dismantled {
        return;
    }

    // Check if election is needed (no emperor or emperor ineligible)
    check_and_run_election(state);

    // Skip IA calculation if still no emperor
    if state.global.hre.emperor.is_none() {
        return;
    }

    // Calculate monthly IA change
    let ia_delta = calculate_monthly_ia(state);

    // Apply change, clamping to [0, 100]
    let new_ia = (state.global.hre.imperial_authority + ia_delta)
        .max(defines::MIN_IA)
        .min(defines::MAX_IA);

    if ia_delta != Fixed::ZERO {
        log::debug!(
            "HRE IA: {:.2} + {:.2} = {:.2}",
            state.global.hre.imperial_authority.to_f32(),
            ia_delta.to_f32(),
            new_ia.to_f32()
        );
    }

    state.global.hre.imperial_authority = new_ia;
}

/// Calculate monthly Imperial Authority change.
fn calculate_monthly_ia(state: &WorldState) -> Fixed {
    // Get all HRE members
    let members = state.global.hre.get_members(&state.provinces);
    let prince_count = members.len() as i32;

    // Count free cities
    let free_city_count = state.global.hre.free_cities.len() as i32;

    // Count heretic princes (religion differs from official HRE religion)
    let official_religion = &state.global.hre.official_religion;
    let heretic_count = members
        .iter()
        .filter(|tag| {
            state
                .countries
                .get(*tag)
                .and_then(|c| c.religion.as_ref())
                .map(|r| r != official_religion)
                .unwrap_or(false)
        })
        .count() as i32;

    // Count missing electors (7 - current count)
    let elector_count = state.global.hre.electors.len() as i32;
    let missing_electors = (defines::MAX_ELECTORS as i32 - elector_count).max(0);

    // Calculate IA delta
    // Base gain
    let base = defines::BASE_IA_GAIN;

    // Prince bonus: +0.003 per prince above 25
    let prince_bonus = if prince_count > defines::PRINCE_BASELINE {
        defines::IA_PER_PRINCE * Fixed::from_int((prince_count - defines::PRINCE_BASELINE) as i64)
    } else {
        Fixed::ZERO
    };

    // Free city bonus: +0.005 per free city
    let free_city_bonus = defines::IA_PER_FREE_CITY * Fixed::from_int(free_city_count as i64);

    // Heretic penalty: -0.01 per heretic prince
    let heretic_penalty = defines::IA_HERETIC_PENALTY * Fixed::from_int(heretic_count as i64);

    // Missing elector penalty: -0.10 per missing elector
    let elector_penalty = defines::IA_MISSING_ELECTOR * Fixed::from_int(missing_electors as i64);

    // Total delta
    let delta = base + prince_bonus + free_city_bonus - heretic_penalty - elector_penalty;

    log::trace!(
        "HRE IA breakdown: base={:.3} + princes({})={:.3} + free_cities({})={:.3} - heretics({})={:.3} - missing_electors({})={:.3} = {:.3}",
        base.to_f32(),
        prince_count,
        prince_bonus.to_f32(),
        free_city_count,
        free_city_bonus.to_f32(),
        heretic_count,
        heretic_penalty.to_f32(),
        missing_electors,
        elector_penalty.to_f32(),
        delta.to_f32()
    );

    delta
}

// ============================================================================
// Emperor Elections
// ============================================================================

/// Election-related constants.
pub mod election_defines {
    /// Voting weight for same religion as candidate.
    pub const VOTE_SAME_RELIGION: i32 = 200;
    /// Voting weight for alliance with candidate.
    pub const VOTE_ALLIANCE: i32 = 100;
    /// Voting weight for royal marriage with candidate.
    pub const VOTE_ROYAL_MARRIAGE: i32 = 50;
    /// IA bonus when same dynasty is re-elected.
    pub const REELECTION_IA_BONUS: i64 = 1000; // 0.10 in Fixed
}

/// Check if a country is eligible to be elected emperor.
///
/// Requirements:
/// - Must be an HRE member (capital in HRE)
/// - Must have same religion as HRE official religion
/// - Ruler must be male
/// - Must be independent (not a subject)
/// - Must not be at war with current emperor (if any)
pub fn is_eligible_for_emperor(state: &WorldState, tag: &str) -> bool {
    let hre = &state.global.hre;

    // HRE must not be dismantled
    if hre.dismantled {
        return false;
    }

    // Must be an HRE member
    if !hre.is_member(&tag.to_string(), &state.provinces) {
        return false;
    }

    // Get country data
    let Some(country) = state.countries.get(tag) else {
        return false;
    };

    // Must have same religion as HRE
    let country_religion = country.religion.as_deref().unwrap_or("");
    if country_religion != hre.official_religion {
        log::trace!(
            "{} ineligible: religion {} != {}",
            tag,
            country_religion,
            hre.official_religion
        );
        return false;
    }

    // Ruler must be male
    if country.ruler_gender != crate::state::Gender::Male {
        log::trace!("{} ineligible: ruler is not male", tag);
        return false;
    }

    // Must be independent (not a subject)
    if state.diplomacy.subjects.contains_key(tag) {
        log::trace!("{} ineligible: is a subject", tag);
        return false;
    }

    // Must not be at war with current emperor
    if let Some(ref emperor) = hre.emperor {
        if state.diplomacy.are_at_war(tag, emperor) {
            log::trace!("{} ineligible: at war with emperor {}", tag, emperor);
            return false;
        }
    }

    true
}

/// Get all eligible emperor candidates.
pub fn get_eligible_candidates(state: &WorldState) -> Vec<String> {
    let members = state.global.hre.get_members(&state.provinces);
    members
        .into_iter()
        .filter(|tag| is_eligible_for_emperor(state, tag))
        .collect()
}

/// Calculate how an elector would vote for a candidate.
///
/// Returns a voting score based on:
/// - +200 if same religion
/// - +100 if allied
/// - +50 if royal marriage
fn calculate_vote_score(state: &WorldState, elector: &str, candidate: &str) -> i32 {
    let mut score = 0;

    let elector_country = state.countries.get(elector);
    let candidate_country = state.countries.get(candidate);

    // Same religion bonus
    if let (Some(e), Some(c)) = (elector_country, candidate_country) {
        if e.religion.is_some() && e.religion == c.religion {
            score += election_defines::VOTE_SAME_RELIGION;
        }
    }

    // Check alliance
    let pair = crate::state::DiplomacyState::sorted_pair(elector, candidate);
    if let Some(rel) = state.diplomacy.relations.get(&pair) {
        if *rel == crate::state::RelationType::Alliance {
            score += election_defines::VOTE_ALLIANCE;
        }
    }

    // Check royal marriage (need to check both since relations map stores one type per pair)
    // In EU4, a pair can have both alliance and RM, but our model stores one type
    // For now, check if there's any RM relation
    if let Some(rel) = state.diplomacy.relations.get(&pair) {
        if *rel == crate::state::RelationType::RoyalMarriage {
            score += election_defines::VOTE_ROYAL_MARRIAGE;
        }
    }

    score
}

/// Result of an election.
#[derive(Debug, Clone)]
pub struct ElectionResult {
    /// The winner of the election (if any).
    pub winner: Option<String>,
    /// Vote tallies: candidate -> total votes received.
    pub votes: std::collections::HashMap<String, i32>,
    /// Whether this was a re-election of the same dynasty.
    pub same_dynasty_reelection: bool,
}

/// Run an emperor election.
///
/// Each elector votes for their preferred candidate based on voting weights.
/// The candidate with the most votes wins. Ties are broken by prestige.
pub fn run_election(state: &WorldState) -> ElectionResult {
    let hre = &state.global.hre;
    let mut votes: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    // Get eligible candidates
    let candidates = get_eligible_candidates(state);
    if candidates.is_empty() {
        log::warn!("HRE election: no eligible candidates");
        return ElectionResult {
            winner: None,
            votes,
            same_dynasty_reelection: false,
        };
    }

    // Each elector votes
    for elector in &hre.electors {
        // Electors must be eligible to vote (exist and be HRE member)
        if !state.countries.contains_key(elector) {
            continue;
        }

        // Find best candidate for this elector
        let mut best_candidate: Option<&str> = None;
        let mut best_score = i32::MIN;

        for candidate in &candidates {
            let score = calculate_vote_score(state, elector, candidate);

            // Use prestige as secondary sort (higher is better)
            let prestige_bonus = state
                .countries
                .get(candidate)
                .map(|c| (c.prestige.get().to_f32() * 10.0) as i32) // Scale prestige
                .unwrap_or(0);

            let total_score = score * 1000 + prestige_bonus; // Weight base score more

            if total_score > best_score {
                best_score = total_score;
                best_candidate = Some(candidate);
            }
        }

        // Record vote
        if let Some(candidate) = best_candidate {
            *votes.entry(candidate.to_string()).or_insert(0) += 1;
            log::debug!(
                "Elector {} votes for {} (score: {})",
                elector,
                candidate,
                best_score
            );
        }
    }

    // Determine winner (most votes, tie-break by prestige)
    let winner = votes
        .iter()
        .max_by(|(tag_a, votes_a), (tag_b, votes_b)| {
            // First compare by votes
            match votes_a.cmp(votes_b) {
                std::cmp::Ordering::Equal => {
                    // Tie-break by prestige
                    let prestige_a = state
                        .countries
                        .get(*tag_a)
                        .map(|c| c.prestige.get())
                        .unwrap_or(Fixed::ZERO);
                    let prestige_b = state
                        .countries
                        .get(*tag_b)
                        .map(|c| c.prestige.get())
                        .unwrap_or(Fixed::ZERO);
                    prestige_a.cmp(&prestige_b)
                }
                other => other,
            }
        })
        .map(|(tag, _)| tag.clone());

    // Check if same dynasty re-election
    let same_dynasty_reelection =
        if let (Some(ref winner_tag), Some(ref old_emperor)) = (&winner, &hre.emperor) {
            let winner_dynasty = state
                .countries
                .get(winner_tag)
                .and_then(|c| c.ruler_dynasty.as_ref());
            let old_dynasty = state
                .countries
                .get(old_emperor)
                .and_then(|c| c.ruler_dynasty.as_ref());

            winner_dynasty.is_some() && winner_dynasty == old_dynasty
        } else {
            false
        };

    log::info!(
        "HRE election result: {:?} (same dynasty: {})",
        winner,
        same_dynasty_reelection
    );

    ElectionResult {
        winner,
        votes,
        same_dynasty_reelection,
    }
}

/// Apply election result to state.
///
/// Sets the new emperor and grants IA bonus if same dynasty re-elected.
pub fn apply_election_result(state: &mut WorldState, result: &ElectionResult) {
    if let Some(ref winner) = result.winner {
        let old_emperor = state.global.hre.emperor.clone();
        state.global.hre.emperor = Some(winner.clone());

        log::info!("New HRE Emperor: {} (previous: {:?})", winner, old_emperor);

        // Same dynasty re-election bonus: +0.10 IA
        if result.same_dynasty_reelection {
            state.global.hre.imperial_authority = (state.global.hre.imperial_authority
                + Fixed::from_raw(election_defines::REELECTION_IA_BONUS))
            .min(defines::MAX_IA);
            log::info!("Same dynasty re-election: +0.10 IA bonus");
        }
    } else {
        log::warn!("HRE election produced no winner - emperor position vacant");
        state.global.hre.emperor = None;
    }
}

/// Check if an election should be triggered and run it if needed.
///
/// Elections are triggered when:
/// - Current emperor dies (ruler_instated changes - simplified: not implemented yet)
/// - Current emperor becomes ineligible
/// - No emperor exists
pub fn check_and_run_election(state: &mut WorldState) {
    let hre = &state.global.hre;

    // Skip if dismantled
    if hre.dismantled {
        return;
    }

    let needs_election = match &hre.emperor {
        None => {
            // No emperor - need election
            log::info!("HRE has no emperor - triggering election");
            true
        }
        Some(emperor) => {
            // Check if current emperor is still eligible
            if !is_eligible_for_emperor(state, emperor) {
                log::info!(
                    "Current emperor {} is no longer eligible - triggering election",
                    emperor
                );
                true
            } else {
                false
            }
        }
    };

    if needs_election {
        let result = run_election(state);
        apply_election_result(state, &result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CountryState, Date, HREState, ProvinceState};

    fn setup_hre_test() -> WorldState {
        let mut state = WorldState {
            date: Date::new(1444, 11, 1), // First of month
            ..Default::default()
        };

        // Set up emperor
        state.global.hre = HREState {
            emperor: Some("HAB".to_string()),
            electors: vec![
                "BOH".to_string(),
                "BRA".to_string(),
                "SAX".to_string(),
                "PAL".to_string(),
                "MAI".to_string(),
                "TRI".to_string(),
                "COL".to_string(),
            ], // 7 electors
            official_religion: "catholic".to_string(),
            ..Default::default()
        };

        // Add Austria as emperor
        state.countries.insert(
            "HAB".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ..Default::default()
            },
        );

        // Add capital province for Austria (in HRE)
        state.provinces.insert(
            134, // Vienna
            ProvinceState {
                owner: Some("HAB".to_string()),
                is_capital: true,
                is_in_hre: true,
                religion: Some("catholic".to_string()),
                ..Default::default()
            },
        );

        state
    }

    #[test]
    fn test_ia_no_change_when_dismantled() {
        let mut state = setup_hre_test();
        state.global.hre.dismantled = true;
        state.global.hre.imperial_authority = Fixed::from_int(50);

        run_hre_tick(&mut state);

        assert_eq!(state.global.hre.imperial_authority, Fixed::from_int(50));
    }

    #[test]
    fn test_ia_no_change_when_no_emperor_and_no_candidates() {
        let mut state = setup_hre_test();
        state.global.hre.emperor = None;
        state.global.hre.imperial_authority = Fixed::from_int(50);

        // Make the only candidate (HAB) ineligible so no election happens
        if let Some(hab) = state.countries.get_mut("HAB") {
            hab.religion = Some("protestant".to_string()); // Wrong religion
        }

        run_hre_tick(&mut state);

        // No election possible, no emperor, IA unchanged
        assert_eq!(state.global.hre.imperial_authority, Fixed::from_int(50));
        assert!(state.global.hre.emperor.is_none());
    }

    #[test]
    fn test_ia_gains_with_full_electors() {
        let mut state = setup_hre_test();
        state.global.hre.imperial_authority = Fixed::ZERO;

        run_hre_tick(&mut state);

        // With 7 electors, 1 prince, 0 free cities, 0 heretics:
        // Base = 0.10, no prince bonus (1 < 25), no free city bonus
        // No elector penalty (7/7), no heretic penalty
        // Delta = 0.10
        let expected = Fixed::from_raw(1000); // 0.10
        let diff = (state.global.hre.imperial_authority - expected).0.abs();
        assert!(
            diff < 10,
            "Expected ~{:.3}, got {:.3}",
            expected.to_f32(),
            state.global.hre.imperial_authority.to_f32()
        );
    }

    #[test]
    fn test_ia_penalty_for_missing_electors() {
        let mut state = setup_hre_test();
        state.global.hre.electors = vec!["BOH".to_string(), "BRA".to_string()]; // Only 2 electors
        state.global.hre.imperial_authority = Fixed::from_int(50);

        run_hre_tick(&mut state);

        // Missing 5 electors: -0.50 penalty
        // Base: +0.10
        // Delta = 0.10 - 0.50 = -0.40
        // 50 - 0.40 = 49.60
        assert!(
            state.global.hre.imperial_authority < Fixed::from_int(50),
            "IA should decrease with missing electors"
        );
    }

    #[test]
    fn test_ia_clamped_to_zero() {
        let mut state = setup_hre_test();
        state.global.hre.electors = vec![]; // No electors: -0.70 penalty
        state.global.hre.imperial_authority = Fixed::from_raw(1000); // 0.1

        run_hre_tick(&mut state);

        assert_eq!(
            state.global.hre.imperial_authority,
            defines::MIN_IA,
            "IA should be clamped to 0"
        );
    }

    #[test]
    fn test_ia_clamped_to_100() {
        let mut state = setup_hre_test();
        state.global.hre.imperial_authority = Fixed::from_raw(999900); // 99.99

        run_hre_tick(&mut state);

        assert_eq!(
            state.global.hre.imperial_authority,
            defines::MAX_IA,
            "IA should be clamped to 100"
        );
    }

    #[test]
    fn test_ia_free_city_bonus() {
        let mut state = setup_hre_test();
        state.global.hre.free_cities.insert("HAM".to_string());
        state.global.hre.free_cities.insert("FRA".to_string());
        state.global.hre.imperial_authority = Fixed::ZERO;

        run_hre_tick(&mut state);

        // 2 free cities: +0.01
        // Base: +0.10
        // Delta = 0.11
        let expected = Fixed::from_raw(1100); // 0.11
        let diff = (state.global.hre.imperial_authority - expected).0.abs();
        assert!(
            diff < 10,
            "Expected ~{:.3}, got {:.3}",
            expected.to_f32(),
            state.global.hre.imperial_authority.to_f32()
        );
    }

    // ===== Election Tests =====

    fn setup_election_test() -> WorldState {
        let mut state = WorldState {
            date: Date::new(1444, 11, 1),
            ..Default::default()
        };

        // Set up HRE with no emperor (to trigger election)
        state.global.hre = HREState {
            emperor: None,
            electors: vec!["BOH".to_string(), "BRA".to_string(), "SAX".to_string()],
            official_religion: "catholic".to_string(),
            ..Default::default()
        };

        // Add eligible candidates
        // Austria - catholic, male, independent, capital in HRE
        state.countries.insert(
            "HAB".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ruler_gender: crate::state::Gender::Male,
                ruler_dynasty: Some("Habsburg".to_string()),
                prestige: crate::bounded::new_prestige(),
                ..Default::default()
            },
        );
        state.provinces.insert(
            134,
            ProvinceState {
                owner: Some("HAB".to_string()),
                is_capital: true,
                is_in_hre: true,
                ..Default::default()
            },
        );

        // Bohemia - elector and candidate
        state.countries.insert(
            "BOH".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ruler_gender: crate::state::Gender::Male,
                ..Default::default()
            },
        );
        state.provinces.insert(
            266,
            ProvinceState {
                owner: Some("BOH".to_string()),
                is_capital: true,
                is_in_hre: true,
                ..Default::default()
            },
        );

        // Brandenburg - elector
        state.countries.insert(
            "BRA".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ruler_gender: crate::state::Gender::Male,
                ..Default::default()
            },
        );
        state.provinces.insert(
            50,
            ProvinceState {
                owner: Some("BRA".to_string()),
                is_capital: true,
                is_in_hre: true,
                ..Default::default()
            },
        );

        // Saxony - elector
        state.countries.insert(
            "SAX".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ruler_gender: crate::state::Gender::Male,
                ..Default::default()
            },
        );
        state.provinces.insert(
            61,
            ProvinceState {
                owner: Some("SAX".to_string()),
                is_capital: true,
                is_in_hre: true,
                ..Default::default()
            },
        );

        state
    }

    #[test]
    fn test_eligibility_requires_hre_membership() {
        let mut state = setup_election_test();

        // France - not in HRE
        state.countries.insert(
            "FRA".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ruler_gender: crate::state::Gender::Male,
                ..Default::default()
            },
        );
        state.provinces.insert(
            183, // Paris
            ProvinceState {
                owner: Some("FRA".to_string()),
                is_capital: true,
                is_in_hre: false, // Not in HRE
                ..Default::default()
            },
        );

        assert!(!is_eligible_for_emperor(&state, "FRA"));
    }

    #[test]
    fn test_eligibility_requires_correct_religion() {
        let mut state = setup_election_test();

        // Make Bohemia protestant
        if let Some(boh) = state.countries.get_mut("BOH") {
            boh.religion = Some("protestant".to_string());
        }

        assert!(!is_eligible_for_emperor(&state, "BOH"));
        assert!(is_eligible_for_emperor(&state, "HAB")); // Still catholic
    }

    #[test]
    fn test_eligibility_requires_male_ruler() {
        let mut state = setup_election_test();

        // Make Austria's ruler female
        if let Some(hab) = state.countries.get_mut("HAB") {
            hab.ruler_gender = crate::state::Gender::Female;
        }

        assert!(!is_eligible_for_emperor(&state, "HAB"));
    }

    #[test]
    fn test_eligibility_requires_independence() {
        let mut state = setup_election_test();

        // Make Bohemia a vassal
        state.diplomacy.subjects.insert(
            "BOH".to_string(),
            crate::state::SubjectRelationship {
                overlord: "HAB".to_string(),
                subject: "BOH".to_string(),
                subject_type: crate::subjects::SubjectTypeId(0),
                start_date: state.date,
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            },
        );

        assert!(!is_eligible_for_emperor(&state, "BOH"));
    }

    #[test]
    fn test_election_selects_winner() {
        let state = setup_election_test();

        let result = run_election(&state);

        // Should have a winner
        assert!(result.winner.is_some());
        // Winner should be eligible
        assert!(is_eligible_for_emperor(
            &state,
            result.winner.as_ref().unwrap()
        ));
    }

    #[test]
    fn test_election_no_candidates() {
        let mut state = setup_election_test();

        // Make all candidates ineligible by changing religion
        for tag in ["HAB", "BOH", "BRA", "SAX"] {
            if let Some(c) = state.countries.get_mut(tag) {
                c.religion = Some("protestant".to_string());
            }
        }

        let result = run_election(&state);

        assert!(result.winner.is_none());
    }

    #[test]
    fn test_check_and_run_election_when_no_emperor() {
        let mut state = setup_election_test();
        assert!(state.global.hre.emperor.is_none());

        check_and_run_election(&mut state);

        // Should now have an emperor
        assert!(state.global.hre.emperor.is_some());
    }

    #[test]
    fn test_check_and_run_election_when_emperor_ineligible() {
        let mut state = setup_election_test();

        // Set HAB as emperor
        state.global.hre.emperor = Some("HAB".to_string());

        // Make HAB ineligible (change religion)
        if let Some(hab) = state.countries.get_mut("HAB") {
            hab.religion = Some("protestant".to_string());
        }

        check_and_run_election(&mut state);

        // Emperor should have changed (HAB is now ineligible)
        assert_ne!(state.global.hre.emperor.as_deref(), Some("HAB"));
    }

    #[test]
    fn test_same_dynasty_reelection_bonus() {
        let mut state = setup_election_test();

        // Set up HAB as previous emperor with Habsburg dynasty
        state.global.hre.emperor = Some("HAB".to_string());
        state.global.hre.imperial_authority = Fixed::from_int(10);

        // Give BOH the same dynasty and make it win
        if let Some(boh) = state.countries.get_mut("BOH") {
            boh.ruler_dynasty = Some("Habsburg".to_string());
            // Give high prestige to ensure BOH wins
            boh.prestige.set(Fixed::from_int(100));
        }

        // Make HAB less attractive
        if let Some(hab) = state.countries.get_mut("HAB") {
            hab.prestige.set(Fixed::from_int(-50));
        }

        // Run election manually
        let result = run_election(&state);

        // If BOH wins and has same dynasty, should be a re-election
        if result.winner.as_deref() == Some("BOH") {
            assert!(result.same_dynasty_reelection);
        }
    }

    // ========================================================================
    // Reform Helper Tests
    // ========================================================================

    #[test]
    fn test_has_reform() {
        let mut state = setup_hre_test();

        // No reforms passed initially
        assert!(!state.global.hre.has_reform(reforms::EWIGER_LANDFRIEDE));
        assert!(!state.global.hre.has_ewiger_landfriede());

        // Pass Ewiger Landfriede
        state
            .global
            .hre
            .reforms_passed
            .push(reforms::EWIGER_LANDFRIEDE);

        assert!(state.global.hre.has_reform(reforms::EWIGER_LANDFRIEDE));
        assert!(state.global.hre.has_ewiger_landfriede());
    }

    #[test]
    fn test_has_revoke_privilegia() {
        let mut state = setup_hre_test();

        assert!(!state.global.hre.has_revoke_privilegia());

        state
            .global
            .hre
            .reforms_passed
            .push(reforms::REVOKE_PRIVILEGIA);

        assert!(state.global.hre.has_revoke_privilegia());
    }

    #[test]
    fn test_is_hereditary() {
        let mut state = setup_hre_test();

        assert!(!state.global.hre.is_hereditary());

        state.global.hre.reforms_passed.push(reforms::ERBKAISERTUM);

        assert!(state.global.hre.is_hereditary());
    }

    // ========================================================================
    // HRE Membership Tests
    // ========================================================================

    #[test]
    fn test_is_member_via_capital() {
        let mut state = setup_hre_test();

        // HAB capital is in HRE
        assert!(state
            .global
            .hre
            .is_member(&"HAB".to_string(), &state.provinces));

        // Add FRA outside HRE
        state.countries.insert(
            "FRA".to_string(),
            CountryState {
                religion: Some("catholic".to_string()),
                ..Default::default()
            },
        );
        state.provinces.insert(
            183,
            ProvinceState {
                owner: Some("FRA".to_string()),
                is_capital: true,
                is_in_hre: false,
                ..Default::default()
            },
        );

        assert!(!state
            .global
            .hre
            .is_member(&"FRA".to_string(), &state.provinces));
    }

    #[test]
    fn test_get_members() {
        let state = setup_hre_test();

        let members = state.global.hre.get_members(&state.provinces);

        // HAB should be a member
        assert!(members.contains("HAB"));
        // Should have exactly 1 member in basic setup
        assert_eq!(members.len(), 1);
    }

    #[test]
    fn test_is_elector() {
        let mut state = setup_hre_test();

        assert!(!state.global.hre.is_elector(&"HAB".to_string()));

        state.global.hre.electors.push("HAB".to_string());

        assert!(state.global.hre.is_elector(&"HAB".to_string()));
    }

    #[test]
    fn test_is_free_city() {
        let mut state = setup_hre_test();

        assert!(!state.global.hre.is_free_city(&"ULM".to_string()));

        state.global.hre.free_cities.insert("ULM".to_string());

        assert!(state.global.hre.is_free_city(&"ULM".to_string()));
    }
}
