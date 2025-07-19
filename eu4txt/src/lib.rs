use std::path::PathBuf;
use std::fs::File;
use std::iter::{self, from_fn};
use std::io::{BufReader, Read};
use std::vec::Vec;

use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

#[derive(Debug)]
pub enum EU4TxtToken {
    Identifier(String),
    StringValue(String),
    FloatValue(f32),
    IntValue(i32),
    Comment(String),
    LeftBrace,
    RightBrace,
    Equals,
}

#[derive(Debug)]
pub enum EU4TxtAstItem {
    Brace,
    Assignment,
    AssignmentList,
    Identifier(String),
    StringValue(String),
    FloatValue(f32),
    IntValue(i32),
}

#[derive(Debug)]
pub struct EU4TxtParseNode {
    pub children: Vec<EU4TxtParseNode>,
    pub entry: EU4TxtAstItem,
}
impl EU4TxtParseNode {
    pub fn new() -> EU4TxtParseNode {
        EU4TxtParseNode {
            children: Vec::new(),
            entry: EU4TxtAstItem::Brace,
        }
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
        let (tokens, _comments): (Vec<EU4TxtToken>, Vec<EU4TxtToken>) = {
            let mut tokens: Vec<EU4TxtToken> = Vec::new();
            let mut comments: Vec<EU4TxtToken> = Vec::new();
            let mut line_iter = contents.lines().peekable();
            while let Some(line) = line_iter.next() {
                let mut tok_iter = line.split_ascii_whitespace().peekable();
                while let Some(tok) = tok_iter.next() {
                    match tok {
                        // TODO: make the tokenizer a bit smarter, to remove vvv
                        // Found in common/achievements.txt
                        "={" => {
                            tokens.push(EU4TxtToken::Equals);
                            tokens.push(EU4TxtToken::LeftBrace);
                        }
                        "=" => tokens.push(EU4TxtToken::Equals),
                        tok if tok.ends_with('=') => {
                            tokens.push(EU4TxtToken::Identifier(tok.trim_end_matches('=').to_string()));
                            tokens.push(EU4TxtToken::Equals);
                        }
                        "{" => tokens.push(EU4TxtToken::LeftBrace),
                        "}" => tokens.push(EU4TxtToken::RightBrace),
                        tok if tok.starts_with('#') => {
                            comments.push(EU4TxtToken::Comment(
                                iter::once(tok)
                                    .chain(from_fn(|| tok_iter.by_ref().next()))
                                    .collect::<Vec<&str>>().join(" ")
                            ));
                        }
                        tok if tok.starts_with('"') => {
                            let mut string_parts = vec![tok];
                            if !tok.ends_with('"') {
                                while let Some(next_tok) = tok_iter.next() {
                                    string_parts.push(next_tok);
                                    if next_tok.ends_with('"') {
                                        break;
                                    }
                                }
                            }
                            tokens.push(EU4TxtToken::StringValue(string_parts.join(" ")));
                        }
                        tok if tok.parse::<i32>().is_ok() => {
                            tokens.push(EU4TxtToken::IntValue(tok.parse::<i32>().unwrap()));
                        }
                        tok if tok.parse::<f32>().is_ok() => {
                            if tok.parse::<f32>().unwrap().is_nan() {
                                // countries/LanNa.txt
                                if tok == "Nan" {
                                    tokens.push(EU4TxtToken::StringValue("Nan".to_string()));
                                }
                                else {
                                    tokens.push(EU4TxtToken::FloatValue(std::f32::NAN));
                                }
                            } else {
                                tokens.push(EU4TxtToken::FloatValue(tok.parse::<f32>().unwrap()));
                            }
                        }
                        _ => {
                            tokens.push(EU4TxtToken::Identifier(tok.to_string()));
                        }
                    }
                }
            }
            (tokens, comments)
        };
        // tokens.iter().for_each(|f| println!("{:?}", f));
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
                    match &child.entry {
                        EU4TxtAstItem::Assignment => {
                            Self::pretty_print(child, depth + 1)?;
                        }
                        _ => {
                            return Err("Unknown type for printing!".to_string())
                        }
                    }
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
            _ => {
                println!("Unknown -> {:?}", ast.entry);
                return Err("Unknown type for printing!".to_string())
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
