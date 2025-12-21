# Cap'n Proto schema for EU4 simulation training data
#
# This schema defines the binary format for ML training samples generated
# by the Rust simulation and consumed by Python training pipelines.
#
# Schema evolution rules:
# - New fields MUST be added at the end with the next ordinal number
# - Field ordinals MUST NOT be reused or reordered
# - Deprecated fields should be renamed with "deprecated" prefix
# - See: https://capnproto.org/language.html#evolving-your-protocol

@0xb8e7d4a2c3f1e590;

# =============================================================================
# Basic Types
# =============================================================================

struct Date {
  year @0 :Int16;
  month @1 :UInt8;
  day @2 :UInt8;
}

# Fixed-point number representation (matches Rust's Fixed type)
# Value = raw / 10000 (e.g., raw=15000 means 1.5)
struct Fixed {
  raw @0 :Int64;
}

# =============================================================================
# Enums
# =============================================================================

enum TechType {
  adm @0;
  dip @1;
  mil @2;
}

enum DevType {
  tax @0;
  production @1;
  manpower @2;
}

# =============================================================================
# Map Entry Types (Cap'n Proto has no native map type)
# =============================================================================

struct CountryStrengthEntry {
  country @0 :Text;
  strength @1 :UInt32;
}

struct WarScoreEntry {
  warId @0 :UInt32;
  score @1 :Fixed;
}

# =============================================================================
# Peace Terms (union for variant types)
# =============================================================================

struct PeaceTerms {
  union {
    whitePeace @0 :Void;
    takeProvinces @1 :List(UInt32);  # List of province IDs
    fullAnnexation @2 :Void;
  }
}

# =============================================================================
# Country State
# =============================================================================

struct CountryState {
  treasury @0 :Fixed;
  manpower @1 :Fixed;
  stability @2 :Int8;           # -3 to +3
  prestige @3 :Fixed;           # -100 to +100
  armyTradition @4 :Fixed;      # 0 to 100
  admMana @5 :Fixed;
  dipMana @6 :Fixed;
  milMana @7 :Fixed;
  admTech @8 :UInt8;
  dipTech @9 :UInt8;
  milTech @10 :UInt8;
  embracedInstitutions @11 :List(Text);
  religion @12 :Text;           # Empty string if none
}

# =============================================================================
# Visible World State (AI's view of the game)
# =============================================================================

struct VisibleWorldState {
  date @0 :Date;
  observer @1 :Text;            # Country tag (e.g., "FRA")
  ownCountry @2 :CountryState;
  atWar @3 :Bool;
  knownCountries @4 :List(Text);
  enemyProvinces @5 :List(UInt32);
  knownCountryStrength @6 :List(CountryStrengthEntry);
  ourWarScore @7 :List(WarScoreEntry);
}

# =============================================================================
# Command (large union covering all game actions)
# =============================================================================

struct Command {
  union {
    # Control
    pass @0 :Void;
    quit @1 :Void;

    # Military Movement
    move :group {
      armyId @2 :UInt32;
      destination @3 :UInt32;
    }
    moveFleet :group {
      fleetId @4 :UInt32;
      destination @5 :UInt32;
    }
    embark :group {
      armyId @6 :UInt32;
      fleetId @7 :UInt32;
    }
    disembark :group {
      armyId @8 :UInt32;
      destination @9 :UInt32;
    }
    mergeArmies @10 :List(UInt32);  # List of army IDs
    splitArmy :group {
      armyId @11 :UInt32;
      regimentCount @12 :UInt32;
    }

    # War & Peace
    declareWar :group {
      target @13 :Text;
      cb @14 :Text;             # Empty string if none
    }
    offerPeace :group {
      warId @15 :UInt32;
      terms @16 :PeaceTerms;
    }
    acceptPeace @17 :UInt32;    # war_id
    rejectPeace @18 :UInt32;    # war_id

    # Tech & Institutions
    buyTech @19 :TechType;
    embraceInstitution @20 :Text;  # institution_id

    # Economic
    buildInProvince :group {
      province @21 :UInt32;
      building @22 :Text;
    }
    developProvince :group {
      province @23 :UInt32;
      devType @24 :DevType;
    }

    # Colonization
    startColony @25 :UInt32;    # province_id
    abandonColony @26 :UInt32;  # province_id

    # Diplomacy - Outgoing
    offerAlliance @27 :Text;
    breakAlliance @28 :Text;
    offerRoyalMarriage @29 :Text;
    breakRoyalMarriage @30 :Text;
    requestMilitaryAccess @31 :Text;
    cancelMilitaryAccess @32 :Text;
    setRival @33 :Text;
    removeRival @34 :Text;

    # Diplomacy - Responses
    acceptAlliance @35 :Text;
    rejectAlliance @36 :Text;
    acceptRoyalMarriage @37 :Text;
    rejectRoyalMarriage @38 :Text;
    grantMilitaryAccess @39 :Text;
    denyMilitaryAccess @40 :Text;

    # Religion
    assignMissionary @41 :UInt32;   # province_id
    recallMissionary @42 :UInt32;   # province_id
    convertCountryReligion @43 :Text;

    # Control
    moveCapital @44 :UInt32;    # province_id
  }
}

# =============================================================================
# Training Sample (one decision point)
# =============================================================================

struct TrainingSample {
  tick @0 :UInt64;
  country @1 :Text;
  state @2 :VisibleWorldState;
  availableCommands @3 :List(Command);
  chosenAction @4 :Int32;       # Index into availableCommands, -1 for Pass
  chosenCommand @5 :Command;    # The actual command (for debugging)
}

# =============================================================================
# Training Batch (for efficient I/O)
# =============================================================================

struct TrainingBatch {
  # Year this batch covers (for organizing by year like the ZIP format)
  year @0 :Int16;
  samples @1 :List(TrainingSample);
}

# File-level container for streaming multiple batches
struct TrainingFile {
  # Schema version for compatibility checking
  schemaVersion @0 :UInt16;
  batches @1 :List(TrainingBatch);
}
