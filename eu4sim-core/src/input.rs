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
    // ===== IMPLEMENTED COMMANDS =====

    // Economic
    BuildInProvince {
        province: ProvinceId,
        building: String,
    },
    PurchaseDevelopment {
        province: ProvinceId,
        dev_type: DevType,
    },

    // Military
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

    // Diplomatic - War
    DeclareWar {
        target: Tag,
        cb: Option<String>,
    },
    OfferPeace {
        war_id: WarId,
        terms: PeaceTerms,
    },
    AcceptPeace {
        war_id: WarId,
    },
    RejectPeace {
        war_id: WarId,
    },

    // ===== STUB COMMANDS (Phase 2+) =====

    // Military (additional)
    MergeArmies {
        army_ids: Vec<ArmyId>,
    },
    SplitArmy {
        army_id: ArmyId,
        regiment_count: u32,
    },

    // Colonization
    StartColony {
        province: ProvinceId,
    },
    AbandonColony {
        province: ProvinceId,
    },

    // Diplomacy - Outgoing
    OfferAlliance {
        target: Tag,
    },
    BreakAlliance {
        target: Tag,
    },
    OfferRoyalMarriage {
        target: Tag,
    },
    BreakRoyalMarriage {
        target: Tag,
    },
    RequestMilitaryAccess {
        target: Tag,
    },
    CancelMilitaryAccess {
        target: Tag,
    },
    SetRival {
        target: Tag,
    },
    RemoveRival {
        target: Tag,
    },

    // Diplomacy - Responses
    AcceptAlliance {
        from: Tag,
    },
    RejectAlliance {
        from: Tag,
    },
    AcceptRoyalMarriage {
        from: Tag,
    },
    RejectRoyalMarriage {
        from: Tag,
    },
    GrantMilitaryAccess {
        to: Tag,
    },
    DenyMilitaryAccess {
        to: Tag,
    },

    // Tech & Institutions
    BuyTech {
        tech_type: String, // "ADM", "DIP", or "MIL"
    },
    EmbraceInstitution {
        institution: String,
    },

    // Religion
    AssignMissionary {
        province: ProvinceId,
    },
    RecallMissionary {
        province: ProvinceId,
    },
    ConvertCountryReligion {
        religion: String,
    },

    // Development (renamed from PurchaseDevelopment for consistency)
    DevelopProvince {
        province: ProvinceId,
        dev_type: DevType,
    },

    // Control
    MoveCapital {
        province: ProvinceId,
    },
    Pass,

    // Meta
    Quit,
}
