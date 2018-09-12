use std::fmt;
use std::sync::PoisonError;
error_chain! {
    foreign_links {
        Url(::url::ParseError);
        Uuid(::uuid::ParseError);
        RocksDB(::rocksdb::Error);
        Utf8(::std::str::Utf8Error);
        Io(::std::io::Error);
        NTriples(::rio::ntriples::ParseError);
        Turtle(::rio::turtle::ParseError);
        SparqlParser(::sparql::parser::ParseError);
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        //TODO: improve conversion
        "Unexpected lock error".into()
    }
}

impl From<Error> for fmt::Error {
    fn from(_: Error) -> Self {
        fmt::Error
    }
}
