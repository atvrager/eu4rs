use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::{ExtractedState, SaveMeta};

/// Load and parse an EU4 save file
pub fn load_save(path: &Path) -> Result<ExtractedState> {
    log::info!("Loading save file: {}", path.display());

    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read save file: {}", path.display()))?;

    log::info!("Read {} bytes from {}", data.len(), path.display());

    // Check if it's a ZIP archive or plain text
    if data.starts_with(b"PK") {
        // ZIP archive (ironman or compressed text save)
        log::info!("Detected ZIP archive format");
        let cursor = std::io::Cursor::new(&data);
        let mut archive =
            zip::ZipArchive::new(cursor).with_context(|| "Failed to read save as ZIP archive")?;

        log::info!("Archive contains {} files", archive.len());
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            log::debug!("  {}: {} bytes", file.name(), file.size());
        }

        // Read meta file for date/player info
        let meta = read_meta(&mut archive);
        log::debug!("Meta: {:?}", meta);

        let gamestate = read_gamestate(&mut archive)?;
        log::info!("Read gamestate: {} bytes", gamestate.len());
        let mut state = parse_gamestate(&gamestate)?;

        // Override meta with meta file data if available
        if let Some((date, player)) = meta {
            state.meta.date = date;
            if player.is_some() {
                state.meta.player = player;
            }
        }

        Ok(state)
    } else if data.starts_with(b"EU4txt") || data.starts_with(b"EU4bin") {
        // Plain text or binary file (not zipped)
        log::info!("Detected plain save format (not zipped)");
        parse_gamestate(&data)
    } else {
        // Try to detect format
        let sample = &data[..std::cmp::min(1000, data.len())];
        if sample.iter().all(|&b| b.is_ascii() || b > 127) {
            log::info!("Assuming plain text format");
            parse_text_gamestate(&data)
        } else {
            anyhow::bail!("Unknown save file format")
        }
    }
}

fn read_gamestate<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<u8>> {
    // Try common gamestate file names
    let candidates = ["gamestate", "meta", "ai"];

    for name in candidates {
        if let Ok(mut file) = archive.by_name(name) {
            let mut content = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut content)?;
            return Ok(content);
        }
    }

    // If no known file found, try first file
    if !archive.is_empty() {
        let mut file = archive.by_index(0)?;
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut content)?;
        return Ok(content);
    }

    anyhow::bail!("No gamestate file found in save archive")
}

/// Read date and player from meta file in archive
fn read_meta<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Option<(String, Option<String>)> {
    let mut file = archive.by_name("meta").ok()?;
    let mut content = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut content).ok()?;

    let text = String::from_utf8_lossy(&content);

    // Extract date from meta
    let date = extract_date(&text)?;
    let player = extract_player(&text);

    Some((date, player))
}

fn parse_gamestate(data: &[u8]) -> Result<ExtractedState> {
    // Check if binary or text format
    let is_binary = data.starts_with(b"EU4bin");
    let is_text = data.starts_with(b"EU4txt");

    if is_binary {
        log::info!("Detected binary (Ironman) save format");
        parse_binary_gamestate(data)
    } else if is_text {
        log::info!("Detected text save format");
        parse_text_gamestate(data)
    } else {
        // Try to detect format from content
        if data
            .iter()
            .take(1000)
            .any(|&b| b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t')
        {
            log::info!("Detected binary format (no header)");
            parse_binary_gamestate(data)
        } else {
            log::info!("Detected text format (no header)");
            parse_text_gamestate(data)
        }
    }
}

fn parse_binary_gamestate(data: &[u8]) -> Result<ExtractedState> {
    use eu4save::{EnvTokens, Eu4File, PdsDate};

    // Check for tokens
    let tokens_path = find_tokens_file();
    if tokens_path.is_none() && std::env::var("EU4_IRONMAN_TOKENS").is_err() {
        anyhow::bail!(
            "Binary (Ironman) save requires token file.\n\n\
            The token file maps binary field IDs to names. Options:\n\n\
            1. Set EU4_IRONMAN_TOKENS=/path/to/eu4.txt\n\
            2. Place tokens at assets/tokens/eu4.txt\n\n\
            Token sources:\n\
            - pdx-tools (https://pdx.tools) - use their Rakaly CLI\n\
            - PDX-Unlimiter can extract from eu4.exe\n\
            - Our eu4tokens tool extracts strings but not IDs (WIP)\n\n\
            Note: Token IDs change between game versions. Use tokens\n\
            matching your save file version."
        );
    }

    // Set environment variable if we found a local tokens file
    if let Some(path) = tokens_path {
        if std::env::var("EU4_IRONMAN_TOKENS").is_err() {
            log::info!("Using tokens from: {}", path.display());
            std::env::set_var("EU4_IRONMAN_TOKENS", &path);
        }
    }

    log::info!("Parsing binary save with eu4save...");

    // Parse the save file
    let file = Eu4File::from_slice(data).context("Failed to parse EU4 save file")?;

    // Try deserialization with token resolver
    let save = match file.deserializer().build_save(&EnvTokens) {
        Ok(save) => save,
        Err(e) => {
            // Provide helpful error for token mismatch
            let msg = format!("{}", e);
            if msg.contains("missing field") || msg.contains("unknown token") {
                anyhow::bail!(
                    "Token file doesn't match save version.\n\n\
                    Error: {}\n\n\
                    The token file may be from a different game version.\n\
                    Save version can be checked with: unzip -p save.eu4 meta | xxd | head\n\
                    (Look for version string like 1.37.x.x)",
                    msg
                );
            }
            return Err(e).context("Failed to deserialize binary save");
        }
    };

    // Extract metadata
    let meta = SaveMeta {
        date: save.meta.date.iso_8601().to_string(),
        player: Some(save.meta.player.to_string()),
        ironman: save.meta.is_ironman,
        save_version: Some(format!(
            "{}.{}.{}.{}",
            save.meta.savegame_version.first,
            save.meta.savegame_version.second,
            save.meta.savegame_version.third,
            save.meta.savegame_version.fourth
        )),
    };

    log::info!(
        "Save date: {}, player: {:?}, version: {:?}",
        meta.date,
        meta.player,
        meta.save_version
    );

    // Extract countries and provinces using query API
    let query = eu4save::query::Query::from_save(save);
    let mut countries = HashMap::new();
    let mut provinces = HashMap::new();

    // First pass: extract provinces and build owner mapping
    for (&id, province) in query.save().game.provinces.iter() {
        let id_u32 = id.as_u16() as u32;

        // Extract buildings from province (HashMap<String, bool> - key is building name, value is true if present)
        let buildings: Vec<String> = province
            .buildings
            .iter()
            .filter_map(|(name, &present)| if present { Some(name.clone()) } else { None })
            .collect();

        let extracted = crate::ExtractedProvince {
            id: id_u32,
            name: Some(province.name.clone()),
            owner: province.owner.as_ref().map(|t| t.to_string()),
            base_tax: Some(province.base_tax.into()),
            base_production: Some(province.base_production.into()),
            base_manpower: Some(province.base_manpower.into()),
            institutions: HashMap::new(), // TODO: Extract institution progress
            local_autonomy: Some(province.local_autonomy.into()),
            buildings,
            trade_good: province.trade_goods.clone(),
        };

        provinces.insert(id_u32, extracted);
    }

    // Build owned provinces map from province owners
    let mut owned_provinces_map: HashMap<String, Vec<u32>> = HashMap::new();
    for (id, prov) in &provinces {
        if let Some(owner) = &prov.owner {
            owned_provinces_map
                .entry(owner.clone())
                .or_default()
                .push(*id);
        }
    }

    // Extract country data
    for (tag, country) in query.save().game.countries.iter() {
        let tag_str = tag.to_string();
        let owned = owned_provinces_map.remove(&tag_str).unwrap_or_default();

        // Extract advisor info from expense ledger
        // Binary saves don't expose individual advisors, but we can get total monthly cost
        let advisors = extract_advisors_from_ledger(&query, country);

        // Note: eu4save doesn't expose mana points directly, only in ledger data
        // For binary saves, we skip mana extraction

        // Extract ideas from active_idea_groups
        let ideas = extract_ideas_from_binary(country);

        // Extract income and expense breakdown from ledger
        let income_ledger = query.country_income_breakdown(country);
        let expense_ledger = query.country_expense_breakdown(country);

        let total_income = income_ledger.taxation
            + income_ledger.production
            + income_ledger.trade
            + income_ledger.gold
            + income_ledger.tariffs
            + income_ledger.subsidies
            + income_ledger.vassals
            + income_ledger.harbor_fees
            + income_ledger.war_reparations
            + income_ledger.interest
            + income_ledger.gifts
            + income_ledger.events
            + income_ledger.spoils_of_war
            + income_ledger.treasure_fleet
            + income_ledger.siphoning_income
            + income_ledger.condottieri
            + income_ledger.knowledge_sharing
            + income_ledger.blockading_foreign_ports
            + income_ledger.looting_foreign_cities
            + income_ledger.other;

        let monthly_income = Some(crate::MonthlyIncome {
            tax: income_ledger.taxation as f64,
            production: income_ledger.production as f64,
            trade: income_ledger.trade as f64,
            gold: income_ledger.gold as f64,
            tariffs: income_ledger.tariffs as f64,
            subsidies: income_ledger.subsidies as f64,
            total: total_income as f64,
        });

        // Extract ruler stats from history
        let monarch = extract_ruler_stats(country);

        let extracted = crate::ExtractedCountry {
            tag: tag_str.clone(),
            max_manpower: Some(country.max_manpower.into()),
            current_manpower: Some(country.manpower.into()),
            treasury: Some(country.treasury.into()),
            adm_power: None, // Not directly available in eu4save Country struct
            dip_power: None,
            mil_power: None,
            ruler_adm: monarch.adm,
            ruler_dip: monarch.dip,
            ruler_mil: monarch.mil,
            ruler_dynasty: monarch.dynasty,
            tribute_type: country.tribute_type,
            monthly_income,
            total_monthly_expenses: None, // Will be calculated from ledger array in text parse
            army_maintenance: Some(expense_ledger.army_maintenance as f64),
            navy_maintenance: Some(expense_ledger.fleet_maintenance as f64),
            fort_maintenance: Some(expense_ledger.fort_maintenance as f64),
            state_maintenance: None, // TODO: Extract when ledger has detailed breakdown
            root_out_corruption: None, // TODO: Extract when ledger has detailed breakdown
            advisors,
            ideas,
            active_modifiers: vec![], // TODO: Extract from eu4save when deserialization works
            owned_province_ids: owned,
        };

        countries.insert(tag_str, extracted);
    }

    // Extract subject relationships from diplomacy.dependencies
    let mut subjects = HashMap::new();
    for dep in &query.save().game.diplomacy.dependencies {
        let subject = crate::ExtractedSubject {
            overlord: dep.first.to_string(),
            subject: dep.second.to_string(),
            subject_type: dep.subject_type.clone(),
            start_date: dep.start_date.map(|d| d.iso_8601().to_string()),
        };
        subjects.insert(subject.subject.clone(), subject);
    }

    log::info!(
        "Extracted {} countries, {} provinces, {} subjects",
        countries.len(),
        provinces.len(),
        subjects.len()
    );

    // Extract celestial empire data from query (if available)
    let celestial_empire = extract_celestial_empire_from_query(&query);

    Ok(ExtractedState {
        meta,
        countries,
        provinces,
        subjects,
        celestial_empire,
        trade_nodes: HashMap::new(), // TODO: Extract from Query API
    })
}

/// Extract celestial empire data from eu4save query
///
/// Note: eu4save library doesn't expose celestial_empire data directly,
/// so this function returns None. Celestial empire data is extracted
/// via text parsing instead.
fn extract_celestial_empire_from_query(
    _query: &eu4save::query::Query,
) -> Option<crate::ExtractedCelestialEmpire> {
    // eu4save doesn't expose celestial_empire in GameState
    // We'll rely on text parsing for this data
    None
}

/// Extract ideas from binary save country data
fn extract_ideas_from_binary(country: &eu4save::models::Country) -> crate::ExtractedIdeas {
    let mut ideas = crate::ExtractedIdeas::default();

    // eu4save exposes active_idea_groups as Vec<(String, u8)>
    for (group_name, ideas_unlocked) in &country.active_idea_groups {
        // Detect if this is a national idea (TAG_ideas pattern)
        let is_national = group_name.len() >= 6 && group_name.ends_with("_ideas") && {
            let prefix = &group_name[..group_name.len() - 6];
            prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase())
        };

        if is_national {
            ideas.national_ideas = Some(group_name.clone());
            ideas.national_ideas_progress = *ideas_unlocked;
        } else {
            ideas.idea_groups.push(crate::ExtractedIdeaGroup {
                name: group_name.clone(),
                ideas_unlocked: *ideas_unlocked,
            });
        }
    }

    ideas
}

/// Extract advisor information from expense ledger
///
/// Binary saves don't expose individual advisors through eu4save, but we can get
/// the total monthly advisor maintenance cost from the expense ledger. We create
/// stub advisors to represent this expense for simulation purposes.
fn extract_advisors_from_ledger(
    query: &eu4save::query::Query,
    country: &eu4save::models::Country,
) -> Vec<crate::ExtractedAdvisor> {
    let expense = query.country_expense_breakdown(country);
    let monthly_cost = expense.advisor_maintenance;

    log::trace!(
        "Advisor maintenance from ledger: {:.2} ducats/month",
        monthly_cost
    );

    if monthly_cost <= 0.0 {
        return Vec::new();
    }

    // Create stub advisors to match the total monthly cost
    // EU4 advisor cost formula: base_cost × skill²
    // We'll assume skill-1 advisors (simplest case) with base_cost = 5.0
    // So each advisor costs 5.0 × 1² = 5.0 ducats/month
    //
    // This is a simplification since we don't know actual advisor skills,
    // but it preserves the correct total expense for treasury calculation.
    let base_cost_per_advisor = 5.0;
    let num_advisors = (monthly_cost / base_cost_per_advisor).round() as usize;

    log::debug!(
        "Creating {} stub advisors for {:.2} ducats/month maintenance",
        num_advisors,
        monthly_cost
    );

    // Create the advisors - distribute types evenly across ADM/DIP/MIL
    let advisor_types = ["philosopher", "statesman", "army_reformer"];
    (0..num_advisors)
        .map(|i| crate::ExtractedAdvisor {
            advisor_type: advisor_types[i % 3].to_string(),
            skill: 1, // Assume skill 1 since base_cost × 1² = 5.0
            is_hired: true,
        })
        .collect()
}

/// Extracted monarch information from save file.
#[derive(Debug, Default)]
pub struct ExtractedMonarch {
    pub adm: Option<u16>,
    pub dip: Option<u16>,
    pub mil: Option<u16>,
    pub dynasty: Option<String>,
}

/// Extract the current ruler's stats and dynasty from country history.
///
/// Iterates through history events to find the most recent monarch,
/// then returns their stats (0-6 for each) and dynasty. Returns defaults if no monarch found.
fn extract_ruler_stats(country: &eu4save::models::Country) -> ExtractedMonarch {
    // Find the most recent monarch in history
    // History events are in chronological order, so we iterate and take the last monarch
    let mut last_monarch: Option<&eu4save::models::Monarch> = None;

    log::trace!(
        "Checking country history events: {} entries",
        country.history.events.len()
    );

    for (date, events) in &country.history.events {
        for event in &events.0 {
            if let Some(monarch) = event.as_monarch() {
                log::trace!(
                    "Found monarch in history at {:?}: {} (ADM={}, DIP={}, MIL={})",
                    date,
                    monarch.name,
                    monarch.adm,
                    monarch.dip,
                    monarch.mil
                );
                last_monarch = Some(monarch);
            }
        }
    }

    if let Some(monarch) = last_monarch {
        log::debug!(
            "Current ruler: {} (ADM={}, DIP={}, MIL={})",
            monarch.name,
            monarch.adm,
            monarch.dip,
            monarch.mil
        );
        // Note: eu4save::models::Monarch doesn't expose dynasty field directly
        // Dynasty extraction requires text parsing or a dynasties lookup table
        ExtractedMonarch {
            adm: Some(monarch.adm),
            dip: Some(monarch.dip),
            mil: Some(monarch.mil),
            dynasty: None,
        }
    } else {
        log::trace!("No monarch found in history events");
        ExtractedMonarch::default()
    }
}

/// Find tokens file in standard locations
fn find_tokens_file() -> Option<std::path::PathBuf> {
    // Check relative to current directory
    let candidates = [
        "assets/tokens/eu4.txt",
        "../assets/tokens/eu4.txt",
        "eu4.tokens.txt",
    ];

    for candidate in candidates {
        let path = std::path::PathBuf::from(candidate);
        if path.exists() {
            return path.canonicalize().ok();
        }
    }

    // Check relative to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join("assets/tokens/eu4.txt");
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

fn parse_text_gamestate(data: &[u8]) -> Result<ExtractedState> {
    // Try to use eu4save Query API for ledger data extraction
    // This works for both binary and text (melted) formats
    use eu4save::{EnvTokens, Eu4File};

    match Eu4File::from_slice(data) {
        Ok(file) => {
            log::debug!("Successfully parsed text save with eu4save, attempting deserialization");
            // Deserialize the file to get Eu4Save
            // For text saves, token resolution is not needed but we still pass EnvTokens
            match file.deserializer().build_save(&EnvTokens) {
                Ok(save) => {
                    log::debug!("Deserialized successfully, using Query API for ledger data");
                    let query = eu4save::query::Query::from_save(save);
                    parse_with_query(query)
                }
                Err(e) => {
                    log::warn!(
                        "Failed to deserialize text save: {}, falling back to regex parsing",
                        e
                    );
                    // Fall back to regex-based text parsing
                    let content = if data.starts_with(b"EU4txt") {
                        &data[6..]
                    } else {
                        data
                    };
                    let text = String::from_utf8_lossy(content);
                    parse_text_content(&text)
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to parse text save with eu4save Query API: {}, falling back to regex parsing (no ledger data)", e);
            // Fall back to regex-based text parsing (no ledger data)
            let content = if data.starts_with(b"EU4txt") {
                &data[6..]
            } else {
                data
            };
            let text = String::from_utf8_lossy(content);
            parse_text_content(&text)
        }
    }
}

/// Parse save using eu4save Query API (works for binary and melted text)
fn parse_with_query(query: eu4save::query::Query) -> Result<ExtractedState> {
    use eu4save::PdsDate;
    use std::collections::HashMap;

    // Extract meta from save
    let meta_date = query.save().meta.date.iso_8601().to_string();
    let meta_player = Some(query.save().meta.player.to_string());
    let meta_version = format!(
        "{}.{}.{}.{}",
        query.save().meta.savegame_version.first,
        query.save().meta.savegame_version.second,
        query.save().meta.savegame_version.third,
        query.save().meta.savegame_version.fourth
    );

    let save_meta = SaveMeta {
        date: meta_date,
        player: meta_player,
        ironman: query.save().meta.is_ironman,
        save_version: Some(meta_version),
    };

    let mut countries = HashMap::new();
    let mut provinces = HashMap::new();

    // Build owned provinces map from province owners
    let mut owned_provinces_map: HashMap<String, Vec<u32>> = HashMap::new();
    for (&id, province) in query.save().game.provinces.iter() {
        let id_u32 = id.as_u16() as u32;
        let owner_str = province.owner.as_ref().map(|t| t.to_string());

        if let Some(owner) = &owner_str {
            owned_provinces_map
                .entry(owner.clone())
                .or_default()
                .push(id_u32);
        }

        let buildings: Vec<String> = province
            .buildings
            .iter()
            .filter_map(|(name, &present)| if present { Some(name.clone()) } else { None })
            .collect();

        let extracted = crate::ExtractedProvince {
            id: id_u32,
            name: Some(province.name.clone()),
            owner: owner_str,
            base_tax: Some(province.base_tax.into()),
            base_production: Some(province.base_production.into()),
            base_manpower: Some(province.base_manpower.into()),
            institutions: HashMap::new(),
            local_autonomy: Some(province.local_autonomy.into()),
            buildings,
            trade_good: province.trade_goods.clone(),
        };

        provinces.insert(id_u32, extracted);
    }

    // Extract country data with ledger information
    for (tag, country) in query.save().game.countries.iter() {
        let tag_str = tag.to_string();
        let owned = owned_provinces_map.remove(&tag_str).unwrap_or_default();

        let advisors = extract_advisors_from_ledger(&query, country);
        let ideas = extract_ideas_from_binary(country);

        // Extract ledger data
        let income_ledger = query.country_income_breakdown(country);
        let expense_ledger = query.country_expense_breakdown(country);

        let total_income = income_ledger.taxation
            + income_ledger.production
            + income_ledger.trade
            + income_ledger.gold
            + income_ledger.tariffs
            + income_ledger.subsidies
            + income_ledger.vassals
            + income_ledger.harbor_fees
            + income_ledger.war_reparations
            + income_ledger.interest
            + income_ledger.gifts
            + income_ledger.events
            + income_ledger.spoils_of_war
            + income_ledger.treasure_fleet
            + income_ledger.siphoning_income
            + income_ledger.condottieri
            + income_ledger.knowledge_sharing
            + income_ledger.blockading_foreign_ports
            + income_ledger.looting_foreign_cities
            + income_ledger.other;

        let monthly_income = Some(crate::MonthlyIncome {
            tax: income_ledger.taxation as f64,
            production: income_ledger.production as f64,
            trade: income_ledger.trade as f64,
            gold: income_ledger.gold as f64,
            tariffs: income_ledger.tariffs as f64,
            subsidies: income_ledger.subsidies as f64,
            total: total_income as f64,
        });

        // Extract ruler stats from history
        let monarch = extract_ruler_stats(country);

        let extracted = crate::ExtractedCountry {
            tag: tag_str.clone(),
            max_manpower: Some(country.max_manpower.into()),
            current_manpower: Some(country.manpower.into()),
            treasury: Some(country.treasury.into()),
            adm_power: None,
            dip_power: None,
            mil_power: None,
            ruler_adm: monarch.adm,
            ruler_dip: monarch.dip,
            ruler_mil: monarch.mil,
            ruler_dynasty: monarch.dynasty,
            tribute_type: country.tribute_type,
            monthly_income,
            total_monthly_expenses: None, // Will be calculated from ledger array in text parse
            army_maintenance: Some(expense_ledger.army_maintenance as f64),
            navy_maintenance: Some(expense_ledger.fleet_maintenance as f64),
            fort_maintenance: Some(expense_ledger.fort_maintenance as f64),
            state_maintenance: None, // TODO: Extract when ledger has detailed breakdown
            root_out_corruption: None, // TODO: Extract when ledger has detailed breakdown
            advisors,
            ideas,
            active_modifiers: vec![], // TODO: Extract from eu4save when deserialization works
            owned_province_ids: owned,
        };

        countries.insert(tag_str, extracted);
    }

    // Extract subjects
    let mut subjects = HashMap::new();
    for dep in &query.save().game.diplomacy.dependencies {
        let subject_tag = dep.second.to_string();
        subjects.insert(
            subject_tag.clone(),
            crate::ExtractedSubject {
                overlord: dep.first.to_string(),
                subject: subject_tag,
                subject_type: dep.subject_type.clone(),
                start_date: dep.start_date.as_ref().map(|d| d.iso_8601().to_string()),
            },
        );
    }

    // Extract celestial empire data
    let celestial_empire = extract_celestial_empire_from_query(&query);

    Ok(ExtractedState {
        meta: save_meta,
        countries,
        provinces,
        subjects,
        celestial_empire,
        trade_nodes: HashMap::new(), // TODO: Extract from binary save
    })
}

/// Parse text content (shared between text saves and melted binary)
fn parse_text_content(text: &str) -> Result<ExtractedState> {
    log::info!("Parsing text gamestate ({} chars)", text.len());

    // Basic extraction - look for key patterns
    let mut state = ExtractedState {
        meta: SaveMeta {
            date: extract_date(text).unwrap_or_else(|| "unknown".to_string()),
            player: extract_player(text),
            ironman: false,
            save_version: extract_save_version(text),
        },
        countries: HashMap::new(),
        provinces: HashMap::new(),
        subjects: HashMap::new(),
        celestial_empire: None,
        trade_nodes: HashMap::new(),
    };

    // Extract country data
    extract_countries(text, &mut state)?;

    // Extract province data
    extract_provinces(text, &mut state)?;

    // Extract subject relationships from diplomacy section
    extract_subjects(text, &mut state)?;

    // Extract celestial empire from text
    state.celestial_empire = extract_celestial_empire_from_text(text);

    // Extract trade node data
    extract_trade_nodes(text, &mut state);

    log::info!(
        "Extracted {} countries, {} provinces, {} subjects, {} trade nodes",
        state.countries.len(),
        state.provinces.len(),
        state.subjects.len(),
        state.trade_nodes.len()
    );

    Ok(state)
}

fn extract_date(text: &str) -> Option<String> {
    // Look for date=YYYY.M.D pattern
    let re = regex::Regex::new(r"date=(\d+\.\d+\.\d+)").ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_player(text: &str) -> Option<String> {
    // Look for player="TAG" pattern
    let re = regex::Regex::new(r#"player="([A-Z]{3})""#).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_save_version(text: &str) -> Option<String> {
    // Look for save_game_version="X.Y.Z" pattern
    let re = regex::Regex::new(r#"save_game_version="([^"]+)""#).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn extract_countries(text: &str, state: &mut ExtractedState) -> Result<()> {
    // Find the countries section
    let countries_start = text.find("\ncountries={");
    if countries_start.is_none() {
        log::warn!("Could not find countries section");
        return Ok(());
    }

    // Find the matching closing brace for the countries section
    let section_start = countries_start.unwrap() + "\ncountries={".len();
    let countries_section = if let Some(section_content) = extract_block(&text[section_start..]) {
        section_content
    } else {
        log::warn!("Could not find end of countries section");
        return Ok(());
    };

    log::info!(
        "Found countries section at offset {} ({} chars)",
        countries_start.unwrap(),
        countries_section.len()
    );

    // Find country blocks: \n\tTAG={
    let tag_pattern =
        regex::Regex::new(r"\n\t([A-Z]{3})=\{").context("Failed to compile tag regex")?;

    for cap in tag_pattern.captures_iter(countries_section) {
        let tag = cap.get(1).map(|m| m.as_str().to_string()).unwrap();
        let match_start = cap.get(0).unwrap().start();

        // Find the country block content (everything until the matching closing brace)
        let block_start = match_start + cap.get(0).unwrap().len();
        if let Some(block_content) = extract_block(&countries_section[block_start..]) {
            let country = parse_country_block(&tag, block_content);
            if country.treasury.is_some() || country.current_manpower.is_some() {
                log::debug!(
                    "Extracted {}: treasury={:?}, manpower={:?}, max_manpower={:?}",
                    tag,
                    country.treasury,
                    country.current_manpower,
                    country.max_manpower
                );
            }
            state.countries.insert(tag, country);
        }
    }

    log::info!("Extracted {} countries", state.countries.len());
    Ok(())
}

/// Extract content inside braces, handling nested braces
fn extract_block(text: &str) -> Option<&str> {
    let mut depth = 1;
    let mut end = 0;

    for (i, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if end > 0 {
        Some(&text[..end])
    } else {
        None
    }
}

/// Parse ledger income/expense arrays from country block.
///
/// Returns (income_array, expense_array) where indices map to specific categories.
///
/// Income indices (19 total):
/// 0: Taxation, 1: Production, 2: Trade, 3: Gold, 4: Tariffs, 5: Vassals,
/// 6: Harbor Fees, 7: Subsidies, 8: War Reparations, 9: Interest, 10: Gifts,
/// 11: Events, 12: Spoils of War, 13: Treasure Fleet, 14: Siphoning Income,
/// 15: Condottieri, 16: Knowledge Sharing, 17: Blockading Foreign Ports, 18: Looting
///
/// Expense indices (38 total):
/// 0: Advisor, 1: Interest, 2: State maintenance, 3: Subsidies, 4: War reparations,
/// 5: Army recruitiment, 6: Army maintenance, 7: Fleet maintenance, 8: Fort maintenance,
/// 9-37: Various other expenses
fn parse_ledger_arrays(block: &str) -> Option<(Vec<f64>, Vec<f64>)> {
    // Look for lastmonthincometable and lastmonthexpensetable
    // These span multiple lines, so we need to match across newlines
    let income_re = regex::Regex::new(r"(?s)lastmonthincometable=\{([^}]+)\}").ok()?;
    let expense_re = regex::Regex::new(r"(?s)lastmonthexpensetable=\{([^}]+)\}").ok()?;

    log::trace!("Looking for ledger data in block of length {}", block.len());
    let income_cap = income_re.captures(block);
    let expense_cap = expense_re.captures(block);

    if income_cap.is_none() {
        log::trace!("No income ledger found");
        return None;
    }
    if expense_cap.is_none() {
        log::trace!("No expense ledger found");
        return None;
    }

    let income_cap = income_cap?;
    let expense_cap = expense_cap?;

    let income_str = income_cap.get(1)?.as_str();
    let expense_str = expense_cap.get(1)?.as_str();

    let income: Vec<f64> = income_str
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    let expense: Vec<f64> = expense_str
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    Some((income, expense))
}

/// Parse a country block to extract key values
fn parse_country_block(tag: &str, content: &str) -> crate::ExtractedCountry {
    let mut country = crate::ExtractedCountry {
        tag: tag.to_string(),
        ..Default::default()
    };

    // Extract key numeric fields
    country.treasury = extract_float_value(content, "treasury=");
    country.current_manpower = extract_float_value(content, "manpower=");
    country.max_manpower = extract_float_value(content, "max_manpower=");

    // Extract monarch power points from powers={ADM DIP MIL} array
    // This is stored as an array of 3 integers: powers={\n\t\tADM DIP MIL\n\t}
    if let Some((adm, dip, mil)) = extract_powers_array(content) {
        country.adm_power = Some(adm);
        country.dip_power = Some(dip);
        country.mil_power = Some(mil);
    }

    // Extract tribute type (for tributary states)
    country.tribute_type = extract_int_value(content, "tribute_type=");

    // Extract advisors
    country.advisors = extract_advisors(content, tag);

    // Extract ruler stats from history section
    if let Some((adm, dip, mil)) = extract_ruler_stats_from_text(content) {
        country.ruler_adm = Some(adm);
        country.ruler_dip = Some(dip);
        country.ruler_mil = Some(mil);
        log::debug!(
            "{} ruler stats from text: ADM={}, DIP={}, MIL={}",
            tag,
            adm,
            dip,
            mil
        );
    } else if tag == "KOR" {
        // Debug: check if we can find monarch= at all
        let monarch_count = content.matches("monarch={").count();
        log::debug!(
            "KOR: No ruler stats found. monarch={{ count={}, content_len={}",
            monarch_count,
            content.len()
        );
    }

    // Extract ideas
    country.ideas = extract_ideas(content, tag);

    // Extract active modifiers
    country.active_modifiers = extract_country_modifiers(content);
    if tag == "KOR" {
        log::info!("KOR modifiers extracted: {:?}", country.active_modifiers);
    }

    // Extract ledger data (income/expense arrays)
    log::trace!("{}: Attempting to parse ledger arrays", tag);
    if let Some((income_array, expense_array)) = parse_ledger_arrays(content) {
        // Map array indices to MonthlyIncome fields
        // Based on EU4's ledger format (19 income categories):
        // 0: Taxation, 1: Production, 2: Trade, 3: Gold, 4: Tariffs,
        // 5: Vassals, 6: Harbor Fees, 7: Subsidies, 8: War Reparations,
        // 9: Interest, 10: Gifts, 11: Events, 12: Spoils of War,
        // 13: Treasure Fleet, 14: Siphoning Income, 15: Condottieri,
        // 16: Knowledge Sharing, 17: Blockading Foreign Ports, 18: Looting
        if income_array.len() >= 7 {
            country.monthly_income = Some(crate::MonthlyIncome {
                tax: income_array[0],
                production: income_array[1],
                trade: income_array[2],
                gold: income_array[3],
                tariffs: income_array[4],
                subsidies: income_array[7], // Subsidies is at index 7
                total: income_array.iter().sum(),
            });

            // Log ALL non-zero income categories for MNG (to see tribute income)
            if tag == "MNG" {
                log::debug!("{} ALL income categories:", tag);
                for (i, &val) in income_array.iter().enumerate() {
                    if val.abs() > 0.01 {
                        let category = match i {
                            0 => "Taxation",
                            1 => "Production",
                            2 => "Trade",
                            3 => "Gold",
                            4 => "Tariffs",
                            5 => "Vassals/Tribute",
                            6 => "Harbor Fees",
                            7 => "Subsidies",
                            8 => "War Reparations",
                            9 => "Interest",
                            10 => "Gifts",
                            11 => "Events",
                            _ => "Other",
                        };
                        log::debug!("  [{}] {}: {:.2}", i, category, val);
                    }
                }
            }

            log::debug!(
                "{} ledger parsed: tax={:.2}, prod={:.2}, trade={:.2}, gold={:.2}, tariffs={:.2}, subsidies={:.2}, total={:.2}",
                tag,
                income_array[0],
                income_array[1],
                income_array[2],
                income_array[3],
                income_array[4],
                income_array[7],
                income_array.iter().sum::<f64>()
            );
        } else {
            log::warn!(
                "{} ledger income array too short: expected 19, got {}",
                tag,
                income_array.len()
            );
        }

        // Extract expense data
        // Expense array indices: 2 = State maintenance, 6 = Army maintenance, 7 = Fleet maintenance, 8 = Fort maintenance, 27 = Root out corruption
        if expense_array.len() >= 9 {
            country.state_maintenance = Some(expense_array[2]);
            country.army_maintenance = Some(expense_array[6]);
            country.navy_maintenance = Some(expense_array[7]);
            country.fort_maintenance = Some(expense_array[8]);

            // Extract corruption if array is long enough (index 27)
            if expense_array.len() > 27 {
                country.root_out_corruption = Some(expense_array[27]);
            }

            // Calculate total expenses from array
            let total_expenses: f64 = expense_array.iter().sum();
            country.total_monthly_expenses = Some(total_expenses);

            // Log ALL non-zero expenses for debugging (especially for KOR)
            if tag == "KOR" {
                log::debug!("{} ALL expense categories:", tag);
                for (i, &val) in expense_array.iter().enumerate() {
                    if val.abs() > 0.01 {
                        let category = match i {
                            0 => "Advisor",
                            1 => "Interest",
                            2 => "State maintenance",
                            3 => "Subsidies (outgoing)",
                            4 => "War reparations",
                            5 => "Army maintenance",
                            6 => "Fleet maintenance",
                            7 => "Fort maintenance",
                            8 => "Colonists",
                            9 => "Missionaries",
                            10 => "Raising armies",
                            11 => "Building fleets",
                            12 => "Building fortresses",
                            13 => "Buildings",
                            14 => "Repaid loans",
                            15 => "Gifts",
                            16 => "Advisors (purchase)",
                            17 => "Events",
                            18 => "Peace",
                            19 => "Vassal/Tribute fee",
                            20 => "Tariffs",
                            21 => "Support loyalists",
                            22 => "Condottieri",
                            23 => "Root out corruption",
                            24 => "Embrace institution",
                            25 => "Knowledge sharing",
                            26 => "Trade company investments",
                            27 => "Other",
                            _ => "Unknown",
                        };
                        log::debug!("  [{}] {}: {:.2}", i, category, val);
                    }
                }
            }

            log::debug!(
                "{} expenses from ledger: state={:.2}, army={:.2}, fleet={:.2}, fort={:.2}, corruption={:.2}, total={:.2}",
                tag,
                expense_array[2],
                expense_array[6],
                expense_array[7],
                expense_array[8],
                expense_array.get(27).copied().unwrap_or(0.0),
                total_expenses
            );
        } else {
            log::warn!(
                "{} ledger expense array too short: expected 38, got {}",
                tag,
                expense_array.len()
            );
        }
    }

    country
}

/// Extract advisors from country content
/// Advisors appear in an "advisors={" block with entries like:
/// { id=123 type="philosopher" skill=2 ... }
fn extract_advisors(content: &str, tag: &str) -> Vec<crate::ExtractedAdvisor> {
    let mut advisors = Vec::new();

    // Find the active_advisors block which contains currently hired advisors
    // Format: active_advisors={ advisor_id_1 advisor_id_2 advisor_id_3 }
    let active_ids: std::collections::HashSet<String> =
        if let Some(active_start) = content.find("active_advisors={") {
            let block_start = active_start + "active_advisors={".len();
            if let Some(block) = extract_block(&content[block_start..]) {
                // Parse the IDs from the block
                block
                    .split_whitespace()
                    .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
                    .map(|s| s.to_string())
                    .collect()
            } else {
                std::collections::HashSet::new()
            }
        } else {
            std::collections::HashSet::new()
        };

    // Find the advisors block
    if let Some(advisors_start) = content.find("\n\tadvisors={") {
        let block_start = advisors_start + "\n\tadvisors={".len();
        if let Some(advisors_block) = extract_block(&content[block_start..]) {
            // Parse individual advisor entries
            // Each advisor is in a block like: { id=123 type="philosopher" skill=2 ... }
            let advisor_pattern = regex::Regex::new(r"\{[^}]+\}").unwrap();
            let id_pattern = regex::Regex::new(r"id=(\d+)").unwrap();
            let type_pattern = regex::Regex::new(r#"type="([^"]+)""#).unwrap();
            let skill_pattern = regex::Regex::new(r"skill=(\d+)").unwrap();

            for cap in advisor_pattern.find_iter(advisors_block) {
                let advisor_content = cap.as_str();

                // Extract advisor ID
                let id = id_pattern
                    .captures(advisor_content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string());

                // Extract type
                let advisor_type = type_pattern
                    .captures(advisor_content)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string());

                // Extract skill
                let skill = skill_pattern
                    .captures(advisor_content)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<u8>().ok())
                    .unwrap_or(1);

                if let Some(advisor_type) = advisor_type {
                    let is_hired = id
                        .as_ref()
                        .map(|id| active_ids.contains(id))
                        .unwrap_or(false);

                    advisors.push(crate::ExtractedAdvisor {
                        advisor_type,
                        skill,
                        is_hired,
                    });
                }
            }
        }
    }

    if !advisors.is_empty() {
        log::debug!(
            "{} has {} advisors ({} hired)",
            tag,
            advisors.len(),
            advisors.iter().filter(|a| a.is_hired).count()
        );
    }

    advisors
}

/// Extract idea groups from country content
/// Ideas appear in an "active_idea_groups={" block with entries like:
/// aristocracy_ideas=7
/// diplomatic_ideas=3
fn extract_ideas(content: &str, tag: &str) -> crate::ExtractedIdeas {
    let mut ideas = crate::ExtractedIdeas::default();

    // Find active_idea_groups block
    if let Some(ideas_start) = content.find("active_idea_groups={") {
        let block_start = ideas_start + "active_idea_groups={".len();
        if let Some(ideas_block) = extract_block(&content[block_start..]) {
            // Parse idea group entries: group_name=count
            let entry_pattern = regex::Regex::new(r"(\w+_ideas)=(\d+)").unwrap();

            for cap in entry_pattern.captures_iter(ideas_block) {
                let group_name = cap.get(1).map(|m| m.as_str().to_string()).unwrap();
                let ideas_unlocked: u8 = cap
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0);

                // Detect if this is a national idea (TAG_ideas pattern)
                let is_national = group_name.len() >= 6 && group_name.ends_with("_ideas") && {
                    let prefix = &group_name[..group_name.len() - 6];
                    prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase())
                };

                if is_national {
                    ideas.national_ideas = Some(group_name);
                    ideas.national_ideas_progress = ideas_unlocked;
                } else {
                    ideas.idea_groups.push(crate::ExtractedIdeaGroup {
                        name: group_name,
                        ideas_unlocked,
                    });
                }
            }
        }
    }

    if ideas.national_ideas.is_some() || !ideas.idea_groups.is_empty() {
        log::debug!(
            "{} has {} idea groups, national: {:?} ({}/7)",
            tag,
            ideas.idea_groups.len(),
            ideas.national_ideas,
            ideas.national_ideas_progress
        );
    }

    ideas
}

/// Extract active country modifiers from save file content
///
/// Modifiers are stored in blocks like:
/// ```text
/// modifier={
///     modifier="tripitaka_koreana"
///     date=-1.1.1
///     permanent=yes
/// }
/// ```
fn extract_country_modifiers(content: &str) -> Vec<String> {
    let mut modifiers = Vec::new();

    // Regex to match modifier blocks and extract the modifier name
    let re = regex::Regex::new(r#"modifier=\{\s+modifier="([^"]+)""#).ok();
    if let Some(re) = re {
        for cap in re.captures_iter(content) {
            if let Some(modifier_name) = cap.get(1) {
                modifiers.push(modifier_name.as_str().to_string());
            }
        }
    }

    if !modifiers.is_empty() {
        log::debug!(
            "Extracted {} country modifiers: {:?}",
            modifiers.len(),
            modifiers
        );
    }

    modifiers
}

/// Extract ruler ADM/DIP/MIL stats from country history section.
///
/// Monarchs appear in the history section like:
/// ```text
/// history={
///     1392.8.14={
///         monarch={
///             name="Sejong"
///             ADM=6
///             DIP=5
///             MIL=5
///         }
///     }
/// }
/// ```
///
/// We find all monarch blocks and take the last one (most recent).
fn extract_ruler_stats_from_text(content: &str) -> Option<(u16, u16, u16)> {
    // Find all monarch blocks in history that have stats
    // Country-level monarch blocks are just references (monarch={ id=... type=... })
    // History-level monarch blocks have the actual stats (monarch={ ... ADM=X ... })
    //
    // Strategy: find all monarch={ blocks, check if they contain ADM=,
    // and take the last one that does.

    let mut last_stats: Option<(u16, u16, u16)> = None;
    let mut search_from = 0;

    let adm_re = regex::Regex::new(r"ADM=(\d+)").ok()?;
    let dip_re = regex::Regex::new(r"DIP=(\d+)").ok()?;
    let mil_re = regex::Regex::new(r"MIL=(\d+)").ok()?;

    while let Some(pos) = content[search_from..].find("monarch={") {
        let actual_pos = search_from + pos;

        // Take a chunk after monarch= to check for stats (500 chars should be enough)
        // Make sure we don't slice in the middle of a UTF-8 character
        let chunk_end = (actual_pos + 500).min(content.len());
        // Find the nearest valid char boundary
        let chunk_end = content[actual_pos..]
            .char_indices()
            .take_while(|(i, _)| actual_pos + i <= chunk_end)
            .last()
            .map(|(i, c)| actual_pos + i + c.len_utf8())
            .unwrap_or(actual_pos);
        let monarch_chunk = &content[actual_pos..chunk_end];

        // Try to extract stats from this block
        let adm = adm_re
            .captures(monarch_chunk)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u16>().ok());
        let dip = dip_re
            .captures(monarch_chunk)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u16>().ok());
        let mil = mil_re
            .captures(monarch_chunk)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u16>().ok());

        // Only consider this block if it has all three stats
        if let (Some(a), Some(d), Some(m)) = (adm, dip, mil) {
            last_stats = Some((a, d, m));
        }

        search_from = actual_pos + 1;
    }

    if let Some((adm, dip, mil)) = last_stats {
        log::trace!(
            "Found ruler with stats: ADM={}, DIP={}, MIL={}",
            adm,
            dip,
            mil
        );
    }

    last_stats
}

/// Extract a float value following a pattern like "field=123.456"
fn extract_float_value(text: &str, pattern: &str) -> Option<f64> {
    // Find the pattern (must be at line start or after whitespace)
    let re =
        regex::Regex::new(&format!(r"(?:^|\s){}(-?\d+\.?\d*)", regex::escape(pattern))).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

/// Extract an integer value following a pattern like "field=123"
/// Excludes date-like values (e.g., "1445.6.1") by requiring the number
/// to be followed by whitespace or newline, not a period.
fn extract_int_value(text: &str, pattern: &str) -> Option<i32> {
    // Match integer NOT followed by a period (to avoid matching dates like 1445.6.1)
    let re = regex::Regex::new(&format!(
        r"(?:^|\s){}(-?\d+)(?:\s|$)",
        regex::escape(pattern)
    ))
    .ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

/// Extract monarch power from "powers={ ADM DIP MIL }" array format.
/// Returns (ADM, DIP, MIL) as f64 values.
fn extract_powers_array(text: &str) -> Option<(f64, f64, f64)> {
    // Find the LAST powers={ block (there can be multiple, we want the country-level one)
    // Format: "powers={\n\t\t58 155 127 \n\t\t}"
    let re = regex::Regex::new(r"(?s)powers=\{\s*(\d+)\s+(\d+)\s+(\d+)\s*\}").ok()?;

    // Find all matches and take the last one
    let mut last_match = None;
    for cap in re.captures_iter(text) {
        last_match = Some(cap);
    }

    let cap = last_match?;
    let adm: f64 = cap.get(1)?.as_str().parse().ok()?;
    let dip: f64 = cap.get(2)?.as_str().parse().ok()?;
    let mil: f64 = cap.get(3)?.as_str().parse().ok()?;

    Some((adm, dip, mil))
}

fn extract_provinces(text: &str, state: &mut ExtractedState) -> Result<()> {
    // Find the provinces section
    let provinces_start = text.find("\nprovinces={");
    if provinces_start.is_none() {
        log::warn!("Could not find provinces section");
        return Ok(());
    }

    // Find the matching closing brace for the provinces section
    let section_start = provinces_start.unwrap() + "\nprovinces={".len();
    let provinces_section = if let Some(section_content) = extract_block(&text[section_start..]) {
        section_content
    } else {
        log::warn!("Could not find end of provinces section");
        return Ok(());
    };

    log::info!(
        "Found provinces section at offset {} ({} chars)",
        provinces_start.unwrap(),
        provinces_section.len()
    );

    // Province blocks: -123={ ... }
    // Note: Province IDs in save files are negative for land provinces
    let province_pattern =
        regex::Regex::new(r"\n-(\d+)=\{").context("Failed to compile province regex")?;

    let mut count = 0;
    for cap in province_pattern.captures_iter(provinces_section) {
        let id: u32 = cap
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        let block_start = cap.get(0).unwrap().start() + cap.get(0).unwrap().len();

        // Find the province block content
        if let Some(block_content) = extract_block(&provinces_section[block_start..]) {
            let province = parse_province_block(id, block_content);

            // Update country owned provinces
            if let Some(owner_tag) = &province.owner {
                if let Some(country) = state.countries.get_mut(owner_tag) {
                    country.owned_province_ids.push(id);
                    if owner_tag == "HAB" && country.owned_province_ids.len() <= 3 {
                        log::debug!(
                            "Added province {} to HAB, now has {} provinces",
                            id,
                            country.owned_province_ids.len()
                        );
                    }
                } else {
                    log::trace!("Owner {} not found in countries", owner_tag);
                }
            }

            state.provinces.insert(id, province);
            count += 1;
        }
    }

    log::info!("Extracted {} provinces", count);

    // Log province counts for major countries
    for tag in ["HAB", "FRA", "ENG", "TUR", "POL"] {
        if let Some(country) = state.countries.get(tag) {
            log::debug!("{} has {} provinces", tag, country.owned_province_ids.len());
        }
    }

    Ok(())
}

/// Parse a province block to extract key values
fn parse_province_block(id: u32, content: &str) -> crate::ExtractedProvince {
    let mut province = crate::ExtractedProvince {
        id,
        ..Default::default()
    };

    // Extract owner - look for owner="TAG" pattern
    if let Some(caps) = regex::Regex::new(r#"owner="([A-Z]{3})""#)
        .ok()
        .and_then(|re| re.captures(content))
    {
        province.owner = caps.get(1).map(|m| m.as_str().to_string());
    }

    // Extract numeric fields
    province.base_tax = extract_float_value(content, "base_tax=");
    province.base_production = extract_float_value(content, "base_production=");
    province.base_manpower = extract_float_value(content, "base_manpower=");
    province.local_autonomy = extract_float_value(content, "local_autonomy=");

    // Extract name
    if let Some(caps) = regex::Regex::new(r#"name="([^"]+)""#)
        .ok()
        .and_then(|re| re.captures(content))
    {
        province.name = caps.get(1).map(|m| m.as_str().to_string());
    }

    // Extract buildings - look for building_name=yes patterns
    province.buildings = extract_buildings(content);

    // Extract trade goods - pattern: trade_goods="grain" or trade_goods=grain
    if let Some(caps) = regex::Regex::new(r#"trade_goods="?([a-z_]+)"?"#)
        .ok()
        .and_then(|re| re.captures(content))
    {
        province.trade_good = caps.get(1).map(|m| m.as_str().to_string());
    }

    province
}

/// Extract subject relationships from diplomacy section
/// Dependencies appear as: dependency={ first="FRA" second="PRO" subject_type="vassal" ... }
fn extract_subjects(text: &str, state: &mut ExtractedState) -> Result<()> {
    // Find the diplomacy section
    let diplomacy_start = text.find("\ndiplomacy={");
    if diplomacy_start.is_none() {
        log::debug!("Could not find diplomacy section");
        return Ok(());
    }

    let section_start = diplomacy_start.unwrap() + "\ndiplomacy={".len();
    let diplomacy_section = if let Some(section_content) = extract_block(&text[section_start..]) {
        section_content
    } else {
        log::warn!("Could not find end of diplomacy section");
        return Ok(());
    };

    log::debug!(
        "Found diplomacy section at offset {} ({} chars)",
        diplomacy_start.unwrap(),
        diplomacy_section.len()
    );

    // Find dependency blocks: dependency={ first="TAG" second="TAG" subject_type="type" ... }
    let dependency_pattern =
        regex::Regex::new(r"dependency=\{").context("Failed to compile dependency regex")?;

    let first_pattern = regex::Regex::new(r#"first="([A-Z]{3})""#).unwrap();
    let second_pattern = regex::Regex::new(r#"second="([A-Z]{3})""#).unwrap();
    let type_pattern = regex::Regex::new(r#"subject_type="([^"]+)""#).unwrap();
    let date_pattern = regex::Regex::new(r"start_date=(\d+\.\d+\.\d+)").unwrap();

    for m in dependency_pattern.find_iter(diplomacy_section) {
        let block_start = m.end();
        if let Some(block_content) = extract_block(&diplomacy_section[block_start..]) {
            // Extract first (overlord)
            let overlord = first_pattern
                .captures(block_content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            // Extract second (subject)
            let subject = second_pattern
                .captures(block_content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            // Extract subject_type
            let subject_type = type_pattern
                .captures(block_content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            // Extract start_date
            let start_date = date_pattern
                .captures(block_content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());

            if let (Some(overlord), Some(subject), Some(subject_type)) =
                (overlord, subject, subject_type)
            {
                let relationship = crate::ExtractedSubject {
                    overlord,
                    subject: subject.clone(),
                    subject_type,
                    start_date,
                };
                state.subjects.insert(subject, relationship);
            }
        }
    }

    log::info!("Extracted {} subject relationships", state.subjects.len());
    Ok(())
}

/// Extract building names from province content
/// Buildings appear as: marketplace=yes, temple=yes, etc.
fn extract_buildings(content: &str) -> Vec<String> {
    // Known building types in EU4
    static BUILDING_NAMES: &[&str] = &[
        // Tax buildings
        "temple",
        "cathedral",
        // Production buildings
        "workshop",
        "counting_house",
        // Trade buildings
        "marketplace",
        "trade_depot",
        "stock_exchange",
        // Manpower buildings
        "barracks",
        "training_fields",
        // Military buildings
        "fort_15th",
        "fort_16th",
        "fort_17th",
        "fort_18th",
        "shipyard",
        "grand_shipyard",
        "dock",
        "drydock",
        // Special buildings
        "courthouse",
        "town_hall",
        "university",
        "soldier_households",
        "impressment_offices",
        "state_house",
        "textile",
        "weapons",
        "plantations",
        "tradecompany",
        "wharf",
        "furnace",
        "ramparts",
        "soldiers_monument",
        "native_earthwork",
        "native_fortified_house",
        "native_sweat_lodge",
    ];

    let mut buildings = Vec::new();
    for &building in BUILDING_NAMES {
        let pattern = format!("{}=yes", building);
        if content.contains(&pattern) {
            buildings.push(building.to_string());
        }
    }
    buildings
}

/// Extract celestial empire data from text save
///
/// Celestial empire data appears in a block like:
/// ```text
/// celestial_empire={
///     emperor="MNG"
///     mandate=87.123
///     passed_reforms={ reform_1 reform_2 }
/// }
/// ```
fn extract_celestial_empire_from_text(text: &str) -> Option<crate::ExtractedCelestialEmpire> {
    // Find the celestial_empire section
    let ce_start = text.find("\ncelestial_empire={")?;
    let block_start = ce_start + "\ncelestial_empire={".len();
    let ce_block = extract_block(&text[block_start..])?;

    // Extract emperor tag
    let emperor_re = regex::Regex::new(r#"emperor="([A-Z]{3})""#).ok()?;
    let emperor = emperor_re
        .captures(ce_block)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    // If no emperor, celestial empire doesn't exist or is dismantled
    emperor.as_ref()?;

    // Extract mandate value
    let mandate_re = regex::Regex::new(r"mandate=(-?\d+\.?\d*)").ok()?;
    let mandate = mandate_re
        .captures(ce_block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok());

    // Extract passed reforms
    let mut reforms_passed = Vec::new();
    if let Some(reforms_start) = ce_block.find("passed_reforms={") {
        let reforms_block_start = reforms_start + "passed_reforms={".len();
        if let Some(reforms_block) = extract_block(&ce_block[reforms_block_start..]) {
            // Reform names are space-separated identifiers
            for reform_name in reforms_block.split_whitespace() {
                // Filter out non-reform strings (might have formatting)
                if reform_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    reforms_passed.push(reform_name.to_string());
                }
            }
        }
    }

    log::debug!(
        "Extracted celestial empire: emperor={:?}, mandate={:?}, reforms={}",
        emperor,
        mandate,
        reforms_passed.len()
    );

    Some(crate::ExtractedCelestialEmpire {
        emperor,
        mandate,
        dismantled: false,
        reforms_passed,
    })
}

/// Extract trade node data from text save
///
/// Trade data appears in a block like:
/// ```text
/// trade={
///     node={
///         definitions="girin"
///         current=3.481
///         local_value=4.439
///         total=332.638
///         KOR={
///             val=126.4
///             money=1.23
///             has_trader=yes
///             type=1
///         }
///     }
/// }
/// ```
fn extract_trade_nodes(text: &str, state: &mut ExtractedState) {
    use regex::Regex;

    // Find the trade section
    let trade_start = match text.find("\ntrade={") {
        Some(pos) => pos + "\ntrade={".len(),
        None => {
            log::debug!("No trade section found in save");
            return;
        }
    };

    // Get the trade section (may be very large, so limit search)
    let trade_section = &text[trade_start..];

    // Compile regex patterns
    let node_start_re = Regex::new(r"(?m)^\s*node=\{").unwrap();
    let definitions_re = Regex::new(r#"definitions="([^"]+)""#).unwrap();
    let current_re = Regex::new(r"current=(-?\d+\.?\d*)").unwrap();
    let local_value_re = Regex::new(r"local_value=(-?\d+\.?\d*)").unwrap();
    let total_re = Regex::new(r"(?m)^\s*total=(-?\d+\.?\d*)").unwrap();

    // Country data patterns
    let country_block_re = Regex::new(r"(?m)^\s*([A-Z]{3})=\{").unwrap();
    let val_re = Regex::new(r"(?m)^\s*val=(-?\d+\.?\d*)").unwrap();
    let money_re = Regex::new(r"(?m)^\s*money=(-?\d+\.?\d*)").unwrap();
    let has_trader_re = Regex::new(r"has_trader=yes").unwrap();
    let has_capital_re = Regex::new(r"has_capital=yes").unwrap();
    let type_re = Regex::new(r"(?m)^\s*type=(\d+)").unwrap();

    for node_match in node_start_re.find_iter(trade_section) {
        let block_start = node_match.end();
        let node_block = match extract_block(&trade_section[block_start..]) {
            Some(b) => b,
            None => continue,
        };

        // Extract node name
        let node_name = match definitions_re.captures(node_block) {
            Some(c) => c.get(1).unwrap().as_str().to_string(),
            None => continue,
        };

        // Extract node-level values
        let current_value = current_re
            .captures(node_block)
            .and_then(|c: regex::Captures<'_>| c.get(1))
            .and_then(|m: regex::Match<'_>| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);

        let local_value = local_value_re
            .captures(node_block)
            .and_then(|c: regex::Captures<'_>| c.get(1))
            .and_then(|m: regex::Match<'_>| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);

        let total_power = total_re
            .captures(node_block)
            .and_then(|c: regex::Captures<'_>| c.get(1))
            .and_then(|m: regex::Match<'_>| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);

        // Extract per-country data
        let mut country_data = HashMap::new();
        for country_match in country_block_re.find_iter(node_block) {
            let tag = &node_block[country_match.start()..country_match.end() - 2];
            let tag = tag.trim();

            let country_block_start = country_match.end();
            let country_block = match extract_block(&node_block[country_block_start..]) {
                Some(b) => b,
                None => continue,
            };

            // Only process if has val field (skip empty entries with just max_demand)
            let power = match val_re.captures(country_block) {
                Some(c) => c.get(1).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0),
                None => continue,
            };

            let money = money_re
                .captures(country_block)
                .and_then(|c: regex::Captures<'_>| c.get(1))
                .and_then(|m: regex::Match<'_>| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0);

            let has_trader = has_trader_re.is_match(country_block);
            let has_capital = has_capital_re.is_match(country_block);

            let action = type_re
                .captures(country_block)
                .and_then(|c: regex::Captures<'_>| c.get(1))
                .and_then(|m: regex::Match<'_>| m.as_str().parse::<u8>().ok());

            country_data.insert(
                tag.to_string(),
                crate::ExtractedCountryTradeData {
                    power,
                    money,
                    has_trader,
                    has_capital,
                    action,
                },
            );
        }

        if !country_data.is_empty() {
            log::trace!(
                "Trade node '{}': current={:.2}, power={:.2}, {} countries",
                node_name,
                current_value,
                total_power,
                country_data.len()
            );

            state.trade_nodes.insert(
                node_name.clone(),
                crate::ExtractedTradeNode {
                    name: node_name,
                    current_value,
                    local_value,
                    total_power,
                    country_data,
                },
            );
        }
    }

    log::debug!("Extracted {} trade nodes", state.trade_nodes.len());
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
