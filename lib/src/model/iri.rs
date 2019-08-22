use std::error::Error;
use std::fmt;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Iri {
    iri: String,
    positions: IriElementsPositions,
}

impl Iri {
    pub fn parse(iri: String) -> Result<Self, IriParseError> {
        let base_positions =
            parse_iri(iri.as_bytes(), 0).map_err(|position| IriParseError { position })?;
        Ok(Self {
            iri,
            positions: base_positions,
        })
    }

    pub fn resolve(&self, iri: &str) -> Result<Iri, IriParseError> {
        let mut target_buffer = String::with_capacity(self.iri.len() + iri.len());
        let positions = resolve_relative_iri(iri, &self.iri, &self.positions, &mut target_buffer)
            .map_err(|position| IriParseError { position })?;
        Ok(Self {
            iri: target_buffer,
            positions,
        })
    }

    pub fn into_string(self) -> String {
        self.iri
    }
}

#[derive(Debug)]
pub struct IriParseError {
    position: usize,
}

impl fmt::Display for IriParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid IRI at char {}", self.position)
    }
}

impl Error for IriParseError {}

type IriState = Result<usize, usize>; // usize = the end position

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct IriElementsPositions {
    scheme_end: usize,
    authority_end: usize,
    path_end: usize,
    query_end: usize,
    fragment_end: usize,
}

// RFC 3986 5.2 Relative Resolution algorithm
fn resolve_relative_iri(
    reference_iri: &str,
    base_iri: &str,
    base_positions: &IriElementsPositions,
    target_buffer: &mut String,
) -> Result<IriElementsPositions, usize> {
    let base_scheme = &base_iri[0..base_positions.scheme_end];
    let base_authority = &base_iri[base_positions.scheme_end..base_positions.authority_end];
    let base_path = &base_iri[base_positions.authority_end..base_positions.path_end];
    let base_query = &base_iri[base_positions.path_end..base_positions.query_end];

    let reference_positions = parse_iri_reference(reference_iri.as_bytes(), 0)?;
    let r_scheme = &reference_iri[0..reference_positions.scheme_end];
    let r_authority =
        &reference_iri[reference_positions.scheme_end..reference_positions.authority_end];
    let r_path = &reference_iri[reference_positions.authority_end..reference_positions.path_end];
    let r_query = &reference_iri[reference_positions.path_end..reference_positions.query_end];
    let r_fragment = &reference_iri[reference_positions.query_end..];

    let scheme_end;
    let authority_end;
    let path_end;
    let query_end;
    let fragment_end;

    // if defined(R.scheme) then
    if !r_scheme.is_empty() {
        // T.scheme = R.scheme;
        target_buffer.push_str(r_scheme);
        scheme_end = target_buffer.len();

        // T.authority = R.authority;
        target_buffer.push_str(r_authority);
        authority_end = target_buffer.len();

        // T.path = remove_dot_segments(R.path);
        append_and_remove_dot_segments(r_path, target_buffer, target_buffer.len());
        path_end = target_buffer.len();

        // T.query = R.query;
        target_buffer.push_str(r_query);
        query_end = target_buffer.len();

        // T.fragment = R.fragment;
        target_buffer.push_str(r_fragment);
        fragment_end = target_buffer.len();
    } else {
        // T.scheme = Base.scheme;
        target_buffer.push_str(base_scheme);
        scheme_end = target_buffer.len();

        // if defined(R.authority) then
        if !r_authority.is_empty() {
            // T.authority = R.authority;
            target_buffer.push_str(r_authority);
            authority_end = target_buffer.len();

            // T.path = remove_dot_segments(R.path);
            append_and_remove_dot_segments(r_path, target_buffer, target_buffer.len());
            path_end = target_buffer.len();

            // T.query = R.query;
            target_buffer.push_str(r_query);
            query_end = target_buffer.len();

            // T.fragment = R.fragment;
            target_buffer.push_str(r_fragment);
            fragment_end = target_buffer.len();
        } else {
            // T.authority = Base.authority;
            target_buffer.push_str(base_authority);
            authority_end = target_buffer.len();

            // if (R.path == "") then
            if r_path == "" {
                // T.path = Base.path;
                target_buffer.push_str(base_path);
                path_end = target_buffer.len();

                // if defined(R.query) then
                if !r_query.is_empty() {
                    // T.query = R.query;
                    target_buffer.push_str(r_query);
                } else {
                    // T.query = Base.query;
                    target_buffer.push_str(base_query);
                }
                query_end = target_buffer.len();
            } else {
                // if (R.path starts-with "/") then
                if r_path.starts_with('/') {
                    // T.path = remove_dot_segments(R.path);
                    append_and_remove_dot_segments(r_path, target_buffer, target_buffer.len());
                } else {
                    let path_start_in_target = target_buffer.len();
                    // T.path = merge(Base.path, R.path);
                    // T.path = remove_dot_segments(T.path);
                    if base_positions.authority_end > base_positions.scheme_end
                        && base_positions.path_end == base_positions.authority_end
                    {
                        append_and_remove_dot_segments_with_extra_slash(
                            r_path,
                            target_buffer,
                            path_start_in_target,
                        );
                    } else {
                        let last_base_slash = base_path
                            .char_indices()
                            .rev()
                            .find(|(_, c)| *c == '/')
                            .map_or(0, |(i, _)| i)
                            + base_positions.authority_end;
                        append_and_remove_dot_segments(
                            &base_iri[base_positions.authority_end..=last_base_slash],
                            target_buffer,
                            path_start_in_target,
                        );
                        if target_buffer.ends_with('/') {
                            target_buffer.pop();
                            append_and_remove_dot_segments_with_extra_slash(
                                r_path,
                                target_buffer,
                                path_start_in_target,
                            );
                        } else {
                            append_and_remove_dot_segments(
                                r_path,
                                target_buffer,
                                path_start_in_target,
                            );
                        }
                    }
                }
                path_end = target_buffer.len();

                // T.query = R.query;
                target_buffer.push_str(r_query);
                query_end = target_buffer.len();
            }
            // T.fragment = R.fragment;
            target_buffer.push_str(r_fragment);
            fragment_end = target_buffer.len();
        }
    }
    Ok(IriElementsPositions {
        scheme_end,
        authority_end,
        path_end,
        query_end,
        fragment_end,
    })
}

// RFC 3986 5.2.4 Remove Dot Segments
fn append_and_remove_dot_segments(
    mut input: &str,
    output: &mut String,
    path_start_in_output: usize,
) {
    while !input.is_empty() {
        if input.starts_with("../") {
            input = &input[3..];
        } else if input.starts_with("./") || input.starts_with("/./") {
            input = &input[2..];
        } else if input == "/." {
            input = "/";
        } else if input.starts_with("/../") {
            pop_last_segment(output, path_start_in_output);
            input = &input[3..];
        } else if input == "/.." {
            pop_last_segment(output, path_start_in_output);
            input = "/";
        } else if input == "." || input == ".." {
            input = "";
        } else {
            if input.starts_with('/') {
                output.push('/');
                input = &input[1..];
            }
            if let Some(i) = input.find('/') {
                output.push_str(&input[..i]);
                input = &input[i..];
            } else {
                output.push_str(input);
                input = "";
            }
        }
    }
}

fn pop_last_segment(buffer: &mut String, path_start_in_buffer: usize) {
    if let Some((last_slash_position, _)) = buffer[path_start_in_buffer..]
        .char_indices()
        .rev()
        .find(|(_, c)| *c == '/')
    {
        buffer.truncate(last_slash_position + path_start_in_buffer)
    }
}

fn append_and_remove_dot_segments_with_extra_slash(
    input: &str,
    output: &mut String,
    path_start_in_output: usize,
) {
    if input.is_empty() {
        output.push('/');
    } else if input.starts_with("./") {
        append_and_remove_dot_segments(&input[1..], output, path_start_in_output)
    } else if input == "." {
        append_and_remove_dot_segments("/", output, path_start_in_output)
    } else if input.starts_with("../") {
        pop_last_segment(output, path_start_in_output);
        append_and_remove_dot_segments(&input[2..], output, path_start_in_output)
    } else if input == ".." {
        pop_last_segment(output, path_start_in_output);
        append_and_remove_dot_segments("/", output, path_start_in_output)
    } else {
        output.push('/');
        if let Some(i) = input.find('/') {
            output.push_str(&input[..i]);
            append_and_remove_dot_segments(&input[i..], output, path_start_in_output)
        } else {
            output.push_str(input);
        }
    }
}

fn parse_iri(value: &[u8], start: usize) -> Result<IriElementsPositions, usize> {
    // IRI = scheme ":" ihier-part [ "?" iquery ] [ "#" ifragment ]
    let scheme_end = parse_scheme(value, start)?;
    if scheme_end >= value.len() || value[scheme_end] != b':' {
        return Err(scheme_end);
    }

    let (authority_end, path_end) = parse_ihier_part(value, scheme_end + 1)?;

    let query_end = if path_end < value.len() && value[path_end] == b'?' {
        parse_iquery(value, path_end + 1)?
    } else {
        path_end
    };

    let fragment_end = if query_end < value.len() && value[query_end] == b'#' {
        parse_ifragment(value, query_end + 1)?
    } else {
        query_end
    };

    Ok(IriElementsPositions {
        scheme_end: scheme_end + 1,
        authority_end,
        path_end,
        query_end,
        fragment_end,
    })
}

fn parse_ihier_part(value: &[u8], start: usize) -> Result<(usize, usize), usize> {
    // (authority_end, path_end)
    // ihier-part = "//" iauthority ipath-abempty / ipath-absolute / ipath-rootless / ipath-empty
    if value[start..].starts_with(b"//") {
        let authority_end = parse_iauthority(value, start + 2)?;
        Ok((authority_end, parse_ipath_abempty(value, authority_end)?))
    } else if value[start..].starts_with(b"/") {
        Ok((start, parse_ipath_absolute(value, start)?))
    } else {
        match parse_ipath_rootless(value, start) {
            Ok(i) => Ok((start, i)),
            Err(i) => {
                if i == start {
                    Ok((start, i)) // ipath empty
                } else {
                    Err(i)
                }
            }
        }
    }
}

fn parse_iri_reference(value: &[u8], start: usize) -> Result<IriElementsPositions, usize> {
    // IRI-reference  = IRI / irelative-ref
    match parse_iri(value, start) {
        Ok(positions) => Ok(positions),
        Err(_) => parse_irelative_ref(value, start),
    }
}

fn parse_irelative_ref(value: &[u8], start: usize) -> Result<IriElementsPositions, usize> {
    // irelative-ref = irelative-part [ "?" iquery ] [ "#" ifragment ]
    let (authority_end, path_end) = parse_irelative_path(value, start)?;

    let query_end = if path_end < value.len() && value[path_end] == b'?' {
        parse_iquery(value, path_end + 1)?
    } else {
        path_end
    };
    let fragment_end = if query_end < value.len() && value[query_end] == b'#' {
        parse_ifragment(&value, query_end + 1)?
    } else {
        query_end
    };

    Ok(IriElementsPositions {
        scheme_end: start,
        authority_end,
        path_end,
        query_end,
        fragment_end,
    })
}

fn parse_irelative_path(value: &[u8], start: usize) -> Result<(usize, usize), usize> {
    // (authority_end, path_end)
    // irelative-part = "//" iauthority ipath-abempty / ipath-absolute / ipath-noscheme / ipath-empty
    if value[start..].starts_with(b"//") {
        let authority_end = parse_iauthority(&value, start + 2)?;
        Ok((authority_end, parse_ipath_abempty(value, authority_end)?))
    } else if value[start..].starts_with(b"/") {
        Ok((start, parse_ipath_absolute(value, start)?))
    } else {
        match parse_ipath_noscheme(value, start) {
            Ok(i) => Ok((start, i)),
            Err(i) => {
                if i == start {
                    Ok((start, i)) // ipath empty
                } else {
                    Err(i)
                }
            }
        }
    }
}

fn parse_scheme(value: &[u8], start: usize) -> IriState {
    //  scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
    if value.len() <= start || !is_alpha(value[start]) {
        return Err(start);
    }
    for (i, c) in value[start..].iter().enumerate() {
        match *c {
            c if is_alpha(c) || is_digit(c) || c == b'+' || c == b'-' || c == b'.' => (),
            _ => return Ok(start + i),
        }
    }
    Err(value.len())
}

fn parse_iauthority(value: &[u8], start: usize) -> IriState {
    // iauthority = [ iuserinfo "@" ] ihost [ ":" port ]
    //TODO: implement properly
    for (i, c) in value[start..].iter().enumerate() {
        match *c {
            b'/' | b'?' | b'#' => return Ok(start + i),
            _ => (),
        }
    }
    Ok(value.len())
}

fn parse_ipath_abempty(value: &[u8], start: usize) -> IriState {
    // ipath-abempty  = *( "/" isegment )
    let mut i = start;
    while i < value.len() {
        match value[i] {
            b'/' => {
                i = parse_isegment(value, i + 1)?;
            }
            _ => return Ok(i),
        }
    }
    Ok(value.len())
}

fn parse_ipath_absolute(value: &[u8], start: usize) -> IriState {
    // ipath-absolute = "/" [ isegment-nz *( "/" isegment ) ] = "/" [ isegment-nz ipath-abempty ]
    if !value[start..].starts_with(b"/") {
        return Err(start);
    }

    match parse_isegment_nz(value, start + 1) {
        Ok(i) => parse_ipath_abempty(value, i),
        Err(i) => {
            if i == start + 1 {
                Ok(i) // optional
            } else {
                Err(i)
            }
        }
    }
}

fn parse_ipath_noscheme(value: &[u8], start: usize) -> IriState {
    // ipath-noscheme = isegment-nz-nc *( "/" isegment ) =  isegment-nz-nc ipath-abempty
    let i = parse_isegment_nz_nc(value, start)?;
    parse_ipath_abempty(&value, i)
}

fn parse_ipath_rootless(value: &[u8], start: usize) -> IriState {
    // ipath-rootless = isegment-nz *( "/" isegment ) = isegment-nz ipath-abempty
    let i = parse_isegment_nz(value, start)?;
    parse_ipath_abempty(value, i)
}

fn parse_isegment(value: &[u8], start: usize) -> IriState {
    // isegment = *ipchar
    //TODO: implement properly
    for (i, c) in value[start..].iter().enumerate() {
        match *c {
            b'/' | b'?' | b'#' => return Ok(start + i),
            _ => (),
        }
    }
    Ok(value.len())
}

fn parse_isegment_nz(value: &[u8], start: usize) -> IriState {
    // isegment-nz    = 1*ipchar
    let i = parse_isegment(value, start)?;
    if i == start {
        Err(0)
    } else {
        Ok(i)
    }
}

fn parse_isegment_nz_nc(value: &[u8], start: usize) -> IriState {
    // isegment-nz-nc = 1*( iunreserved / pct-encoded / sub-delims / "@" )
    //TODO: implement properly
    for (i, c) in value[start..].iter().enumerate() {
        match *c {
            b'/' | b'?' | b'#' | b':' => return if i == start { Err(i) } else { Ok(i) },
            _ => (),
        }
    }
    Ok(value.len())
}

fn parse_iquery(value: &[u8], start: usize) -> IriState {
    // iquery = *( ipchar / iprivate / "/" / "?" )
    //TODO: implement properly
    for (i, c) in value[start..].iter().enumerate() {
        if *c == b'#' {
            return Ok(start + i);
        }
    }
    Ok(value.len())
}

fn parse_ifragment(value: &[u8], _start: usize) -> IriState {
    // ifragment = *( ipchar / "/" / "?" )
    //TODO: implement properly
    Ok(value.len())
}

fn is_alpha(b: u8) -> bool {
    match b {
        b'a'..=b'z' | b'A'..=b'Z' => true,
        _ => false,
    }
}

fn is_digit(b: u8) -> bool {
    match b {
        b'0'..=b'9' => true,
        _ => false,
    }
}

#[test]
fn test_parsing() {
    let examples = [
        "file://foo",
        "ftp://ftp.is.co.za/rfc/rfc1808.txt",
        "http://www.ietf.org/rfc/rfc2396.txt",
        "ldap://[2001:db8::7]/c=GB?objectClass?one",
        "mailto:John.Doe@example.com",
        "news:comp.infosystems.www.servers.unix",
        "tel:+1-816-555-1212",
        "telnet://192.0.2.16:80/",
        "urn:oasis:names:specification:docbook:dtd:xml:4.1.2",
        "http://example.com",
        "http://example.com/",
        "http://example.com/foo",
        "http://example.com/foo/bar",
        "http://example.com/foo/bar/",
        "http://example.com/foo/bar?q=1&r=2",
        "http://example.com/foo/bar/?q=1&r=2",
        "http://example.com#toto",
        "http://example.com/#toto",
        "http://example.com/foo#toto",
        "http://example.com/foo/bar#toto",
        "http://example.com/foo/bar/#toto",
        "http://example.com/foo/bar?q=1&r=2#toto",
        "http://example.com/foo/bar/?q=1&r=2#toto",
    ];

    for e in &examples {
        assert!(
            Iri::parse(e.to_string()).is_ok(),
            "{} is not recognized as an IRI",
            e
        );
    }
}

#[test]
fn test_resolve_relative_iri() {
    let base = "http://a/b/c/d;p?q";

    let examples = [
        ("g:h", "g:h"),
        ("g", "http://a/b/c/g"),
        ("g/", "http://a/b/c/g/"),
        ("/g", "http://a/g"),
        ("//g", "http://g"),
        ("?y", "http://a/b/c/d;p?y"),
        ("g?y", "http://a/b/c/g?y"),
        ("#s", "http://a/b/c/d;p?q#s"),
        ("g#s", "http://a/b/c/g#s"),
        ("g?y#s", "http://a/b/c/g?y#s"),
        (";x", "http://a/b/c/;x"),
        ("g;x", "http://a/b/c/g;x"),
        ("g;x?y#s", "http://a/b/c/g;x?y#s"),
        ("", "http://a/b/c/d;p?q"),
        (".", "http://a/b/c/"),
        ("./", "http://a/b/c/"),
        ("./g", "http://a/b/c/g"),
        ("..", "http://a/b/"),
        ("../", "http://a/b/"),
        ("../g", "http://a/b/g"),
        ("../..", "http://a/"),
        ("../../", "http://a/"),
        ("../../g", "http://a/g"),
        ("../../../g", "http://a/g"),
        ("../../../../g", "http://a/g"),
        ("/./g", "http://a/g"),
        ("/../g", "http://a/g"),
        ("g.", "http://a/b/c/g."),
        (".g", "http://a/b/c/.g"),
        ("g..", "http://a/b/c/g.."),
        ("..g", "http://a/b/c/..g"),
        ("./../g", "http://a/b/g"),
        ("./g/.", "http://a/b/c/g/"),
        ("g/./h", "http://a/b/c/g/h"),
        ("g/../h", "http://a/b/c/h"),
        ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
        ("g;x=1/../y", "http://a/b/c/y"),
        ("g?y/./x", "http://a/b/c/g?y/./x"),
        ("g?y/../x", "http://a/b/c/g?y/../x"),
        ("g#s/./x", "http://a/b/c/g#s/./x"),
        ("g#s/../x", "http://a/b/c/g#s/../x"),
        ("http:g", "http:g"),
        ("./g:h", "http://a/b/c/g:h"),
    ];

    let base = Iri::parse(base.to_owned()).unwrap();
    for (input, output) in examples.iter() {
        let result = base.resolve(input);
        assert!(
            result.is_ok(),
            "Resolving of {} failed with error: {}",
            input,
            result.unwrap_err()
        );
        let result = result.unwrap().into_string();
        assert_eq!(
            result, *output,
            "Resolving of {} is wrong. Found {} and expecting {}",
            input, result, output
        );
    }
}
