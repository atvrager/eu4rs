//! Melt binary saves to text format with unknown token support

use anyhow::{Context, Result};
use jomini::binary::{Lexer, Token};
use jomini::Windows1252Encoding;
use std::io::Write;

/// Melt a binary EU4 save to text format
pub fn melt_save(data: &[u8], output: &mut impl Write) -> Result<MeltStats> {
    // Check for EU4bin header
    let content = if data.starts_with(b"EU4bin") {
        &data[6..]
    } else {
        data
    };

    // Parse the binary format
    let mut lexer = Lexer::new(content);
    let mut stats = MeltStats::default();
    let mut depth: usize = 0;
    let mut need_newline = false;

    while let Some(token) = lexer.next_token().context("Failed to read token")? {
        match token {
            Token::Open => {
                if need_newline {
                    writeln!(output)?;
                }
                writeln!(output, "{{")?;
                depth += 1;
                need_newline = false;
            }
            Token::Close => {
                if need_newline {
                    writeln!(output)?;
                }
                depth = depth.saturating_sub(1);
                write_indent(output, depth)?;
                writeln!(output, "}}")?;
                need_newline = false;
            }
            Token::Equal => {
                write!(output, "=")?;
                need_newline = false;
            }
            Token::Id(id) => {
                stats.total_tokens += 1;
                stats.unknown_tokens += 1;
                if need_newline {
                    writeln!(output)?;
                }
                write_indent(output, depth)?;
                write!(output, "__0x{:04x}", id)?;
                need_newline = false;
            }
            Token::Quoted(s) => {
                let decoded = Windows1252Encoding::decode(s.as_bytes());
                write!(output, "\"{}\"", escape_string(&decoded))?;
                need_newline = true;
            }
            Token::Unquoted(s) => {
                let decoded = Windows1252Encoding::decode(s.as_bytes());
                write!(output, "{}", decoded)?;
                need_newline = true;
            }
            Token::I32(v) => {
                write!(output, "{}", v)?;
                need_newline = true;
            }
            Token::U32(v) => {
                write!(output, "{}", v)?;
                need_newline = true;
            }
            Token::I64(v) => {
                write!(output, "{}", v)?;
                need_newline = true;
            }
            Token::U64(v) => {
                write!(output, "{}", v)?;
                need_newline = true;
            }
            Token::Bool(v) => {
                write!(output, "{}", if v { "yes" } else { "no" })?;
                need_newline = true;
            }
            Token::F32(v) => {
                // v is [u8; 4], convert to f32
                let val = f32::from_le_bytes(v);
                write!(output, "{:.5}", val)?;
                need_newline = true;
            }
            Token::F64(v) => {
                // v is [u8; 8], convert to f64
                let val = f64::from_le_bytes(v);
                write!(output, "{:.5}", val)?;
                need_newline = true;
            }
            Token::Rgb(r) => {
                write!(output, "rgb {{ {} {} {} }}", r.r, r.g, r.b)?;
                need_newline = true;
            }
            Token::Lookup(v) => {
                // Lookup references - output as hex
                write!(output, "lookup:{}", v)?;
                need_newline = true;
            }
        }
    }

    if need_newline {
        writeln!(output)?;
    }

    Ok(stats)
}

fn write_indent(output: &mut impl Write, depth: usize) -> std::io::Result<()> {
    for _ in 0..depth {
        write!(output, "\t")?;
    }
    Ok(())
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[derive(Default, Debug)]
pub struct MeltStats {
    pub total_tokens: usize,
    pub unknown_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello\"world"), "hello\\\"world");
    }
}
