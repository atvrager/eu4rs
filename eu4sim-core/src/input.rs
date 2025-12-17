use crate::state::{ArmyId, FleetId, ProvinceId, Tag};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputs {
    pub country: Tag,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Command {
    // Economic
    BuildInProvince {
        province: ProvinceId,
        building: String,
    },
    // SetMerchant { trade_node: TradeNodeId, action: MerchantAction },
    // RaiseTaxes { province: ProvinceId },

    // Military
    // RecruitUnit { province: ProvinceId, unit_type: UnitType },
    Move {
        army_id: ArmyId,
        destination: ProvinceId,
    },
    MoveFleet {
        fleet_id: FleetId,
        destination: ProvinceId,
    },
    Embark {
        army_id: ArmyId,
        fleet_id: FleetId,
    },
    Disembark {
        army_id: ArmyId,
        destination: ProvinceId,
    },

    // Diplomatic
    DeclareWar {
        target: Tag,
    },
    // SendDiplomat { target: Tag, action: DiplomaticAction },
    // AcceptPeace { war_id: WarId },

    // Internal
    // SetNationalFocus { focus: NationalFocus },
    // PassLaw { law: LawType },

    // Meta
    Quit,
}
