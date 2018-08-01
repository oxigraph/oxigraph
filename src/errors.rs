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
