//! Provides ready to use [`NamedNodeRef`](super::NamedNodeRef)s for basic RDF vocabularies.

pub mod rdf {
    //! [RDF](https://www.w3.org/TR/rdf11-concepts/) vocabulary.
    use crate::named_node::NamedNodeRef;

    /// The class of containers of alternatives.
    pub const ALT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#Alt");
    /// The class of unordered containers.
    pub const BAG: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#Bag");
    /// The class of language-tagged string literal values with a base direction.
    #[cfg(feature = "rdf-12")]
    pub const DIR_LANG_STRING: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#dirLangString");
    /// The first item in the subject RDF list.
    pub const FIRST: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#first");
    /// The class of HTML literal values.
    pub const HTML: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#HTML");
    /// The datatype of RDF literals storing JSON content.
    #[cfg(feature = "rdf-12")]
    pub const JSON: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#JSON");
    pub const LANG_STRING: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#langString");
    /// The class of RDF lists.
    pub const LIST: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#List");
    /// The empty list.
    pub const NIL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil");
    /// The object of the subject RDF statement.
    pub const OBJECT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#object");
    /// The predicate of the subject RDF statement.
    pub const PREDICATE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate");
    /// The class of RDF properties.
    pub const PROPERTY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#Property");
    /// Associate a resource (reifier) with a triple (proposition).
    #[cfg(feature = "rdf-12")]
    pub const REIFIES: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#reifies");
    /// The rest of the subject RDF list after the first item.
    pub const REST: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest");
    /// The class of ordered containers.
    pub const SEQ: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#Seq");
    /// The class of RDF statements.
    pub const STATEMENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement");
    /// The subject of the subject RDF statement.
    pub const SUBJECT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#subject");
    /// The subject is an instance of a class.
    pub const TYPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    /// Idiomatic property used for structured values.
    pub const VALUE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#value");
    /// The class of XML literal values.
    pub const XML_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#XMLLiteral");
}

pub mod rdfs {
    //! [RDFS](https://www.w3.org/TR/rdf-schema/) vocabulary.
    use crate::named_node::NamedNodeRef;

    /// The class of classes.
    pub const CLASS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#Class");
    /// A description of the subject resource.
    pub const COMMENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#comment");
    /// The class of RDF containers.
    pub const CONTAINER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#Container");
    /// The class of container membership properties, `rdf:_1`, `rdf:_2`, ..., all of which are sub-properties of `member`.
    pub const CONTAINER_MEMBERSHIP_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked(
        "http://www.w3.org/2000/01/rdf-schema#ContainerMembershipProperty",
    );
    /// The class of RDF datatypes.
    pub const DATATYPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#Datatype");
    /// A domain of the subject property.
    pub const DOMAIN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#domain");
    /// The definition of the subject resource.
    pub const IS_DEFINED_BY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#isDefinedBy");
    /// A human-readable name for the subject.
    pub const LABEL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#label");
    /// The class of literal values, e.g. textual strings and integers.
    pub const LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#Literal");
    /// A member of the subject resource.
    pub const MEMBER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#member");
    /// A range of the subject property.
    pub const RANGE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#range");
    /// The class resource, everything.
    pub const RESOURCE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#Resource");
    /// Further information about the subject resource.
    pub const SEE_ALSO: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#seeAlso");
    /// The subject is a subclass of a class.
    pub const SUB_CLASS_OF: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#subClassOf");
    /// The subject is a subproperty of a property.
    pub const SUB_PROPERTY_OF: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2000/01/rdf-schema#subPropertyOf");
}

pub mod xsd {
    //! [RDF compatible XSD datatypes](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-compatible-xsd-types).
    use crate::named_node::NamedNodeRef;

    /// Absolute or relative URIs and IRIs.
    pub const ANY_URI: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#anyURI");
    /// Base64-encoded binary data.
    pub const BASE_64_BINARY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#base64Binary");
    /// true, false.
    pub const BOOLEAN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#boolean");
    /// 128…+127 (8 bit).
    pub const BYTE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#byte");
    /// Dates (yyyy-mm-dd) with or without timezone.
    pub const DATE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#date");
    /// Duration of time (days, hours, minutes, seconds only).
    pub const DAY_TIME_DURATION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dayTimeDuration");
    /// Date and time with or without timezone.
    pub const DATE_TIME: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTime");
    /// Date and time with required timezone.
    pub const DATE_TIME_STAMP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#dateTimeStamp");
    /// Arbitrary-precision decimal numbers.
    pub const DECIMAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#decimal");
    /// 64-bit floating point numbers incl. ±Inf, ±0, NaN.
    pub const DOUBLE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#double");
    /// Duration of time.
    pub const DURATION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#duration");
    /// 32-bit floating point numbers incl. ±Inf, ±0, NaN.
    pub const FLOAT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#float");
    /// Gregorian calendar day of the month.
    pub const G_DAY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#gDay");
    /// Gregorian calendar month.
    pub const G_MONTH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#gMonth");
    /// Gregorian calendar month and day.
    pub const G_MONTH_DAY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#gMonthDay");
    /// Gregorian calendar year.
    pub const G_YEAR: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#gYear");
    /// Gregorian calendar year and month.
    pub const G_YEAR_MONTH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#gYearMonth");
    /// Hex-encoded binary data.
    pub const HEX_BINARY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#hexBinary");
    /// -2147483648…+2147483647 (32 bit).
    pub const INT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#int");
    /// Arbitrary-size integer numbers.
    pub const INTEGER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#integer");
    /// Language tags per [BCP47](http://tools.ietf.org/html/bcp47).
    pub const LANGUAGE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#language");
    /// -9223372036854775808…+9223372036854775807 (64 bit).
    pub const LONG: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#long");
    /// XML Names.
    pub const NAME: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#Name");
    /// XML NCName.
    pub const NC_NAME: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#NCName");
    /// Integer numbers <0.
    pub const NEGATIVE_INTEGER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#negativeInteger");
    /// XML NMTOKENs.
    pub const NMTOKEN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#NMTOKEN");
    /// Integer numbers ≥0.
    pub const NON_NEGATIVE_INTEGER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#nonNegativeInteger");
    /// Integer numbers ≤0.
    pub const NON_POSITIVE_INTEGER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#nonPositiveInteger");
    /// Whitespace-normalized strings.
    pub const NORMALIZED_STRING: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#normalizedString");
    /// Integer numbers >0.
    pub const POSITIVE_INTEGER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#positiveInteger");
    /// Times (hh:mm:ss.sss…) with or without timezone.
    pub const TIME: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#time");
    /// -32768…+32767 (16 bit).
    pub const SHORT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#short");
    /// Character strings (but not all Unicode character strings).
    pub const STRING: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#string");
    /// Tokenized strings.
    pub const TOKEN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#token");
    /// 0…255 (8 bit).
    pub const UNSIGNED_BYTE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#unsignedByte");
    /// 0…4294967295 (32 bit).
    pub const UNSIGNED_INT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#unsignedInt");
    /// 0…18446744073709551615 (64 bit).
    pub const UNSIGNED_LONG: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#unsignedLong");
    /// 0…65535 (16 bit).
    pub const UNSIGNED_SHORT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#unsignedShort");
    /// Duration of time (months and years only).
    pub const YEAR_MONTH_DURATION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/2001/XMLSchema#yearMonthDuration");
}

pub mod geosparql {
    //! [GeoSpatial](https://opengeospatial.github.io/ogc-geosparql/) vocabulary.
    use crate::named_node::NamedNodeRef;

    /// Geospatial datatype like `"Point({longitude} {latitude})"^^geo:wktLiteral`
    pub const WKT_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.opengis.net/ont/geosparql#wktLiteral");
}

pub mod shacl {
    //! [SHACL](https://www.w3.org/TR/shacl/) vocabulary.
    //!
    //! The Shapes Constraint Language (SHACL) is a W3C specification for validating
    //! RDF graphs against a set of conditions called "shapes".
    use crate::named_node::NamedNodeRef;

    // === NAMESPACE ===
    /// The SHACL namespace: `http://www.w3.org/ns/shacl#`
    pub const NAMESPACE: &str = "http://www.w3.org/ns/shacl#";

    // === SHAPE CLASSES ===
    /// The class of all shapes.
    pub const SHAPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#Shape");
    /// The class of all node shapes.
    pub const NODE_SHAPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#NodeShape");
    /// The class of all property shapes.
    pub const PROPERTY_SHAPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#PropertyShape");

    // === TARGET DECLARATIONS ===
    /// Links a shape to a class whose instances are to be validated.
    pub const TARGET_CLASS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#targetClass");
    /// Links a shape to specific focus nodes.
    pub const TARGET_NODE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#targetNode");
    /// Links a shape to subjects of triples with a specific predicate.
    pub const TARGET_SUBJECTS_OF: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#targetSubjectsOf");
    /// Links a shape to objects of triples with a specific predicate.
    pub const TARGET_OBJECTS_OF: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#targetObjectsOf");

    // === PROPERTY PATH ===
    /// Specifies a property path for a property shape.
    pub const PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#path");
    /// RDF list of alternative paths.
    pub const ALTERNATIVE_PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#alternativePath");
    /// Inverse path operator.
    pub const INVERSE_PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#inversePath");
    /// Zero-or-more path operator.
    pub const ZERO_OR_MORE_PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#zeroOrMorePath");
    /// One-or-more path operator.
    pub const ONE_OR_MORE_PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#oneOrMorePath");
    /// Zero-or-one path operator.
    pub const ZERO_OR_ONE_PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#zeroOrOnePath");

    // === VALUE TYPE CONSTRAINTS ===
    /// Specifies the datatype of all value nodes.
    pub const DATATYPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#datatype");
    /// Specifies the required class of all value nodes.
    pub const CLASS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#class");
    /// Specifies the node kind of all value nodes.
    pub const NODE_KIND: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#nodeKind");

    // === NODE KIND VALUES ===
    /// Node kind: IRI.
    pub const IRI: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#IRI");
    /// Node kind: Literal.
    pub const LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#Literal");
    /// Node kind: BlankNode.
    pub const BLANK_NODE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#BlankNode");
    /// Node kind: BlankNodeOrIRI.
    pub const BLANK_NODE_OR_IRI: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#BlankNodeOrIRI");
    /// Node kind: BlankNodeOrLiteral.
    pub const BLANK_NODE_OR_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#BlankNodeOrLiteral");
    /// Node kind: IRIOrLiteral.
    pub const IRI_OR_LITERAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#IRIOrLiteral");

    // === CARDINALITY CONSTRAINTS ===
    /// Minimum count constraint.
    pub const MIN_COUNT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#minCount");
    /// Maximum count constraint.
    pub const MAX_COUNT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#maxCount");

    // === VALUE RANGE CONSTRAINTS ===
    /// Minimum exclusive value constraint.
    pub const MIN_EXCLUSIVE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#minExclusive");
    /// Maximum exclusive value constraint.
    pub const MAX_EXCLUSIVE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#maxExclusive");
    /// Minimum inclusive value constraint.
    pub const MIN_INCLUSIVE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#minInclusive");
    /// Maximum inclusive value constraint.
    pub const MAX_INCLUSIVE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#maxInclusive");

    // === STRING CONSTRAINTS ===
    /// Minimum length constraint.
    pub const MIN_LENGTH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#minLength");
    /// Maximum length constraint.
    pub const MAX_LENGTH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#maxLength");
    /// Regular expression pattern constraint.
    pub const PATTERN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#pattern");
    /// Regex flags for pattern matching.
    pub const FLAGS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#flags");
    /// Allowed language tags constraint.
    pub const LANGUAGE_IN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#languageIn");
    /// Unique language constraint.
    pub const UNIQUE_LANG: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#uniqueLang");

    // === PROPERTY PAIR CONSTRAINTS ===
    /// Equal property values constraint.
    pub const EQUALS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#equals");
    /// Disjoint property values constraint.
    pub const DISJOINT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#disjoint");
    /// Less than property values constraint.
    pub const LESS_THAN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#lessThan");
    /// Less than or equals property values constraint.
    pub const LESS_THAN_OR_EQUALS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#lessThanOrEquals");

    // === LOGICAL CONSTRAINTS ===
    /// Negation constraint.
    pub const NOT: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#not");
    /// Conjunction constraint (all shapes must match).
    pub const AND: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#and");
    /// Disjunction constraint (at least one shape must match).
    pub const OR: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#or");
    /// Exclusive disjunction constraint (exactly one shape must match).
    pub const XONE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#xone");

    // === SHAPE-BASED CONSTRAINTS ===
    /// Links to a property shape.
    pub const PROPERTY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#property");
    /// Validates value nodes against a shape.
    pub const NODE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#node");
    /// Qualified value shape constraint.
    pub const QUALIFIED_VALUE_SHAPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#qualifiedValueShape");
    /// Qualified minimum count constraint.
    pub const QUALIFIED_MIN_COUNT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#qualifiedMinCount");
    /// Qualified maximum count constraint.
    pub const QUALIFIED_MAX_COUNT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#qualifiedMaxCount");
    /// Qualified value shapes disjoint constraint.
    pub const QUALIFIED_VALUE_SHAPES_DISJOINT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#qualifiedValueShapesDisjoint");

    // === OTHER CONSTRAINTS ===
    /// Closed shape constraint.
    pub const CLOSED: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#closed");
    /// Properties to ignore in closed shapes.
    pub const IGNORED_PROPERTIES: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ignoredProperties");
    /// Has value constraint.
    pub const HAS_VALUE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#hasValue");
    /// In allowed values list constraint.
    pub const IN: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#in");

    // === VALIDATION REPORT ===
    /// The class of validation reports.
    pub const VALIDATION_REPORT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ValidationReport");
    /// The class of validation results.
    pub const VALIDATION_RESULT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ValidationResult");
    /// Indicates overall conformance.
    pub const CONFORMS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#conforms");
    /// Links a report to its results.
    pub const RESULT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#result");
    /// The focus node that caused a validation result.
    pub const FOCUS_NODE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#focusNode");
    /// The path that caused a validation result.
    pub const RESULT_PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#resultPath");
    /// The value that caused a validation result.
    pub const VALUE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#value");
    /// The source shape of a validation result.
    pub const SOURCE_SHAPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#sourceShape");
    /// The source constraint component of a validation result.
    pub const SOURCE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#sourceConstraintComponent");
    /// Human-readable message for a validation result.
    pub const RESULT_MESSAGE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#resultMessage");
    /// The severity of a validation result.
    pub const RESULT_SEVERITY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#resultSeverity");
    /// Nested validation results.
    pub const DETAIL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#detail");

    // === SEVERITY LEVELS ===
    /// Violation severity level.
    pub const VIOLATION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#Violation");
    /// Warning severity level.
    pub const WARNING: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#Warning");
    /// Info severity level.
    pub const INFO: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#Info");

    // === SHAPE METADATA ===
    /// Human-readable name for a shape or property.
    pub const NAME: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#name");
    /// Human-readable description for a shape or property.
    pub const DESCRIPTION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#description");
    /// Custom validation message.
    pub const MESSAGE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#message");
    /// Custom severity level for a shape.
    pub const SEVERITY: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#severity");
    /// Deactivates a shape.
    pub const DEACTIVATED: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#deactivated");
    /// Property ordering hint.
    pub const ORDER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#order");
    /// Property grouping.
    pub const GROUP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#group");
    /// Default value.
    pub const DEFAULT_VALUE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#defaultValue");

    // === SPARQL CONSTRAINTS ===
    /// SPARQL-based constraint.
    pub const SPARQL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#sparql");
    /// SPARQL SELECT query for constraint.
    pub const SELECT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#select");
    /// SPARQL ASK query for constraint.
    pub const ASK: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ask");
    /// Prefixes for SPARQL queries.
    pub const PREFIXES: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#prefixes");
    /// Prefix declaration.
    pub const DECLARE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#declare");
    /// Prefix name.
    pub const PREFIX: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#prefix");
    /// Namespace for prefix.
    pub const NAMESPACE_PROP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#namespace");

    // === CONSTRAINT COMPONENTS ===
    /// The class of constraint components.
    pub const CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ConstraintComponent");
    /// Parameter for a constraint component.
    pub const PARAMETER: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#parameter");
    /// Validator for constraint component.
    pub const VALIDATOR: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#validator");
    /// Node validator for constraint component.
    pub const NODE_VALIDATOR: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#nodeValidator");
    /// Property validator for constraint component.
    pub const PROPERTY_VALIDATOR: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#propertyValidator");
    /// Optional parameter.
    pub const OPTIONAL: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#optional");

    // === BUILT-IN CONSTRAINT COMPONENT IRIS ===
    /// Class constraint component.
    pub const CLASS_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ClassConstraintComponent");
    /// Datatype constraint component.
    pub const DATATYPE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#DatatypeConstraintComponent");
    /// Node kind constraint component.
    pub const NODE_KIND_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#NodeKindConstraintComponent");
    /// Min count constraint component.
    pub const MIN_COUNT_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MinCountConstraintComponent");
    /// Max count constraint component.
    pub const MAX_COUNT_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MaxCountConstraintComponent");
    /// Min exclusive constraint component.
    pub const MIN_EXCLUSIVE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MinExclusiveConstraintComponent");
    /// Max exclusive constraint component.
    pub const MAX_EXCLUSIVE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MaxExclusiveConstraintComponent");
    /// Min inclusive constraint component.
    pub const MIN_INCLUSIVE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MinInclusiveConstraintComponent");
    /// Max inclusive constraint component.
    pub const MAX_INCLUSIVE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MaxInclusiveConstraintComponent");
    /// Min length constraint component.
    pub const MIN_LENGTH_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MinLengthConstraintComponent");
    /// Max length constraint component.
    pub const MAX_LENGTH_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MaxLengthConstraintComponent");
    /// Pattern constraint component.
    pub const PATTERN_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#PatternConstraintComponent");
    /// Language in constraint component.
    pub const LANGUAGE_IN_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#LanguageInConstraintComponent");
    /// Unique lang constraint component.
    pub const UNIQUE_LANG_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#UniqueLangConstraintComponent");
    /// Equals constraint component.
    pub const EQUALS_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#EqualsConstraintComponent");
    /// Disjoint constraint component.
    pub const DISJOINT_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#DisjointConstraintComponent");
    /// Less than constraint component.
    pub const LESS_THAN_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#LessThanConstraintComponent");
    /// Less than or equals constraint component.
    pub const LESS_THAN_OR_EQUALS_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked(
            "http://www.w3.org/ns/shacl#LessThanOrEqualsConstraintComponent",
        );
    /// Not constraint component.
    pub const NOT_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#NotConstraintComponent");
    /// And constraint component.
    pub const AND_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#AndConstraintComponent");
    /// Or constraint component.
    pub const OR_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#OrConstraintComponent");
    /// Xone constraint component.
    pub const XONE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#XoneConstraintComponent");
    /// Node constraint component.
    pub const NODE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#NodeConstraintComponent");
    /// Property constraint component.
    pub const PROPERTY_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#PropertyConstraintComponent");
    /// Qualified value shape constraint component.
    pub const QUALIFIED_VALUE_SHAPE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked(
            "http://www.w3.org/ns/shacl#QualifiedValueShapeConstraintComponent",
        );
    /// Closed constraint component.
    pub const CLOSED_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#ClosedConstraintComponent");
    /// Has value constraint component.
    pub const HAS_VALUE_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#HasValueConstraintComponent");
    /// In constraint component.
    pub const IN_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#InConstraintComponent");
    /// SPARQL constraint component.
    pub const SPARQL_CONSTRAINT_COMPONENT: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#SPARQLConstraintComponent");

    // === SHAPES GRAPH WELL-FORMEDNESS ===
    /// Indicates the shapes graph is well-formed.
    pub const SHAPES_GRAPH_WELL_FORMED: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#shapesGraphWellFormed");

    // === SPARQL TARGET ===
    /// SPARQL-based target.
    pub const SPARQL_TARGET: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#SPARQLTarget");
    /// SPARQL target type.
    pub const SPARQL_TARGET_TYPE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#SPARQLTargetType");
    /// Target property.
    pub const TARGET: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#target");
}
