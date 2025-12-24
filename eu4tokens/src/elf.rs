use anyhow::{Context, Result};
use goblin::elf::Elf;

/// Extract token mappings from an ELF binary (Linux EU4)
///
/// Token format in binary:
/// - 16-bit integer IDs mapped to null-terminated strings
/// - Stored in a contiguous section (likely .rodata or similar)
/// - Pattern: sequences of valid C identifiers indexed by sequential u16 values
pub fn extract_tokens(data: &[u8]) -> Result<Vec<(u16, String)>> {
    let elf = Elf::parse(data).context("Failed to parse ELF binary")?;

    log::info!(
        "ELF binary: {} sections, {} program headers",
        elf.section_headers.len(),
        elf.program_headers.len()
    );

    // Log section names for debugging
    for (i, sh) in elf.section_headers.iter().enumerate() {
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("<unknown>");
        log::debug!(
            "Section {}: {} (offset: 0x{:x}, size: 0x{:x})",
            i,
            name,
            sh.sh_offset,
            sh.sh_size
        );
    }

    // Look for token table in various sections
    let candidate_sections = [".rodata", ".rdata", ".data", ".text"];
    let mut all_tokens = Vec::new();

    for section_name in candidate_sections {
        if let Some(tokens) = try_extract_from_section(&elf, data, section_name)? {
            log::info!(
                "Found {} candidate tokens in {}",
                tokens.len(),
                section_name
            );
            all_tokens.extend(tokens);
        }
    }

    if all_tokens.is_empty() {
        // Fallback: scan entire binary for token-like patterns
        log::warn!("No tokens found in known sections, scanning entire binary...");
        all_tokens = scan_for_tokens(data)?;
    }

    // Deduplicate and sort by ID
    all_tokens.sort_by_key(|(id, _)| *id);
    all_tokens.dedup_by_key(|(id, _)| *id);

    Ok(all_tokens)
}

fn try_extract_from_section(
    elf: &Elf,
    data: &[u8],
    section_name: &str,
) -> Result<Option<Vec<(u16, String)>>> {
    // Find section by name
    let section = elf.section_headers.iter().find(|sh| {
        elf.shdr_strtab
            .get_at(sh.sh_name)
            .map(|n| n == section_name)
            .unwrap_or(false)
    });

    let Some(section) = section else {
        return Ok(None);
    };

    let start = section.sh_offset as usize;
    let end = start + section.sh_size as usize;

    if end > data.len() {
        log::warn!("Section {} extends beyond file bounds", section_name);
        return Ok(None);
    }

    let section_data = &data[start..end];
    let tokens = find_token_table(section_data, start)?;

    if tokens.is_empty() {
        return Ok(None);
    }

    Ok(Some(tokens))
}

/// Scan section data for a token table
///
/// We're looking for a pattern of:
/// - Contiguous null-terminated strings
/// - That look like valid Clausewitz identifiers (lowercase, underscores)
/// - Indexed by sequential u16 values nearby
fn find_token_table(data: &[u8], _base_offset: usize) -> Result<Vec<(u16, String)>> {
    let mut tokens = Vec::new();

    // Strategy 1: Look for sequences of valid identifier strings
    let mut i = 0;
    let mut current_sequence = Vec::new();
    let mut sequence_start = 0;

    while i < data.len() {
        if let Some((s, len)) = try_read_identifier(&data[i..]) {
            if current_sequence.is_empty() {
                sequence_start = i;
            }
            current_sequence.push(s);
            i += len;
        } else {
            // End of sequence - check if it's a valid token table
            if current_sequence.len() >= 100 {
                log::debug!(
                    "Found identifier sequence of {} at offset 0x{:x}",
                    current_sequence.len(),
                    sequence_start
                );

                // Assign sequential IDs (this is a heuristic - real extraction
                // would need to find the index table)
                for (idx, name) in current_sequence.iter().enumerate() {
                    tokens.push((idx as u16, name.clone()));
                }
            }
            current_sequence.clear();
            i += 1;
        }
    }

    Ok(tokens)
}

/// Try to read a valid Clausewitz identifier from data
/// Returns (identifier, bytes_consumed) or None
fn try_read_identifier(data: &[u8]) -> Option<(String, usize)> {
    if data.is_empty() {
        return None;
    }

    // Find null terminator
    let end = data.iter().position(|&b| b == 0)?;

    if end == 0 || end > 64 {
        // Empty or too long
        return None;
    }

    let bytes = &data[..end];

    // Check if it's a valid identifier
    // - Must start with letter or underscore
    // - Can contain letters, digits, underscores
    // - Typically lowercase in EU4
    if !is_valid_identifier(bytes) {
        return None;
    }

    let s = String::from_utf8(bytes.to_vec()).ok()?;
    Some((s, end + 1)) // +1 for null terminator
}

fn is_valid_identifier(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    // First char: letter or underscore
    let first = bytes[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return false;
    }

    // Rest: alphanumeric or underscore
    for &b in &bytes[1..] {
        if !b.is_ascii_alphanumeric() && b != b'_' {
            return false;
        }
    }

    // Heuristic: EU4 tokens are typically lowercase
    // Allow some uppercase but flag if mostly uppercase
    let lowercase_count = bytes.iter().filter(|b| b.is_ascii_lowercase()).count();
    let uppercase_count = bytes.iter().filter(|b| b.is_ascii_uppercase()).count();

    // Reject if more than 50% uppercase (likely not a token)
    if uppercase_count > lowercase_count && bytes.len() > 3 {
        return false;
    }

    true
}

/// Fallback: scan entire binary for token patterns
fn scan_for_tokens(data: &[u8]) -> Result<Vec<(u16, String)>> {
    log::info!("Scanning {} bytes for token patterns...", data.len());

    let tokens = find_token_table(data, 0)?;

    log::info!("Found {} potential tokens via full scan", tokens.len());
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier(b"foo"));
        assert!(is_valid_identifier(b"foo_bar"));
        assert!(is_valid_identifier(b"_foo"));
        assert!(is_valid_identifier(b"foo123"));
        assert!(is_valid_identifier(b"max_manpower"));

        assert!(!is_valid_identifier(b""));
        assert!(!is_valid_identifier(b"123foo"));
        assert!(!is_valid_identifier(b"foo-bar"));
        assert!(!is_valid_identifier(b"foo bar"));
    }

    #[test]
    fn test_try_read_identifier() {
        let data = b"foo\0bar\0";
        let (s, len) = try_read_identifier(data).unwrap();
        assert_eq!(s, "foo");
        assert_eq!(len, 4);

        let (s2, len2) = try_read_identifier(&data[len..]).unwrap();
        assert_eq!(s2, "bar");
        assert_eq!(len2, 4);
    }
}
