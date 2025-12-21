//! Prompt building for EU4 AI inference.
//!
//! Converts game state and available actions into a prompt string
//! that the model can process.

use eu4sim_core::Command;
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
