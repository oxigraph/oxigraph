//! Utilities to read and write RDF graphs and datasets

pub mod read;
mod syntax;
pub mod write;

pub use self::read::DatasetParser;
pub use self::read::GraphParser;
pub use self::syntax::DatasetSyntax;
#[allow(deprecated)]
pub use self::syntax::FileSyntax;
pub use self::syntax::GraphSyntax;
pub use self::write::DatasetSerializer;
pub use self::write::GraphSerializer;
