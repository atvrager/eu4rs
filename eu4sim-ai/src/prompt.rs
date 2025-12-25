//! Prompt building for EU4 AI inference.
//!
//! Converts game state and available actions into a prompt string
//! that the model can process.

use eu4sim_core::Command;
use eu4sim_core::ai::CommandCategory;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Builds prompts for the AI model.
pub struct PromptBuilder {
    buffer: String,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    /// Create a new prompt builder with pre-allocated buffer.
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(4096),
        }
    }

    /// Build a prompt from game state and available commands.
    ///
    /// # Format
    /// ```text
    /// <|country|>FRA<|/country|>
    /// <|state|>
    /// Date: 1445.3.15
    /// Treasury: 523 ducats
    /// ...
    /// <|/state|>
    /// <|actions|>
    /// 0: Move Army 1 to Normandy
    /// 1: Pass
    /// <|/actions|>
    /// <|choice|>
    /// ```
    pub fn build(&mut self, country: &str, state_text: &str, commands: &[Command]) -> &str {
        self.buffer.clear();

        // Country tag
        writeln!(self.buffer, "<|country|>{}<|/country|>", country).unwrap();

        // State description
        writeln!(self.buffer, "<|state|>\n{}<|/state|>", state_text).unwrap();

        // Available actions
        self.buffer.push_str("<|actions|>\n");
        for (i, cmd) in commands.iter().enumerate() {
            writeln!(self.buffer, "{}: {}", i, format_command(cmd)).unwrap();
        }
        self.buffer.push_str("<|/actions|>\n");

        // Prompt for choice
        self.buffer.push_str("<|choice|>");

        &self.buffer
    }

    /// Maximum commands per category to prevent prompt explosion.
    /// With 6 categories × 10 commands × ~50 chars = ~3000 chars for actions alone.
    const MAX_COMMANDS_PER_CATEGORY: usize = 10;

    /// Build a multi-action prompt grouped by command category.
    ///
    /// Uses per-category local indices (0=Pass, 1=first action, 2=second action, etc.)
    /// which are more intuitive for the model. The parser must remap these back to
    /// global indices using `parse_multi_action_response`.
    ///
    /// Commands are limited to MAX_COMMANDS_PER_CATEGORY per category to keep
    /// prompt length reasonable (~1000-2000 tokens).
    ///
    /// # Format
    /// ```text
    /// <|country|>KOR<|/country|>
    /// <|state|>
    /// Date: 1445.3.15
    /// Treasury: 523 ducats
    /// ...
    /// <|/state|>
    /// <|actions|>
    /// DIPLOMATIC:
    ///   0: Pass
    ///   1: Declare war on MNG
    /// MILITARY:
    ///   0: Pass
    ///   1: Move Army 1 to province 123
    ///   2: Move Army 2 to province 124
    /// ECONOMIC:
    ///   0: Pass
    ///   1: Develop Seoul (admin)
    /// TRADE:
    ///   0: Pass
    /// COLONIZATION:
    ///   0: Pass
    /// OTHER:
    ///   0: Pass
    /// <|/actions|>
    /// <|choice|>
    /// DIPLOMATIC:
    /// MILITARY:
    /// ECONOMIC:
    /// TRADE:
    /// COLONIZATION:
    /// OTHER:
    /// ```
    pub fn build_multi_action(
        &mut self,
        country: &str,
        state_text: &str,
        commands: &[Command],
    ) -> &str {
        self.buffer.clear();

        // Country tag
        writeln!(self.buffer, "<|country|>{}<|/country|>", country).unwrap();

        // State description
        writeln!(self.buffer, "<|state|>\n{}<|/state|>", state_text).unwrap();

        // Group commands by category, preserving global index
        let mut by_category: BTreeMap<CommandCategory, Vec<(usize, &Command)>> = BTreeMap::new();
        for (idx, cmd) in commands.iter().enumerate() {
            by_category
                .entry(cmd.category())
                .or_default()
                .push((idx, cmd));
        }

        // Actions section grouped by category
        self.buffer.push_str("<|actions|>\n");

        // All 6 categories in order
        let all_categories = [
            CommandCategory::Diplomatic,
            CommandCategory::Military,
            CommandCategory::Economic,
            CommandCategory::Trade,
            CommandCategory::Colonization,
            CommandCategory::Other,
        ];

        for category in all_categories {
            writeln!(self.buffer, "{}:", category_name(category)).unwrap();
            self.buffer.push_str("  0: Pass\n");

            if let Some(cmds) = by_category.get(&category) {
                // Use local indices: 1, 2, 3, ... (0 is always Pass)
                // Limit to MAX_COMMANDS_PER_CATEGORY to prevent prompt explosion
                for (local_idx, (_global_idx, cmd)) in cmds
                    .iter()
                    .take(Self::MAX_COMMANDS_PER_CATEGORY)
                    .enumerate()
                {
                    writeln!(self.buffer, "  {}: {}", local_idx + 1, format_command(cmd)).unwrap();
                }
                // Note if truncated
                if cmds.len() > Self::MAX_COMMANDS_PER_CATEGORY {
                    writeln!(
                        self.buffer,
                        "  ... ({} more)",
                        cmds.len() - Self::MAX_COMMANDS_PER_CATEGORY
                    )
                    .unwrap();
                }
            }
        }

        self.buffer.push_str("<|/actions|>\n");

        // Response template
        self.buffer.push_str("<|choice|>\n");
        for category in all_categories {
            writeln!(self.buffer, "{}:", category_name(category)).unwrap();
        }

        &self.buffer
    }

    /// Build index mapping from local per-category indices to global command indices.
    ///
    /// Returns a map: (CommandCategory, local_index) -> global_index
    /// where local_index 0 = Pass (not in map), 1 = first command in category, etc.
    ///
    /// Only includes the first MAX_COMMANDS_PER_CATEGORY commands per category
    /// to match what's shown in the prompt.
    pub fn build_index_map(commands: &[Command]) -> BTreeMap<(CommandCategory, usize), usize> {
        let mut by_category: BTreeMap<CommandCategory, Vec<usize>> = BTreeMap::new();
        for (idx, cmd) in commands.iter().enumerate() {
            by_category.entry(cmd.category()).or_default().push(idx);
        }

        let mut index_map = BTreeMap::new();
        for (category, global_indices) in by_category {
            // Only map the first MAX_COMMANDS_PER_CATEGORY (matching prompt truncation)
            for (local_idx, global_idx) in global_indices
                .into_iter()
                .take(Self::MAX_COMMANDS_PER_CATEGORY)
                .enumerate()
            {
                // local_idx 0 -> local display index 1
                index_map.insert((category, local_idx + 1), global_idx);
            }
        }

        index_map
    }

    /// Build a minimal prompt (for testing or simple scenarios).
    pub fn build_minimal(&mut self, country: &str, date: &str, commands: &[Command]) -> &str {
        self.buffer.clear();

        writeln!(self.buffer, "<|country|>{}<|/country|>", country).unwrap();
        writeln!(self.buffer, "<|state|>\nDate: {}\n<|/state|>", date).unwrap();

        self.buffer.push_str("<|actions|>\n");
        for (i, cmd) in commands.iter().enumerate() {
            writeln!(self.buffer, "{}: {}", i, format_command(cmd)).unwrap();
        }
        self.buffer.push_str("<|/actions|>\n");
        self.buffer.push_str("<|choice|>");

        &self.buffer
    }
}

/// Get display name for a command category.
fn category_name(cat: CommandCategory) -> &'static str {
    match cat {
        CommandCategory::Diplomatic => "DIPLOMATIC",
        CommandCategory::Military => "MILITARY",
        CommandCategory::Economic => "ECONOMIC",
        CommandCategory::Trade => "TRADE",
        CommandCategory::Colonization => "COLONIZATION",
        CommandCategory::Other => "OTHER",
    }
}

/// Format a command for display in the prompt.
fn format_command(cmd: &Command) -> String {
    match cmd {
        // Military movement
        Command::Move {
            army_id,
            destination,
        } => {
            format!("Move Army {} to province {}", army_id, destination)
        }
        Command::MoveFleet {
            fleet_id,
            destination,
        } => {
            format!("Move Fleet {} to province {}", fleet_id, destination)
        }

        // Economic
        Command::BuildInProvince { province, building } => {
            format!("Build {} in province {}", building, province)
        }
        Command::DevelopProvince { province, dev_type } => {
            format!("Develop {:?} in province {}", dev_type, province)
        }

        // Diplomatic
        Command::DeclareWar { target, cb } => {
            if let Some(cb) = cb {
                format!("Declare war on {} (CB: {})", target, cb)
            } else {
                format!("Declare war on {}", target)
            }
        }
        Command::OfferPeace { war_id, .. } => {
            format!("Offer peace in war {}", war_id)
        }
        Command::OfferAlliance { target } => {
            format!("Offer alliance to {}", target)
        }

        // Tech
        Command::BuyTech { tech_type } => {
            format!("Research {:?} technology", tech_type)
        }

        // Pass
        Command::Pass => "Pass (do nothing)".to_string(),

        // Fallback for other commands
        _ => format!("{:?}", cmd),
    }
}

/// Format a number with thousands separators.
#[allow(dead_code)]
pub fn format_thousands(n: f64) -> String {
    let n = n.round() as i64;
    if n.abs() < 1000 {
        return n.to_string();
    }

    let s = n.abs().to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    if n < 0 {
        result.push('-');
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_thousands() {
        assert_eq!(format_thousands(0.0), "0");
        assert_eq!(format_thousands(999.0), "999");
        assert_eq!(format_thousands(1000.0), "1,000");
        assert_eq!(format_thousands(1234567.0), "1,234,567");
        assert_eq!(format_thousands(-1234.0), "-1,234");
    }

    #[test]
    fn test_prompt_builder() {
        let mut builder = PromptBuilder::new();
        let commands = vec![Command::Pass];
        let prompt = builder.build_minimal("FRA", "1444.11.11", &commands);

        assert!(prompt.contains("<|country|>FRA<|/country|>"));
        assert!(prompt.contains("Date: 1444.11.11"));
        assert!(prompt.contains("0: Pass"));
        assert!(prompt.ends_with("<|choice|>"));
    }
}
