use crate::fixed::Fixed;
use crate::state::{Tag, TechType, WorldState};
use anyhow::{anyhow, Result};

/// Executes the BuyTech command.
///
/// Simplified tech system for mid-term goal.
/// Cost formula: 600 * (1 + level * 0.1) - very basic linear increase.
pub fn buy_tech(state: &mut WorldState, country: Tag, tech_type: TechType) -> Result<()> {
    let country_state = state
        .countries
        .get_mut(&country)
        .ok_or_else(|| anyhow!("Country {} not found", country))?;

    let current_level = match tech_type {
        TechType::Adm => country_state.adm_tech,
        TechType::Dip => country_state.dip_tech,
        TechType::Mil => country_state.mil_tech,
    };

    if current_level >= 32 {
        return Err(anyhow!("Already at maximum tech level 32"));
    }

    // Basic cost formula: 600 base + 60 per existing level (10% increase per level)
    let cost = Fixed::from_int(600 + (current_level as i64 * 60));

    match tech_type {
        TechType::Adm => {
            if country_state.adm_mana < cost {
                return Err(anyhow!(
                    "Not enough ADM mana for tech level {}",
                    current_level + 1
                ));
            }
            country_state.adm_mana -= cost;
            country_state.adm_tech += 1;
        }
        TechType::Dip => {
            if country_state.dip_mana < cost {
                return Err(anyhow!(
                    "Not enough DIP mana for tech level {}",
                    current_level + 1
                ));
            }
            country_state.dip_mana -= cost;
            country_state.dip_tech += 1;
        }
        TechType::Mil => {
            if country_state.mil_mana < cost {
                return Err(anyhow!(
                    "Not enough MIL mana for tech level {}",
                    current_level + 1
                ));
            }
            country_state.mil_mana -= cost;
            country_state.mil_tech += 1;
        }
    }

    Ok(())
}
