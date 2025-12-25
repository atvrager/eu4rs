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

        let extracted = crate::ExtractedProvince {
            id: id_u32,
            name: Some(province.name.clone()),
            owner: province.owner.as_ref().map(|t| t.to_string()),
            base_tax: Some(province.base_tax.into()),
            base_production: Some(province.base_production.into()),
            base_manpower: Some(province.base_manpower.into()),
            institutions: HashMap::new(), // TODO: Extract institution progress
            local_autonomy: Some(province.local_autonomy.into()),
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

        let extracted = crate::ExtractedCountry {
            tag: tag_str.clone(),
            max_manpower: Some(country.max_manpower.into()),
            current_manpower: Some(country.manpower.into()),
            treasury: Some(country.treasury.into()),
            monthly_income: None, // TODO: Extract from ledger
            army_maintenance: None,
            navy_maintenance: None,
            owned_province_ids: owned,
        };

        countries.insert(tag_str, extracted);
    }

    log::info!(
        "Extracted {} countries, {} provinces",
        countries.len(),
        provinces.len()
    );

    Ok(ExtractedState {
        meta,
        countries,
        provinces,
    })
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
    // Strip header if present
    let content = if data.starts_with(b"EU4txt") {
        &data[6..]
    } else {
        data
    };

    let text = String::from_utf8_lossy(content);
    parse_text_content(&text)
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
    };

    // Extract country data
    extract_countries(text, &mut state)?;

    // Extract province data
    extract_provinces(text, &mut state)?;

    log::info!(
        "Extracted {} countries, {} provinces",
        state.countries.len(),
        state.provinces.len()
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

    country
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

    province
}
