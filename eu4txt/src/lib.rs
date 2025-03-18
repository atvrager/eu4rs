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
        let tokens: Vec<EU4TxtToken> = {
            let mut tokens: Vec<EU4TxtToken> = Vec::new();
            let mut line_iter = contents.lines().peekable();
            while let Some(line) = line_iter.next() {
                let mut tok_iter = line.split_ascii_whitespace().peekable();
                while let Some(tok) = tok_iter.next() {
                    match tok {
                        "=" => tokens.push(EU4TxtToken::Equals),
                        "{" => tokens.push(EU4TxtToken::LeftBrace),
                        "}" => tokens.push(EU4TxtToken::RightBrace),
                        tok if tok.starts_with('#') => {
                            tokens.push(EU4TxtToken::Comment(
                                iter::once(tok)
                                    .chain(from_fn(|| tok_iter.by_ref().next()))
                                    .collect::<Vec<&str>>().join(" ")
                            ));
                        }
                        tok if tok.starts_with('"') => {
                            tokens.push(EU4TxtToken::StringValue(
                                iter::once(tok)
                                    .chain(from_fn(|| tok_iter.by_ref().next_if(|t| t.ends_with('"'))))
                                    .collect::<Vec<&str>>().join(" ")
                            ));
                        }
                        tok if tok.parse::<i32>().is_ok() => {
                            tokens.push(EU4TxtToken::IntValue(tok.parse::<i32>().unwrap()));
                        }
                        tok if tok.parse::<f32>().is_ok() => {
                            tokens.push(EU4TxtToken::FloatValue(tok.parse::<f32>().unwrap()));
                        }
                        _ => {
                            tokens.push(EU4TxtToken::Identifier(tok.to_string()));
                        }
                    }
                }
            }
            tokens
        };
        // tokens.iter().for_each(|f| println!("{:?}", f));
        Ok(tokens)
    }

    fn parse(tokens: Vec<EU4TxtToken>) -> Result<(), ()> {
        Err(())
    }
}
struct DefaultEU4Txt {}
impl EU4Txt for DefaultEU4Txt {}


#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = 
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
}
