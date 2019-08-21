//! Provides ready to use `NamedNode`s for basic RDF vocabularies

pub mod rdf {
    //! [RDF 1.1](https://www.w3.org/TR/rdf11-concepts/) vocabulary
    use crate::model::named_node::NamedNode;
    use lazy_static::lazy_static;

    lazy_static! {
        /// The class of containers of alternatives.
        pub static ref ALT: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#Alt");
        /// The class of unordered containers.
        pub static ref BAG: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#Bag");
        /// The first item in the subject RDF list.
        pub static ref FIRST: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#first");
        /// The class of HTML literal values.
        pub static ref HTML: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#HTML");
        /// The class of language-tagged string literal values.
        pub static ref LANG_STRING: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString");
        /// The class of RDF Lists.
        pub static ref LIST: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#List");
        pub static ref NIL: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil");
        /// The object of the subject RDF statement.
        pub static ref OBJECT: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#object");
        /// The predicate of the subject RDF statement.
        pub static ref PREDICATE: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate");
        /// The class of RDF properties.
        pub static ref PROPERTY: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#Property");
        /// The rest of the subject RDF list after the first item.
        pub static ref REST: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest");
        /// The class of ordered containers.
        pub static ref SEQ: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#Seq");
        /// The class of RDF statements.
        pub static ref STATEMENT: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement");
        /// The subject of the subject RDF statement.
        pub static ref SUBJECT: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#subject");
        /// The subject is an instance of a class.
        pub static ref TYPE: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
        /// Idiomatic property used for structured values.
        pub static ref VALUE: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#value");
        /// The class of XML literal values.
        pub static ref XML_LITERAL: NamedNode =
            NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#XMLLiteral");
    }
}

pub mod rdfs {
    //! [RDFS](https://www.w3.org/TR/rdf-schema/) vocabulary
    use crate::model::named_node::NamedNode;
    use lazy_static::lazy_static;

    lazy_static! {
        /// The class of classes.
        pub static ref CLASS: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#Class");
        /// A description of the subject resource.
        pub static ref COMMENT: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#comment");
        /// The class of RDF containers.
        pub static ref CONTAINER: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#Container");
        /// The class of container membership properties, rdf:_1, rdf:_2, ..., all of which are sub-properties of 'member'.
        pub static ref CONTAINER_MEMBERSHIP_PROPERTY: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#ContainerMembershipProperty");
        /// The class of RDF datatypes.
        pub static ref DATATYPE: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#Datatype");
        /// A domain of the subject property.
        pub static ref DOMAIN: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#domain");
        /// The definition of the subject resource.
        pub static ref IS_DEFINED_BY: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#isDefinedBy");
        /// A human-readable name for the subject.
        pub static ref LABEL: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#label");
        /// The class of literal values, e.g. textual strings and integers.
        pub static ref LITERAL: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#Literal");
        /// A member of the subject resource.
        pub static ref MEMBER: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#member");
        /// A range of the subject property.
        pub static ref RANGE: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#range");
        /// The class resource, everything.
        pub static ref RESOURCE: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#Resource");
        /// Further information about the subject resource.
        pub static ref SEE_ALSO: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#seeAlso");
        /// The subject is a subclass of a class.
        pub static ref SUB_CLASS_OF: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#subClassOf");
        /// The subject is a subproperty of a property.
        pub static ref SUB_PROPERTY_OF: NamedNode =
            NamedNode::new("http://www.w3.org/2000/01/rdf-schema#subPropertyOf");
    }
}

pub mod xsd {
    //! `NamedNode`s for [RDF compatible XSD datatypes](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-compatible-xsd-types)
    use crate::model::named_node::NamedNode;
    use lazy_static::lazy_static;

    lazy_static! {
        /// true, false
        pub static ref BOOLEAN: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#boolean");
        /// 128…+127 (8 bit)
        pub static ref BYTE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#byte");
        /// Dates (yyyy-mm-dd) with or without timezone
        pub static ref DATE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#date");
        /// Duration of time (days, hours, minutes, seconds only)
        pub static ref DAY_TIME_DURATION: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#dayTimeDuration");
        /// Date and time with or without timezone
        pub static ref DATE_TIME: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#dateTime");
        /// Date and time with required timezone
        pub static ref DATE_TIME_STAMP: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#dateTimeStamp");
        /// Arbitrary-precision decimal numbers
        pub static ref DECIMAL: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#decimal");
        /// 64-bit floating point numbers incl. ±Inf, ±0, NaN
        pub static ref DOUBLE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#double");
        /// Duration of time
        pub static ref DURATION: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#duration");
        /// 32-bit floating point numbers incl. ±Inf, ±0, NaN
        pub static ref FLOAT: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#float");
        /// Gregorian calendar day of the month
        pub static ref G_DAY: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#gDay");
        /// Gregorian calendar month
        pub static ref G_MONTH: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#gMonth");
        /// Gregorian calendar month and day
        pub static ref G_MONTH_DAY: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#gMonthDay");
        /// Gregorian calendar year
        pub static ref G_YEAR: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#gYear");
        /// Gregorian calendar year and month
        pub static ref G_YEAR_MONTH: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#gYearMonth");
        /// -2147483648…+2147483647 (32 bit)
        pub static ref INT: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#int");
        /// Arbitrary-size integer numbers
        pub static ref INTEGER: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#integer");
        /// -9223372036854775808…+9223372036854775807 (64 bit)
        pub static ref LONG: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#long");
        /// Integer numbers <0
        pub static ref NEGATIVE_INTEGER: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#negativeInteger");
        /// Integer numbers ≥0
        pub static ref NON_NEGATIVE_INTEGER: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#nonNegativeInteger");
        /// Integer numbers ≤0
        pub static ref NON_POSITIVE_INTEGER: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#nonPositiveInteger");
        /// Integer numbers >0
        pub static ref POSITIVE_INTEGER: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#positiveInteger");
        /// Times (hh:mm:ss.sss…) with or without timezone
        pub static ref TIME: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#time");
        /// -32768…+32767 (16 bit)
        pub static ref SHORT: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#short");
        /// Character strings (but not all Unicode character strings)
        pub static ref STRING: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#string");
        /// 0…255 (8 bit)
        pub static ref UNSIGNED_BYTE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#unsignedByte");
        /// 0…4294967295 (32 bit)
        pub static ref UNSIGNED_INT: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#unsignedInt");
        /// 0…18446744073709551615 (64 bit)
        pub static ref UNSIGNED_LONG: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#unsignedLong");
        /// 0…65535 (16 bit)
        pub static ref UNSIGNED_SHORT: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#unsignedShort");
        /// Duration of time (months and years only)
        pub static ref YEAR_MONTH_DURATION: NamedNode =
            NamedNode::new("http://www.w3.org/2001/XMLSchema#yearMonthDuration");
    }
}
