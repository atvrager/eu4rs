use crate::ai::VisibleWorldState;
use crate::state::{ArmyId, FleetId, InstitutionId, PeaceTerms, ProvinceId, Tag, TechType, WarId};
use crate::trade::{MerchantAction, TradeNodeId};
use serde::{Deserialize, Serialize};

/// Which side to join in a war.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarSide {
    Attacker,
    Defender,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputs {
    pub country: Tag,
    pub commands: Vec<Command>,
    /// Available commands at the time of decision (precomputed, for datagen).
    ///
    /// **Note**: This field is only populated when observers need it (e.g., datagen mode).
    /// In normal simulation without observers, this will be an empty Vec to save memory.
    /// Do not rely on this being populated unless running with `--datagen`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_commands: Vec<Command>,
    /// Visible world state at decision time (precomputed, for datagen).
    ///
    /// **Note**: Only populated in datagen mode to avoid recomputing in observers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_state: Option<VisibleWorldState>,
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
    DevelopProvince {
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
    JoinWar {
        war_id: WarId,
        side: WarSide,
    },
    CallAllyToWar {
        ally: Tag,
        war_id: WarId,
    },

    // Tech & Institutions
    BuyTech {
        tech_type: TechType,
    },
    EmbraceInstitution {
        institution: InstitutionId,
    },

    // Trade
    SendMerchant {
        node: TradeNodeId,
        action: MerchantAction,
    },
    RecallMerchant {
        node: TradeNodeId,
    },
    UpgradeCenterOfTrade {
        province: ProvinceId,
    },

    // Province Administration
    /// Start coring an owned province to reduce overextension and autonomy.
    /// Costs 10 ADM per development, takes 36 months to complete.
    Core {
        province: ProvinceId,
    },

    // Military Recruitment
    /// Recruit a regiment at specified province.
    /// Infantry: always available. Cavalry: always available.
    /// Artillery: requires mil tech 7+ (see `can_recruit_artillery`).
    /// Costs gold and uses manpower.
    RecruitRegiment {
        province: ProvinceId,
        unit_type: crate::state::RegimentType,
    },

    /// Recruit a new general (costs MIL mana).
    RecruitGeneral,

    /// Assign a general to an army.
    AssignGeneral {
        general: crate::state::GeneralId,
        army: ArmyId,
    },

    /// Remove a general from an army.
    UnassignGeneral {
        army: ArmyId,
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

    // Control
    MoveCapital {
        province: ProvinceId,
    },
    Pass,

    // Meta
    Quit,
}

impl Command {
    /// Returns the category of this command for multi-action AI selection.
    ///
    /// Wraps the module-level `categorize_command()` function for convenience.
    pub fn category(&self) -> crate::ai::CommandCategory {
        crate::ai::categorize_command(self)
    }
}
