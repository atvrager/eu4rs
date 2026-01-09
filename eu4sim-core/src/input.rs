use crate::ai::VisibleWorldState;
use crate::estates::{EstateTypeId, PrivilegeId};
use crate::ideas::IdeaGroupId;
use crate::state::{
    ArmyId, CelestialReformId, FleetId, InstitutionId, PeaceTerms, ProvinceId, ReformId, Tag,
    TechType, WarId,
};
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

    // Economic - Buildings
    BuildInProvince {
        province: ProvinceId,
        building: String,
    },
    /// Cancel an in-progress construction (100% refund).
    CancelConstruction {
        province: ProvinceId,
    },
    /// Demolish a completed building (no refund).
    DemolishBuilding {
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
    /// Decline a call-to-arms from an ally.
    /// Penalties: -25 prestige, alliance breaks, -10 trust with all allies.
    DeclineCallToArms {
        war_id: WarId,
    },

    // Tech & Institutions
    BuyTech {
        tech_type: TechType,
    },
    EmbraceInstitution {
        institution: InstitutionId,
    },

    // Ideas
    /// Pick a new idea group (max 8 per country).
    /// Cannot pick national idea groups (they are auto-assigned).
    /// Costs no mana to pick, but unlocking ideas does.
    PickIdeaGroup {
        group_id: IdeaGroupId,
    },
    /// Unlock the next idea in a picked idea group.
    /// Costs 400 base mana of the group's category (ADM/DIP/MIL).
    /// Reduced by idea cost modifiers.
    UnlockIdea {
        group_id: IdeaGroupId,
    },

    // Estates
    /// Grant a privilege to an estate.
    /// Increases estate loyalty and influence, grants country modifiers.
    /// May reduce crown land and max absolutism.
    GrantPrivilege {
        estate_id: EstateTypeId,
        privilege_id: PrivilegeId,
    },
    /// Revoke a privilege from an estate.
    /// Decreases estate loyalty, removes bonuses.
    /// Subject to cooldown timer.
    RevokePrivilege {
        estate_id: EstateTypeId,
        privilege_id: PrivilegeId,
    },
    /// Seize land from estates to increase crown land.
    /// Costs loyalty with all estates.
    /// Increases crown land percentage.
    SeizeLand {
        percentage: u8,
    },
    /// Sell crown land to an estate.
    /// Increases loyalty and influence with the estate.
    /// Decreases crown land percentage.
    SaleLand {
        estate_id: EstateTypeId,
        percentage: u8,
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

    // Holy Roman Empire
    /// Add a province to the HRE. Requires emperor approval.
    /// Province must border existing HRE territory and not be capital of non-HRE nation.
    AddProvinceToHRE {
        province: ProvinceId,
    },
    /// Remove a province from the HRE. Angers the emperor.
    RemoveProvinceFromHRE {
        province: ProvinceId,
    },
    /// Join the HRE (adds capital to HRE territory).
    /// Requires emperor approval and capital bordering HRE.
    JoinHRE,
    /// Leave the HRE (removes capital from HRE).
    /// Cannot be done if emperor.
    LeaveHRE,
    /// Grant elector status to an HRE member (emperor only).
    /// Max 7 electors.
    GrantElectorate {
        target: Tag,
    },
    /// Remove elector status from an HRE member (emperor only).
    /// Typically done for heretic electors.
    RemoveElectorate {
        target: Tag,
    },
    /// Grant Free Imperial City status (emperor only).
    /// Target must be OPM and HRE member. Max 12 free cities.
    GrantFreeCity {
        target: Tag,
    },
    /// Revoke Free Imperial City status (emperor only).
    RevokeFreeCity {
        target: Tag,
    },
    /// Pass an imperial reform (emperor only).
    /// Costs 50 IA and requires majority support from electors.
    PassImperialReform {
        reform: ReformId,
    },
    /// Issue an imperial ban against a nation (emperor only).
    /// Unlocks Imperial Ban CB to reclaim HRE territory.
    ImperialBan {
        target: Tag,
    },

    // Celestial Empire (Emperor of China)
    /// Claim the Mandate of Heaven (via peace deal or decision).
    /// Transfers emperor status, resets reforms, sets mandate to 80.
    TakeMandate,
    /// Pass a celestial reform (emperor only).
    /// Requires 80+ mandate, costs 70 mandate and 1 stability.
    PassCelestialReform {
        reform: CelestialReformId,
    },
    /// Issue a celestial decree (emperor only).
    /// Costs 20 meritocracy, lasts 10 years.
    IssueCelestialDecree {
        decree: String,
    },
    /// Force a nation to become tributary (peace deal).
    ForceTributary {
        target: Tag,
    },
    /// Diplomatic request for tributary status.
    RequestTributary {
        target: Tag,
    },
    /// Release a tributary from subject status.
    RevokeTributary {
        target: Tag,
    },
    /// Spend 100 MIL power for +10 meritocracy (emperor only).
    StrengthenGovernment,
    /// Abandon the Mandate of Heaven.
    /// Gives up Celestial Empire status.
    AbandonMandate,

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
