/// [SPARQL](https://www.w3.org/ns/sparql) vocabulary.
pub mod sparql {
    use oxrdf::NamedNode;

    /// This operator adds two numeric expressions and returns their sum.
    pub const ADD: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#add");
    /// This operator subtracts the second numeric expression from the first and returns the result.
    pub const SUBTRACT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#subtract");
    /// This operator multiplies two numeric expressions and returns the product.
    pub const MULTIPLY: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#multiply");
    /// This operator divides the first numeric expression by the second and returns the result.
    pub const DIVIDE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#divide");
    /// This unary operator returns the negation of a numeric expression.
    pub const UNARY_MINUS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#unary-minus");
    /// This unary operator returns the numeric expression unchanged, acting primarily as a syntactic indicator.
    pub const UNARY_PLUS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#unary-plus");
    /// This operator compares two expressions for equality.
    pub const EQUALS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#equals");
    /// This operator tests two expressions for inequality.
    pub const NOT_EQUALS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#not-equals");
    /// This operator tests whether the first RDF term is greater than the second RDF term.
    pub const GREATER_THAN: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#greater-than");
    /// This operator tests whether the first RDF term is less than the second RDF term.
    pub const LESS_THAN: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#less-than");
    /// This operator tests whether the first RDF term is greater or equal to the second RDF term.
    pub const GREATER_THAN_OR_EQUAL: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#greater-than-or-equal");
    /// This operator tests whether the first RDF term is less than or equal to the second RDF term.
    pub const LESS_THAN_OR_EQUAL: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#less-than-or-equal");

    /// This form checks whether a variable is bound (assigned a value) in the current solution.
    pub const BOUND: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#bound");
    /// This conditional form evaluates a test expression and returns one of two provided expressions based on the boolean outcome of the test.
    pub const IF: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#if");
    /// This form returns the first non-error, non-unbound value from a sequence of expressions.
    pub const COALESCE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#coalesce");
    /// This form tests whether a given pattern exists for each solution.
    pub const FILTER_EXISTS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#filter-exists");
    /// This form tests whether a given pattern does not exist for each solution.
    pub const FILTER_NOT_EXISTS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#filter-not-exists");
    /// This form computes the logical OR of two boolean expressions.
    pub const LOGICAL_OR: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#logical-or");
    /// This form computes the logical AND of two boolean expressions.
    pub const LOGICAL_AND: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#logical-and");
    /// This form computes the logical NOT of a boolean expression.
    pub const LOGICAL_NOT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#logical-not");
    /// This form computes the effective boolean value.
    pub const EBV: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#ebv");
    /// This form checks whether a given value matches any value from a list of expressions.
    pub const IN: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#in");
    /// This form returns true if the value is not found, or false if the value is found, in the list of expressions.
    pub const NOT_IN: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#not-in");

    /// This function checks whether two RDF terms are the same in the strict sense, including their lexical forms, datatypes, and language tags for literals.
    pub const SAME_TERM: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#sameTerm");
    /// This function compares two RDF terms for equivalent RDF values, potentially considering numeric type equivalencies and other canonical forms beyond strict term identity.
    pub const SAME_VALUE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#sameValue");
    /// Deprecated
    pub const RDF_TERM_EQUAL: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#RDFterm-equal");
    /// This function returns true if the provided term is an IRI, and false otherwise.
    pub const IS_IRI: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#isIRI");
    /// This function returns true if the provided term is an IRI, and false otherwise.
    pub const IS_URI: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#isURI");
    /// This function returns true if the provided term is a blank node, and false otherwise.
    pub const IS_BLANK: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#isBlank");
    /// This function returns true if the provided term is an RDF literal, and false otherwise.
    pub const IS_LITERAL: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#isLiteral");
    /// This function returns true if the provided term is a numeric literal (e.g., xsd:integer, xsd:decimal, xsd:float, or xsd:double), and false otherwise.
    pub const IS_NUMERIC: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#isNumeric");
    /// This function returns the lexical form of an RDF term, which for IRIs is the IRI string, and for literals is the lexical representation.
    pub const STR: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#str");
    /// This function returns the language tag of a literal, or an empty string if no language tag is present or the term is not a literal.
    pub const LANG: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#lang");
    /// This function returns the initial text direction of a literal.
    #[cfg(feature = "sparql-12")]
    pub const LANGDIR: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#langdir");
    /// This function returns true if the given RDF literal has a specified language, matching the literal’s language tag.
    #[cfg(feature = "sparql-12")]
    pub const HAS_LANG: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#hasLang");
    /// This function returns true if the given RDF literal has an initial text direction.
    #[cfg(feature = "sparql-12")]
    pub const HAS_LANGDIR: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#hasLangdir");
    /// This function returns the datatype IRI of a literal term.
    pub const DATATYPE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#datatype");
    /// This function returns an IRI with the given string.
    pub const IRI: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#iri");
    /// This function returns an IRI with the given string.
    pub const URI: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#uri");
    /// This function returns a blank node.
    pub const BNODE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#bnode");
    /// This function creates a typed literal from a string and a datatype IRI, returning an RDF literal with the specified lexical form and datatype.
    pub const STRDT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strdt");
    /// This function creates an RDF literal with the specified lexical form and language tag.
    pub const STRLANG: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strlang");
    /// This function creates an RDF literal with language tag and initial text direction.
    #[cfg(feature = "sparql-12")]
    pub const STRLANGDIR: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strlangdir");
    /// This function generates a UUID as an IRI.
    pub const UUID: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#uuid");
    /// This function generates a UUID as a string.
    pub const STRUUID: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#struuid");

    /// This function returns the length of the lexical form of a string literal, measured in characters.
    pub const STRLEN: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strlen");
    /// This function returns the substring of the given string starting at a specified position and optionally limited to a given length.
    pub const SUBSTR: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#substr");
    /// This function transforms all alphabetic characters in the input string to uppercase, following Unicode case-folding conventions
    pub const UCASE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#ucase");
    /// This function transforms all alphabetic characters in the input string to lowercase, according to Unicode case-folding rules.
    pub const LCASE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#lcase");
    /// This function returns true if the first string argument begins with the second string argument, and false otherwise.
    pub const STRSTARTS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strstarts");
    /// This function returns true if the first string argument ends with the second string argument, and false otherwise.
    pub const STRENDS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strends");
    /// This function returns true if the first string argument contains the second string argument as a substring, and false otherwise.
    pub const CONTAINS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#contains");
    /// This function returns the substring of the first argument that precedes the first occurrence of the second argument.
    pub const STRBEFORE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strbefore");
    /// This function returns the substring of the first argument that follows the first occurrence of the second argument.
    pub const STRAFTER: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#strafter");
    /// This function concatenates two or more string literals into one continuous string.
    pub const CONCAT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#concat");
    /// This function checks whether a given language tag matches a specified language range.
    pub const LANG_MATCHES: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#langMatches");
    /// This function tests whether a string matches a regular expression pattern, optionally with a specified flag (e.g., i for case-insensitive).
    pub const REGEX: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#regex");
    /// This function performs a regular expression search-and-replace on a string, returning the modified string.
    pub const REPLACE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#replace");
    /// This function encodes a string using a specified method (here, URI-encoding), returning the encoded version.
    pub const ENCODE_FOR_URI: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#encodeForUri");

    /// This function returns the absolute value of a numeric argument.
    pub const ABS: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#abs");
    /// This function rounds a numeric argument to the nearest integer.
    pub const ROUND: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#round");
    /// This function returns the smallest integer greater than or equal to the numeric argument.
    pub const CEIL: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#ceil");
    /// This function returns the greatest integer less than or equal to the numeric argument.
    pub const FLOOR: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#floor");
    /// This function returns a random number between 0 and 1.
    pub const RAND: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#rand");

    /// This function returns the current dateTime (with or without a timezone) at the moment of query execution.
    pub const NOW: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#now");
    /// This function returns the year component of an xsd:dateTime or xsd:date.
    pub const YEAR: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#year");
    /// This function returns the month component of an xsd:dateTime or xsd:date.
    pub const MONTH: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#month");
    /// This function returns the day component of an xsd:dateTime or xsd:date.
    pub const DAY: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#day");
    /// This function returns the hour component (0–23) of an xsd:dateTime value.
    pub const HOURS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#hours");
    /// This function returns the minute component (0–59) of an xsd:dateTime value.
    pub const MINUTES: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#minutes");
    /// This function returns the second component (0–60, including leap seconds) of an xsd:dateTime value.
    pub const SECONDS: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#seconds");
    /// This function returns the timezone component as a dayTimeDuration for an xsd:dateTime value with a specified time zone, or an empty value if none.
    pub const TIMEZONE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#timezone");
    /// This function returns the timezone component as a string in ISO 8601 format if present in the xsd:dateTime value, or an empty string otherwise.
    pub const TZ: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#tz");

    /// This function constructs a triple term.
    #[cfg(feature = "sparql-12")]
    pub const TRIPLE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#triple");
    /// This function returns the subject of a triple term.
    #[cfg(feature = "sparql-12")]
    pub const SUBJECT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#subject");
    /// This function returns the predicate of a triple term.
    #[cfg(feature = "sparql-12")]
    pub const PREDICATE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#predicate");
    /// This function returns the object of a triple term.
    #[cfg(feature = "sparql-12")]
    pub const OBJECT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#object");
    /// This function returns true if the argument is a triple term, and false otherwise.
    #[cfg(feature = "sparql-12")]
    pub const IS_TRIPLE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#isTriple");

    /// This function computes the MD5 hash of the lexical form of a string, returning a hexadecimal string representation of the hash.
    pub const MD5: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#md5");
    /// This function computes the SHA-1 hash of the lexical form of a string, returning a hexadecimal string representation of the result.
    pub const SHA1: NamedNode = NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#sha1");
    /// This function computes the SHA-256 hash of the lexical form of a string, returning the resulting hash as a hexadecimal string.
    pub const SHA256: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#sha256");
    /// This function computes the SHA-384 hash of the lexical form of a string, returning the resulting hexadecimal string.
    pub const SHA384: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#sha384");
    /// This function computes the SHA-512 hash of the lexical form of a string, returning the result as a hexadecimal string.
    pub const SHA512: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#sha512");

    /// This function adjusts a date/time value to a timezone.
    #[cfg(feature = "sep-0002")]
    pub const ADJUST: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#adjust");

    /// Aggregate function COUNT
    pub const AGG_COUNT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-count");
    /// Aggregate function SUM
    pub const AGG_SUM: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-sum");
    /// Aggregate function MIN
    pub const AGG_MIN: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-min");
    /// Aggregate function MAX
    pub const AGG_MAX: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-max");
    /// Aggregate function AVG
    pub const AGG_AVG: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-avg");
    /// Aggregate function SAMPLE
    pub const AGG_SAMPLE: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-sample");
    /// Aggregate function GROUP_CONCAT
    pub const AGG_GROUP_CONCAT: NamedNode =
        NamedNode::new_const_unchecked("http://www.w3.org/ns/sparql#agg-group-concat");
}
