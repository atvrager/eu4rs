//! A parser library for Europa Universalis IV text files.
//!
//! This library provides a tokenizer and recursive-descent parser for the EU4
//! text format, which is loosely based on braces `{}` and `key = value` assignments,
//! typically encoded in `WINDOWS_1252`.

use std::path::PathBuf;
use std::fs::File;
use std::io::{BufReader, Read};
use std::vec::Vec;

use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;



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
        let mut buf_reader =
            BufReader::new(DecodeReaderBytesBuilder::new()
                            .encoding(Some(WINDOWS_1252))
                            .build(file));
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
                        if nc.is_whitespace() || nc == '=' || nc == '{' || nc == '}' || nc == '#' || nc == '"' {
                            break;
                        }
                        s.push(chars.next().unwrap());
                    }
                    
                    if let Ok(i) = s.parse::<i32>() {
                        tokens.push(EU4TxtToken::IntValue(i));
                    } else if let Ok(f) = s.parse::<f32>() {
                        if f.is_nan() {
                             if s == "nan" || s == "NaN" { // case insensitive check might be safer but s is exact
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

    fn parse_terminal(tokens: &Vec<EU4TxtToken>, pos: usize) -> Result<(EU4TxtParseNode, usize), String> {
        let tok: &EU4TxtToken = tokens.get(pos).ok_or("Unexpected EOF")?;
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
            _ => {
                Err(format!("Unimplemented {:?} @ {}", tok, pos))
            }
        }
    }

    fn parse_assignment_list(tokens: &Vec<EU4TxtToken>, pos: usize) -> Result<(EU4TxtParseNode, usize), String> {
        let mut assignment_list = EU4TxtParseNode::new();
        assignment_list.entry = EU4TxtAstItem::AssignmentList;
        let mut loop_pos = pos;
        loop {
            if loop_pos == tokens.len() {
                break;
            }
            let lhs_tok = tokens.get(loop_pos).ok_or(format!("no lhs tok @ {}", loop_pos))?;
            match lhs_tok {
                EU4TxtToken::RightBrace => {
                    loop_pos += 1;
                    break;
                }
                _ => {}
            }
            let (node_lhs, eq_pos) = Self::parse_terminal(tokens, loop_pos)?;
            // TODO: what if LHS is }?
            // TODO: assert lhs is identifier
            let eq = tokens.get(eq_pos);
            match eq {
                None => {
                    assignment_list.children.push(node_lhs);
                    loop_pos += 1;
                    continue;
                }
                Some(_) => {}
            };
            match eq.unwrap() {
                EU4TxtToken::Equals => {
                    let rhs_tok = tokens.get(eq_pos + 1).ok_or("no rhs tok".to_string())?;
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
                    // TODO: assert rhs is list OR terminal
                    let mut assignment = EU4TxtParseNode::new();
                    assignment.entry = EU4TxtAstItem::Assignment;
                    assignment.children.push(node_lhs);
                    assignment.children.push(node_rhs);
                    assignment_list.children.push(assignment);
                    loop_pos = next_pos;
                }
                EU4TxtToken::Identifier(id) => {
                    let mut unary = EU4TxtParseNode::new();
                    unary.entry = EU4TxtAstItem::Identifier(id.to_string());
                    assignment_list.children.push(unary);
                    loop_pos += 1;
                }
                EU4TxtToken::IntValue(i) => {
                    let mut unary = EU4TxtParseNode::new();
                    unary.entry = EU4TxtAstItem::IntValue(*i);
                    assignment_list.children.push(unary);
                    loop_pos += 1;
                }
                EU4TxtToken::FloatValue(f) => {
                    let mut unary = EU4TxtParseNode::new();
                    unary.entry = EU4TxtAstItem::FloatValue(*f);
                    assignment_list.children.push(unary);
                    loop_pos += 1;
                }
                EU4TxtToken::StringValue(s) => {
                    let mut unary = EU4TxtParseNode::new();
                    unary.entry = EU4TxtAstItem::StringValue(s.to_string());
                    assignment_list.children.push(unary);
                    loop_pos += 1;
                }
                EU4TxtToken::RightBrace => {
                    loop_pos += 1;
                }
                _ => {
                    return Err(format!("Unhandled {:?} in list @ {}", eq, eq_pos));
                }
            }
        }
        Ok((assignment_list, loop_pos))
    }

    fn parse(tokens: Vec<EU4TxtToken>) -> Result<EU4TxtParseNode, String> {
        // TODO: define an error type enum, that way this can be an error we can discriminate
        if tokens.len() == 0 {
            return Err("NoTokens".to_string());
        }
        Self::parse_assignment_list(&tokens, 0).and_then(|(n, i)| if i == tokens.len() {
            Ok(n)
        } else {
            Err(format!("Parsing failed! {} != {} tok ({:?})", i, tokens.len(), tokens.get(i).unwrap()))
        })
    }

    fn pretty_print(ast: &EU4TxtParseNode, depth: usize) -> Result<(), String> {
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
                let id = ast.children.get(0).ok_or("missing id")?;
                for _ in 0..depth {
                    print!("  ");
                }
                match &id.entry {
                    EU4TxtAstItem::Identifier(id) => {
                        print!("{}", id);
                    }
                    _ => {
                        return Err("LHS not an identifier".to_string());
                    }
                }
                print!(" = ");
                let val = ast.children.get(1).ok_or("missing val")?;
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

    const PATH: &str = 
        // "c:\\users\\atv\\Documents\\src\\eu4rs\\eu4txt\\src\\test.txt";
        "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Europa Universalis IV\\common\\defender_of_faith\\00_defender_of_faith.txt";

    #[test]
    fn nonexistent() {
        let r = DefaultEU4Txt::open_txt("path/to/nowhere");
        assert!(r.is_err());
    }

    #[test]
    fn exists() {
        let r = DefaultEU4Txt::open_txt(PATH);
        assert!(r.is_ok());
    }

    #[test]
    fn parse() {
        let r = DefaultEU4Txt::open_txt(PATH);
        assert!(r.is_ok());
        let r2 = DefaultEU4Txt::parse(r.unwrap());
        assert!(r2.is_ok());
    }

    #[test]
    fn pretty_print() {
        let r = DefaultEU4Txt::open_txt(PATH);
        assert!(r.is_ok());
        let r2 = DefaultEU4Txt::parse(r.unwrap());
        assert!(r2.is_ok());
        assert!(DefaultEU4Txt::pretty_print(&r2.unwrap(), 0).is_ok());
    }
}
