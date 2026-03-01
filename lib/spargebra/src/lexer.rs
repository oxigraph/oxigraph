//! Lexer for the SPARQL grammar i.e. ALL_CAPS rules

// TODO: do unescaping

use chumsky::prelude::*;
use chumsky::text::ascii::ident;
use std::fmt;

pub fn lex_sparql(slice: &str) -> Result<Vec<Token<'_>>, Vec<Rich<'_, char>>> {
    build_lexer().parse(slice).into_result()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    IriRef(&'a str),
    PnameLn(&'a str, &'a str),
    PnameNs(&'a str),
    BlankNodeLabel(&'a str),
    StringLiteral1(&'a str),
    StringLiteral2(&'a str),
    StringLiteralLong1(&'a str),
    StringLiteralLong2(&'a str),
    LangDir(&'a str, #[cfg(feature = "sparql-12")] Option<&'a str>),
    Integer(&'a str),
    Decimal(&'a str),
    Double(&'a str),
    IntegerPositive(&'a str),
    DecimalPositive(&'a str),
    DoublePositive(&'a str),
    IntegerNegative(&'a str),
    DecimalNegative(&'a str),
    DoubleNegative(&'a str),
    Var1(&'a str),
    Var2(&'a str),
    Keyword(&'a str),
    Operator(&'a str),
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IriRef(iri) => write!(f, "<{iri}>"),
            Self::PnameLn(k, v) => write!(f, "{k}:{v}"),
            Self::PnameNs(k) => write!(f, "{k}:"),
            Self::BlankNodeLabel(id) => write!(f, "_:{id}"),
            Self::StringLiteral1(s) => write!(f, "'{s}'"),
            Self::StringLiteral2(s) => write!(f, "\"{s}\""),
            Self::StringLiteralLong1(s) => write!(f, "'''{s}'''"),
            Self::StringLiteralLong2(s) => write!(f, "\"\"\"{s}\"\"\""),
            #[cfg(feature = "sparql-12")]
            Self::LangDir(lang, dir) => {
                if let Some(dir) = dir {
                    write!(f, "@{lang}--{dir}")
                } else {
                    write!(f, "@{lang}")
                }
            }
            #[cfg(not(feature = "sparql-12"))]
            Self::LangDir(lang) => write!(f, "@{lang}"),
            Self::Var1(v) => write!(f, "?{v}"),
            Self::Var2(v) => write!(f, "${v}"),
            Self::Integer(v)
            | Self::Decimal(v)
            | Self::Double(v)
            | Self::IntegerPositive(v)
            | Self::DecimalPositive(v)
            | Self::DoublePositive(v)
            | Self::IntegerNegative(v)
            | Self::DecimalNegative(v)
            | Self::DoubleNegative(v)
            | Self::Keyword(v)
            | Self::Operator(v) => f.write_str(v),
        }
    }
}

fn build_lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Token<'src>>, extra::Err<Rich<'src, char, SimpleSpan>>> {
    // [193]   	PN_LOCAL_ESC 	  ::=   	'\' ( '_' | '~' | '.' | '-' | '!' | '$' | '&' | "'" | '(' | ')' | '*' | '+' | ',' | ';' | '=' | '/' | '?' | '#' | '@' | '%' )
    let pn_local_esc = just('\\').then(one_of([
        '_', '~', '.', '-', '!', '$', '&', '\'', '(', ')', '*', '+', ',', ';', '=', '/', '?', '#',
        '@', '%',
    ]));

    // [192]   	HEX 	  ::=   	[0-9] | [A-F] | [a-f]
    let hex = one_of('0'..='9')
        .or(one_of('A'..='F'))
        .or(one_of('a'..='f'));

    // [191]   	PERCENT 	  ::=   	'%' HEX HEX
    let percent = just('%').then(hex.clone()).then(hex);

    // [190]   	PLX 	  ::=   	PERCENT | PN_LOCAL_ESC
    let plx = percent.ignored().or(pn_local_esc.ignored());

    // [184]   	PN_CHARS_BASE 	  ::=   	[A-Z] | [a-z] | [#x00C0-#x00D6] | [#x00D8-#x00F6] | [#x00F8-#x02FF] | [#x0370-#x037D] | [#x037F-#x1FFF] | [#x200C-#x200D] | [#x2070-#x218F] | [#x2C00-#x2FEF] | [#x3001-#xD7FF] | [#xF900-#xFDCF] | [#xFDF0-#xFFFD] | [#x10000-#xEFFFF]
    let pn_chars_base = any().filter(|c| {
        // TODO: use the same kind of matching for the other variable ranges?
        matches!(c,
        'A'..='Z'
        | 'a'..='z'
        | '\u{00C0}'..='\u{00D6}'
        | '\u{00D8}'..='\u{00F6}'
        | '\u{00F8}'..='\u{02FF}'
        | '\u{0370}'..='\u{037D}'
        | '\u{037F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}')
    });

    // [185]   	PN_CHARS_U 	  ::=   	PN_CHARS_BASE | '_'
    let pn_chars_u = pn_chars_base.or(just('_'));

    // [187]   	PN_CHARS 	  ::=   	PN_CHARS_U | '-' | [0-9] | #x00B7 | [#x0300-#x036F] | [#x203F-#x2040]
    let pn_chars = pn_chars_u
        .or(just('-'))
        .or(one_of('0'..='9'))
        .or(one_of('\u{00B7}'))
        .or(one_of('\u{0300}'..='\u{036F}'))
        .or(one_of('\u{203F}'..='\u{2040}'));

    // [189]   	PN_LOCAL 	  ::=   	(PN_CHARS_U | ':' | [0-9] | PLX ) ((PN_CHARS | '.' | ':' | PLX)* (PN_CHARS | ':' | PLX) )?
    let pn_local = pn_chars_u
        .or(just(':'))
        .or(one_of('0'..='9'))
        .ignored()
        .or(plx.clone())
        .then(
            pn_chars
                .clone()
                .or(just('.'))
                .or(just(':'))
                .ignored()
                .or(plx.clone())
                .repeated()
                .then(pn_chars.clone().or(just(':')).ignored().or(plx))
                .or_not(),
        )
        .to_slice();

    // [188]   	PN_PREFIX 	  ::=   	PN_CHARS_BASE ((PN_CHARS|'.')* PN_CHARS)?
    let pn_prefix = pn_chars_base.then(pn_chars.clone().repeated().separated_by(just('.')));

    // [186]   	VARNAME 	  ::=   	( PN_CHARS_U | [0-9] ) ( PN_CHARS_U | [0-9] | #x00B7 | [#x0300-#x036F] | [#x203F-#x2040] )*
    let varname = pn_chars_u
        .or(one_of('0'..='9'))
        .then(
            pn_chars_u
                .or(one_of('0'..='9'))
                .or(one_of('\u{00B7}'))
                .or(one_of('\u{0300}'..='\u{036F}'))
                .or(one_of('\u{203F}'..='\u{2040}'))
                .repeated(),
        )
        .to_slice();

    // [180]   	ECHAR 	  ::=   	'\' [tbnrf\"']
    let echar = just('\\').then(one_of(['t', 'b', 'n', 'r', 'f', '"', '\'', '\\']));

    // [179]   	STRING_LITERAL_LONG2 	  ::=   	'"""' ( ( '"' | '""' )? ( [^"\] | ECHAR ) )* '"""'
    let string_literal_long2 = just("\"\"\"")
        .ignore_then(
            just('"')
                .then(just('"').or_not())
                .or_not()
                .then(none_of(['"', '\\']).ignored().or(echar.ignored()))
                .repeated()
                .to_slice(),
        )
        .then_ignore(just("\"\"\""))
        .map(Token::StringLiteralLong2);

    // [178]   	STRING_LITERAL_LONG1 	  ::=   	"'''" ( ( "'" | "''" )? ( [^'\] | ECHAR ) )* "'''"
    let string_literal_long1 = just("'''")
        .ignore_then(
            just('\'')
                .then(just('\'').or_not())
                .or_not()
                .then(none_of(['\'', '\\']).ignored().or(echar.ignored()))
                .repeated()
                .to_slice(),
        )
        .then_ignore(just("'''"))
        .map(Token::StringLiteralLong1);

    // [177]   	STRING_LITERAL2 	  ::=   	'"' ( ([^#x22#x5C#xA#xD]) | ECHAR )* '"'
    let string_literal2 = just('"')
        .ignore_then(
            none_of(['\x22', '\x5C', '\x0A', '\x0D'])
                .ignored()
                .or(echar.ignored())
                .repeated()
                .to_slice(),
        )
        .then_ignore(just('"'))
        .map(Token::StringLiteral2);

    // [176]   	STRING_LITERAL1 	  ::=   	"'" ( ([^#x27#x5C#xA#xD]) | ECHAR )* "'"
    let string_literal1 = just('\'')
        .ignore_then(
            none_of(['\x27', '\x5C', '\x0A', '\x0D'])
                .ignored()
                .or(echar.ignored())
                .repeated()
                .to_slice(),
        )
        .then_ignore(just('\''))
        .map(Token::StringLiteral1);

    // [169]   	EXPONENT 	  ::=   	[eE] [+-]? [0-9]+
    let exponent = one_of(['e', 'E'])
        .then(one_of(['+', '=']).or_not())
        .then(one_of('0'..='9').repeated().at_least(1));

    // [168]   	DOUBLE 	  ::=   	( ([0-9]+ ('.'[0-9]*)? ) | ( '.' ([0-9])+ ) ) EXPONENT
    let double = one_of('0'..='9')
        .repeated()
        .at_least(1)
        .then(just('.').then(one_of('0'..='9').repeated()).or_not())
        .ignored()
        .or(just('.')
            .then(one_of('0'..='9').repeated().at_least(1))
            .ignored())
        .then(exponent)
        .to_slice()
        .map(Token::Double);

    // [167]   	DECIMAL 	  ::=   	[0-9]* '.' [0-9]+
    let decimal = one_of('0'..='9')
        .repeated()
        .then(just('.'))
        .then(one_of('0'..='9').repeated().at_least(1))
        .to_slice()
        .map(Token::Decimal);

    // [166]   	INTEGER 	  ::=   	[0-9]+
    let integer = one_of('0'..='9')
        .repeated()
        .at_least(1)
        .to_slice()
        .map(Token::Integer);

    // [175]   	DOUBLE_NEGATIVE 	  ::=   	'-' DOUBLE
    let double_negative = just('-')
        .then(double.clone())
        .to_slice()
        .map(Token::DoubleNegative);

    // [174]   	DECIMAL_NEGATIVE 	  ::=   	'-' DECIMAL
    let decimal_negative = just('-')
        .then(decimal.clone())
        .to_slice()
        .map(Token::DecimalNegative);

    // [173]   	INTEGER_NEGATIVE 	  ::=   	'-' INTEGER
    let integer_negative = just('-')
        .then(integer.clone())
        .to_slice()
        .map(Token::IntegerNegative);

    // [172]   	DOUBLE_POSITIVE 	  ::=   	'+' DOUBLE
    let double_positive = just('+')
        .then(double.clone())
        .to_slice()
        .map(Token::DoublePositive);

    // [171]   	DECIMAL_POSITIVE 	  ::=   	'+' DECIMAL
    let decimal_positive = just('+')
        .then(decimal.clone())
        .to_slice()
        .map(Token::DecimalPositive);

    // [170]   	INTEGER_POSITIVE 	  ::=   	'+' INTEGER
    let integer_positive = just('+')
        .then(integer.clone())
        .to_slice()
        .map(Token::IntegerPositive);

    // [165]   	LANG_DIR 	  ::=   	'@' [a-zA-Z]+ ('-' [a-zA-Z0-9]+)* ('--' [a-zA-Z]+)?
    let lang_dir = just('@').ignore_then(
        one_of('a'..='z')
            .or(one_of('A'..='A'))
            .repeated()
            .at_least(1)
            .then(
                one_of('a'..='z')
                    .or(one_of('A'..='A'))
                    .or(one_of('0'..='9'))
                    .repeated()
                    .at_least(1)
                    .separated_by(just('-')),
            )
            .to_slice(),
    );
    #[cfg(feature = "sparql-12")]
    let lang_dir = lang_dir
        .then(
            just("--")
                .ignore_then(
                    one_of('a'..='z')
                        .or(one_of('A'..='A'))
                        .repeated()
                        .at_least(1)
                        .to_slice(),
                )
                .or_not(),
        )
        .map(|(l, d)| Token::LangDir(l, d));
    #[cfg(not(feature = "sparql-12"))]
    let lang_dir = lang_dir.map(Token::LangDir);

    // [164]   	VAR2 	  ::=   	'$' VARNAME
    let var2 = just('$').ignore_then(varname.clone()).map(Token::Var2);

    // [163]   	VAR1 	  ::=   	'?' VARNAME
    let var1 = just('?').ignore_then(varname).map(Token::Var1);
    let var = var1.or(var2).labelled("variable like ?foo or $foo");

    // [162]   	BLANK_NODE_LABEL 	  ::=   	'_:' ( PN_CHARS_U | [0-9] ) ((PN_CHARS|'.')* PN_CHARS)?
    let blank_node_label = just("_:")
        .ignore_then(
            pn_chars_u
                .or(one_of('0'..='9'))
                .then(
                    pn_chars
                        .clone()
                        .repeated()
                        .at_least(1)
                        .separated_by(just('.')),
                )
                .to_slice(),
        )
        .map(Token::BlankNodeLabel);

    // [160]   	PNAME_NS 	  ::=   	PN_PREFIX? ':'
    let pname_ns = pn_prefix
        .or_not()
        .to_slice()
        .then_ignore(just(':'))
        .map(Token::PnameNs);

    // [161]   	PNAME_LN 	  ::=   	PNAME_NS PN_LOCAL
    let pname_ln = pname_ns
        .clone()
        .to_slice()
        .then(pn_local)
        .map(|(k, v): (&str, _)| Token::PnameLn(&k[..k.len() - 1], v));

    // [159]   	IRIREF 	  ::=   	'<' ([^<>"{}|^`\]-[#x00-#x20])* '>'
    // TODO: We do not validate the content with chumsky because we validate the IRI after, change that?
    let iri_ref = just('<')
        .ignore_then(none_of(['>']).repeated().to_slice())
        .then_ignore(just('>'))
        .map(Token::IriRef);

    let keyword = ident().map(Token::Keyword);

    let operator = just("||")
        .or(just("&&"))
        .or(just("^^"))
        .or(just("<="))
        .or(just(">="))
        .or(just("!="))
        .or(just("<<"))
        .or(just(">>"))
        .or(just("{|"))
        .or(just("|}"))
        .or(one_of([
            '{', '}', '[', ']', '(', ')', ';', ',', '.', '+', '-', '*', '/', '<', '>', '=', '!',
            '^', '|', '~',
        ])
        .to_slice())
        .map(Token::Operator);

    let token = iri_ref
        .or(blank_node_label)
        .or(lang_dir)
        .or(var)
        .or(string_literal_long1)
        .or(string_literal_long2)
        .or(string_literal1)
        .or(string_literal2)
        .or(double_positive)
        .or(decimal_positive)
        .or(integer_positive)
        .or(double_negative)
        .or(decimal_negative)
        .or(integer_negative)
        .or(double)
        .or(decimal)
        .or(integer)
        .or(pname_ln)
        .or(pname_ns)
        .or(keyword)
        .or(operator);

    let comment = just('#').then(just('\n').not().repeated()).padded();

    token
        .padded_by(comment.repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}
