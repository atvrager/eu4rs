//! A parser library for Europa Universalis IV text files.
//!
//! This library provides a tokenizer and recursive-descent parser for the EU4
//! text format, which is loosely based on braces `{}` and `key = value` assignments,
//! typically encoded in `WINDOWS_1252`.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::vec::Vec;

use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

pub mod de;
pub use de::from_node;

pub mod error;
pub use error::ParseError;

/// Represents a token scanned from an EU4 text file.
#[derive(Debug, Clone)]
pub enum EU4TxtToken {
    /// An alphanumeric identifier (keys, values).
    Identifier(String),
    /// A quoted string value.
    StringValue(String),
    /// A floating point number.
    FloatValue(f32),
    /// An integer number.
    IntValue(i32),
    /// A comment starting with `#`.
    Comment(String),
    /// `{`
    LeftBrace,
    /// `}`
    RightBrace,
    /// `=`
    Equals,
}

/// Represents an item in the Abstract Syntax Tree (AST).
#[derive(Debug)]
pub enum EU4TxtAstItem {
    /// An empty brace pair `{}` or container helper.
    Brace,
    /// A `key = value` assignment.
    Assignment,
    /// A list of assignments or values (usually enclosed in braces).
    AssignmentList,
    /// An identifier value.
    Identifier(String),
    /// A string value.
    StringValue(String),
    /// A float value.
    FloatValue(f32),
    /// An integer value.
    IntValue(i32),
}

/// A node in the EU4 parse tree.
#[derive(Debug)]
pub struct EU4TxtParseNode {
    /// Child nodes (for lists or assignments).
    pub children: Vec<EU4TxtParseNode>,
    /// The type of item and its data.
    pub entry: EU4TxtAstItem,
}
impl Default for EU4TxtParseNode {
    fn default() -> Self {
        Self::new()
    }
}

impl EU4TxtParseNode {
    /// Creates a new empty node with `Brace` type.
    pub fn new() -> EU4TxtParseNode {
        EU4TxtParseNode {
            children: Vec::new(),
            entry: EU4TxtAstItem::Brace,
        }
    }

    /// Counts the total number of nodes in this subtree (inclusive).
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.node_count()).sum::<usize>()
    }
}

pub trait EU4Txt {
    fn open_txt(path: &str) -> std::io::Result<Vec<EU4TxtToken>> {
        let path = PathBuf::from(path);
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(
            DecodeReaderBytesBuilder::new()
                .encoding(Some(WINDOWS_1252))
                .build(file),
        );
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents)?;

        let mut tokens: Vec<EU4TxtToken> = Vec::new();
        let mut chars = contents.chars().peekable();

        while let Some(&c) = chars.peek() {
            match c {
                c if c.is_whitespace() => {
                    chars.next();
                }
                '#' => {
                    // Comment
                    let mut comment = String::new();
                    chars.next(); // consume #
                    while let Some(&nc) = chars.peek() {
                        if nc == '\n' || nc == '\r' {
                            break;
                        }
                        comment.push(chars.next().unwrap());
                    }
                    // tokens.push(EU4TxtToken::Comment(comment)); // checking logic generally ignores comments, we can skip them or store them
                }
                '{' => {
                    tokens.push(EU4TxtToken::LeftBrace);
                    chars.next();
                }
                '}' => {
                    tokens.push(EU4TxtToken::RightBrace);
                    chars.next();
                }
                '=' => {
                    tokens.push(EU4TxtToken::Equals);
                    chars.next();
                }
                '"' => {
                    // String
                    chars.next(); // consume "
                    let mut s = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc == '"' {
                            chars.next(); // consume closing "
                            break;
                        }
                        // Handle escaped quotes if necessary? EU4 usually just "text"
                        // But let's just consume
                        s.push(chars.next().unwrap());
                    }
                    tokens.push(EU4TxtToken::StringValue(s));
                }
                _ => {
                    // Identifier or Number
                    let mut s = String::new();
                    while let Some(&nc) = chars.peek() {
                        if nc.is_whitespace()
                            || nc == '='
                            || nc == '{'
                            || nc == '}'
                            || nc == '#'
                            || nc == '"'
                        {
                            break;
                        }
                        s.push(chars.next().unwrap());
                    }

                    if let Ok(i) = s.parse::<i32>() {
                        tokens.push(EU4TxtToken::IntValue(i));
                    } else if let Ok(f) = s.parse::<f32>() {
                        if f.is_nan() {
                            if s == "nan" || s == "NaN" {
                                // case insensitive check might be safer but s is exact
                                // It could be a string "Nan", treating as float NaN for now if it parses, but "Nan" is parsed as NaN by rust?
                                // "Nan".parse::<f32>() is Ok(NaN).
                                // But some files have "Nan" as a country tag or name.
                                // If it looks like a number...
                                // logic from old parser:
                                if s == "Nan" {
                                    tokens.push(EU4TxtToken::StringValue(s));
                                } else {
                                    tokens.push(EU4TxtToken::FloatValue(f));
                                }
                            } else {
                                tokens.push(EU4TxtToken::FloatValue(f));
                            }
                        } else {
                            tokens.push(EU4TxtToken::FloatValue(f));
                        }
                    } else {
                        tokens.push(EU4TxtToken::Identifier(s));
                    }
                }
            }
        }
        Ok(tokens)
    }

    fn parse_terminal(
        tokens: &[EU4TxtToken],
        pos: usize,
    ) -> Result<(EU4TxtParseNode, usize), ParseError> {
        let tok: &EU4TxtToken = tokens
            .get(pos)
            .ok_or(ParseError::UnexpectedEof { position: pos })?;
        match tok {
            EU4TxtToken::Identifier(s) => {
                let mut id = EU4TxtParseNode::new();
                id.entry = EU4TxtAstItem::Identifier(s.to_string());
                Ok((id, pos + 1))
            }
            EU4TxtToken::IntValue(i) => {
                let mut int = EU4TxtParseNode::new();
                int.entry = EU4TxtAstItem::IntValue(*i);
                Ok((int, pos + 1))
            }
            EU4TxtToken::FloatValue(f) => {
                let mut float = EU4TxtParseNode::new();
                float.entry = EU4TxtAstItem::FloatValue(*f);
                Ok((float, pos + 1))
            }
            EU4TxtToken::StringValue(s) => {
                let mut string = EU4TxtParseNode::new();
                string.entry = EU4TxtAstItem::StringValue(s.to_string());
                Ok((string, pos + 1))
            }
            _ => Err(ParseError::UnexpectedToken {
                position: pos,
                token: format!("{:?}", tok),
                expected: "identifier, number, or string".to_string(),
            }),
        }
    }

    fn parse_assignment_list(
        tokens: &[EU4TxtToken],
        pos: usize,
    ) -> Result<(EU4TxtParseNode, usize), ParseError> {
        let mut assignment_list = EU4TxtParseNode::new();
        assignment_list.entry = EU4TxtAstItem::AssignmentList;
        let mut loop_pos = pos;
        loop {
            if loop_pos == tokens.len() {
                break;
            }
            let lhs_tok = tokens
                .get(loop_pos)
                .ok_or(ParseError::UnexpectedEof { position: loop_pos })?;
            if let EU4TxtToken::RightBrace = lhs_tok {
                loop_pos += 1;
                break;
            }
            let (node_lhs, eq_pos) = Self::parse_terminal(tokens, loop_pos)?;

            // Validate LHS: must be an identifier or string for assignments
            // EU4 files use both: `key = value` and `"Quoted Key" = value`
            match &node_lhs.entry {
                EU4TxtAstItem::Identifier(_) | EU4TxtAstItem::StringValue(_) => {
                    // Valid LHS
                }
                _ => {
                    // Check if this is part of an assignment (next token is =)
                    if let Some(EU4TxtToken::Equals) = tokens.get(eq_pos) {
                        return Err(ParseError::InvalidLhs {
                            position: loop_pos,
                            found: format!("{:?}", node_lhs.entry),
                        });
                    }
                    // Otherwise it's a value in a list, which is fine
                }
            }

            let eq = tokens.get(eq_pos);
            if eq.is_none() {
                assignment_list.children.push(node_lhs);
                loop_pos += 1;
                continue;
            }
            match eq.unwrap() {
                EU4TxtToken::Equals => {
                    let rhs_tok = tokens.get(eq_pos + 1).ok_or(ParseError::MissingRhs {
                        position: eq_pos + 1,
                    })?;
                    let node_rhs: EU4TxtParseNode;
                    let next_pos: usize;
                    match rhs_tok {
                        EU4TxtToken::LeftBrace => {
                            (node_rhs, next_pos) = Self::parse_assignment_list(tokens, eq_pos + 2)?;
                        }
                        _ => {
                            (node_rhs, next_pos) = Self::parse_terminal(tokens, eq_pos + 1)?;
                        }
                    }
                    let mut assignment = EU4TxtParseNode::new();
                    assignment.entry = EU4TxtAstItem::Assignment;
                    assignment.children.push(node_lhs);
                    assignment.children.push(node_rhs);
                    assignment_list.children.push(assignment);
                    loop_pos = next_pos;
                }
                _ => {
                    // Not an assignment (key=val).
                    // It is a value in a list (val val val).
                    // node_lhs is the value.
                    // We consume it (loop_pos moves to eq_pos).
                    assignment_list.children.push(node_lhs);
                    loop_pos = eq_pos;
                }
            }
        }
        Ok((assignment_list, loop_pos))
    }

    fn parse(tokens: Vec<EU4TxtToken>) -> Result<EU4TxtParseNode, ParseError> {
        if tokens.is_empty() {
            return Err(ParseError::EmptyInput);
        }
        Self::parse_assignment_list(&tokens, 0).and_then(|(n, i)| {
            if i == tokens.len() {
                Ok(n)
            } else {
                Err(ParseError::UnconsumedTokens {
                    position: i,
                    remaining: tokens.len() - i,
                })
            }
        })
    }

    fn pretty_print(ast: &EU4TxtParseNode, depth: usize) -> Result<(), ParseError> {
        match &ast.entry {
            EU4TxtAstItem::AssignmentList => {
                if depth > 0 {
                    println!("{{");
                }
                for child in &ast.children {
                    Self::pretty_print(child, depth + 1)?;
                }
                if depth > 0 {
                    for _ in 0..depth {
                        print!("  ");
                    }
                    println!("}}");
                }
            }
            EU4TxtAstItem::Assignment => {
                let id = ast
                    .children
                    .first()
                    .ok_or(ParseError::UnexpectedEof { position: 0 })?;
                for _ in 0..depth {
                    print!("  ");
                }
                match &id.entry {
                    EU4TxtAstItem::Identifier(id) => {
                        print!("{}", id);
                    }
                    _ => {
                        return Err(ParseError::InvalidLhs {
                            position: 0,
                            found: format!("{:?}", id.entry),
                        });
                    }
                }
                print!(" = ");
                let val = ast
                    .children
                    .get(1)
                    .ok_or(ParseError::MissingRhs { position: 0 })?;
                Self::pretty_print(val, depth)?;
            }
            EU4TxtAstItem::IntValue(i) => {
                println!("{}", i);
            }
            EU4TxtAstItem::FloatValue(f) => {
                println!("{}", f);
            }
            EU4TxtAstItem::Identifier(id) => {
                println!("{}", id);
            }
            EU4TxtAstItem::StringValue(s) => {
                println!("\"{}\"", s);
            }
            EU4TxtAstItem::Brace => {
                // Do nothing or print?
            }
        }
        Ok(())
    }
}
pub struct DefaultEU4Txt {}
impl EU4Txt for DefaultEU4Txt {}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    #[test]
    fn nonexistent() {
        let r = DefaultEU4Txt::open_txt("path/to/nowhere");
        assert!(r.is_err());
    }

    #[test]
    fn exists() {
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "key = value").expect("Failed to write");
        let path = file.path().to_str().unwrap();

        let r = DefaultEU4Txt::open_txt(path);
        assert!(r.is_ok());
    }

    #[test]
    fn parse() {
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "key = value").expect("Failed to write");
        let path = file.path().to_str().unwrap();

        let r = DefaultEU4Txt::open_txt(path);
        assert!(r.is_ok());
        let r2 = DefaultEU4Txt::parse(r.unwrap());
        assert!(r2.is_ok());
    }

    #[test]
    fn pretty_print() {
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "key = value").expect("Failed to write");
        let path = file.path().to_str().unwrap();

        let r = DefaultEU4Txt::open_txt(path);
        assert!(r.is_ok());
        let r2 = DefaultEU4Txt::parse(r.unwrap());
        assert!(r2.is_ok());
        assert!(DefaultEU4Txt::pretty_print(&r2.unwrap(), 0).is_ok());
    }

    #[test]
    fn test_empty_input() {
        let tokens = vec![];
        let result = DefaultEU4Txt::parse(tokens);
        assert!(matches!(result, Err(ParseError::EmptyInput)));
    }

    #[test]
    fn test_missing_rhs() {
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "key =").expect("Failed to write");
        let path = file.path().to_str().unwrap();

        let tokens = DefaultEU4Txt::open_txt(path).expect("Failed to open");
        let result = DefaultEU4Txt::parse(tokens);
        assert!(matches!(result, Err(ParseError::MissingRhs { .. })));
    }

    #[test]
    fn test_invalid_lhs() {
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "123 = value").expect("Failed to write");
        let path = file.path().to_str().unwrap();

        let tokens = DefaultEU4Txt::open_txt(path).expect("Failed to open");
        let result = DefaultEU4Txt::parse(tokens);
        assert!(matches!(result, Err(ParseError::InvalidLhs { .. })));
    }

    #[test]
    fn test_unconsumed_tokens() {
        // Create tokens where a RightBrace ends the list but more tokens follow
        let tokens = vec![
            EU4TxtToken::Identifier("key".to_string()),
            EU4TxtToken::Equals,
            EU4TxtToken::LeftBrace,
            EU4TxtToken::Identifier("nested".to_string()),
            EU4TxtToken::RightBrace, // Closes the nested brace
            EU4TxtToken::RightBrace, // Closes the top-level implicit list
            EU4TxtToken::Identifier("extra".to_string()), // This should be unconsumed
        ];
        let result = DefaultEU4Txt::parse(tokens);
        assert!(matches!(
            result,
            Err(ParseError::UnconsumedTokens {
                position: 6,
                remaining: 1
            })
        ));
    }

    #[test]
    fn test_error_display() {
        let err = ParseError::UnexpectedEof { position: 5 };
        assert_eq!(err.to_string(), "Unexpected end of file at position 5");

        let err = ParseError::InvalidLhs {
            position: 3,
            found: "IntValue(123)".to_string(),
        };
        assert!(err.to_string().contains("Invalid left-hand side"));
        assert!(err.to_string().contains("position 3"));

        let err = ParseError::EmptyInput;
        assert_eq!(err.to_string(), "Cannot parse empty input");
    }
}
