//! Error types for the EU4 text parser.

use std::fmt;

/// Errors that can occur during parsing of EU4 text files.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Unexpected end of file while parsing.
    UnexpectedEof {
        /// Position in the token stream where EOF was encountered.
        position: usize,
    },
    /// Encountered an unexpected token.
    UnexpectedToken {
        /// Position in the token stream.
        position: usize,
        /// The token that was found.
        token: String,
        /// What was expected instead.
        expected: String,
    },
    /// Invalid left-hand side in an assignment.
    InvalidLhs {
        /// Position in the token stream.
        position: usize,
        /// What was found on the LHS.
        found: String,
    },
    /// Missing right-hand side after `=` in an assignment.
    MissingRhs {
        /// Position in the token stream where RHS was expected.
        position: usize,
    },
    /// Parsing succeeded but there are unconsumed tokens remaining.
    UnconsumedTokens {
        /// Position where unconsumed tokens start.
        position: usize,
        /// Number of tokens remaining.
        remaining: usize,
    },
    /// Input was empty (no tokens to parse).
    EmptyInput,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedEof { position } => {
                write!(f, "Unexpected end of file at position {}", position)
            }
            ParseError::UnexpectedToken {
                position,
                token,
                expected,
            } => {
                write!(
                    f,
                    "Unexpected token '{}' at position {}, expected {}",
                    token, position, expected
                )
            }
            ParseError::InvalidLhs { position, found } => {
                write!(
                    f,
                    "Invalid left-hand side '{}' at position {} (must be an identifier)",
                    found, position
                )
            }
            ParseError::MissingRhs { position } => {
                write!(f, "Missing right-hand side at position {}", position)
            }
            ParseError::UnconsumedTokens {
                position,
                remaining,
            } => {
                write!(
                    f,
                    "Parsing incomplete: {} unconsumed tokens starting at position {}",
                    remaining, position
                )
            }
            ParseError::EmptyInput => {
                write!(f, "Cannot parse empty input")
            }
        }
    }
}

impl std::error::Error for ParseError {}
