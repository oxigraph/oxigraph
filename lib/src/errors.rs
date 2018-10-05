use quick_xml::Error as Xml_Error;
use std::fmt;
use std::sync::PoisonError;

error_chain! {
    foreign_links {
        Url(::url::ParseError);
        RocksDB(::rocksdb::Error);
        Utf8(::std::str::Utf8Error);
        Io(::std::io::Error);
        NTriples(::rio::ntriples::ParseError);
        Turtle(::rio::turtle::ParseError);
        SparqlParser(::sparql::parser::ParseError);
    }

    errors {
        Xml(error: Xml_Error) {
            description("XML parsing error")
            display("XML parsing error: {:?}", error)
        }
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        //TODO: improve conversion
        "Unexpected lock error".into()
    }
}

impl From<Xml_Error> for Error {
    fn from(error: Xml_Error) -> Self {
        match error {
            Xml_Error::Io(error) => error.into(),
            Xml_Error::Utf8(error) => error.into(),
            error => ErrorKind::Xml(error).into(),
        }
    }
}

impl From<Error> for fmt::Error {
    fn from(_: Error) -> Self {
        fmt::Error
    }
}
