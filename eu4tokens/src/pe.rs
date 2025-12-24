use anyhow::Result;

/// Extract token mappings from a PE binary (Windows EU4)
///
/// **Not yet implemented.** Windows/PE support is planned for a future release.
///
/// For now, you can:
/// 1. Use the `use` subcommand with an existing tokens.txt file
/// 2. Run token extraction on a Linux system with the Linux EU4 binary
/// 3. Use tokens from pdx-tools or similar projects
pub fn extract_tokens(_data: &[u8]) -> Result<Vec<(u16, String)>> {
    anyhow::bail!(
        "Windows PE binary support is not yet implemented.\n\n\
        Alternatives:\n\
        1. Use 'eu4tokens use <tokens.txt>' with an existing tokens file\n\
        2. Run extraction on Linux with the Linux EU4 binary\n\
        3. Obtain tokens from pdx-tools or similar projects\n\n\
        PE support is planned for a future release."
    )
}
