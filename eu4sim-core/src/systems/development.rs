use crate::fixed::Fixed;
use crate::input::DevType;
use crate::state::{ProvinceId, Tag, WorldState};
use anyhow::{anyhow, Result};

/// Executes the DevelopProvince command.
///
/// Costs 50 monarch power of the corresponding type and increases the base development of the province.
pub fn develop_province(
    state: &mut WorldState,
    country: Tag,
    province_id: ProvinceId,
    dev_type: DevType,
) -> Result<()> {
    let province = state
        .provinces
        .get_mut(&province_id)
        .ok_or_else(|| anyhow!("Province {} not found", province_id))?;

    if province.owner.as_ref() != Some(&country) {
        return Err(anyhow!(
            "Country {} does not own province {}",
            country,
            province_id
        ));
    }

    let country_state = state
        .countries
        .get_mut(&country)
        .ok_or_else(|| anyhow!("Country {} not found", country))?;

    let cost = Fixed::from_int(50);

    match dev_type {
        DevType::Tax => {
            if country_state.adm_mana < cost {
                return Err(anyhow!("Not enough ADM mana to develop province"));
            }
            country_state.adm_mana -= cost;
            province.base_tax += Fixed::from_int(1);
        }
        DevType::Production => {
            if country_state.dip_mana < cost {
                return Err(anyhow!("Not enough DIP mana to develop province"));
            }
            country_state.dip_mana -= cost;
            province.base_production += Fixed::from_int(1);
        }
        DevType::Manpower => {
            if country_state.mil_mana < cost {
                return Err(anyhow!("Not enough MIL mana to develop province"));
            }
            country_state.mil_mana -= cost;
            province.base_manpower += Fixed::from_int(1);
        }
    }

    Ok(())
}
