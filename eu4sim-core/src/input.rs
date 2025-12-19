use crate::state::{ArmyId, FleetId, PeaceTerms, ProvinceId, Tag, WarId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputs {
    pub country: Tag,
    pub commands: Vec<Command>,
}

/// Type of development that can be purchased
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DevType {
    /// Administrative development (base_tax)
    Tax,
    /// Diplomatic development (base_production)
    Production,
    /// Military development (base_manpower)
    Manpower,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Command {
    // Economic
    BuildInProvince {
        province: ProvinceId,
        building: String,
    },
    PurchaseDevelopment {
        province: ProvinceId,
        dev_type: DevType,
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
    /// Offer peace terms in a war
    OfferPeace {
        war_id: WarId,
        terms: PeaceTerms,
    },
    /// Accept a pending peace offer
    AcceptPeace {
        war_id: WarId,
    },
    /// Reject a pending peace offer
    RejectPeace {
        war_id: WarId,
    },
    // SendDiplomat { target: Tag, action: DiplomaticAction },

    // Internal
    // SetNationalFocus { focus: NationalFocus },
    // PassLaw { law: LawType },

    // Meta
    Quit,
}
