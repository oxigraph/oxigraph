//! oxttl parsing toolkit.
//!
//! Provides the basic code to write plain Rust lexers and parsers able to read files chunk by chunk.

mod error;
mod lexer;
mod parser;

pub use self::error::{TextPosition, TurtleParseError, TurtleSyntaxError};
pub use self::lexer::{Lexer, TokenOrLineJump, TokenRecognizer, TokenRecognizerError};
#[cfg(feature = "async-tokio")]
pub use self::parser::TokioAsyncReaderIterator;
pub use self::parser::{
    Parser, ReaderIterator, RuleRecognizer, RuleRecognizerError, SliceIterator,
};
