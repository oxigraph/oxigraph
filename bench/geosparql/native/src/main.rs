//! Rust-only bench harness for GeoSPARQL point-in-polygon workloads.
//!
//! Usage: `spargeo_bench <engine> <path-to-turtle>`
//!
//! `<engine>` is one of:
//!
//! * `spargeo`: invokes `geof:sfWithin` through the public
//!   `GEOSPARQL_EXTENSION_FUNCTIONS` table the SPARQL evaluator wires in.
//!   Each call reparses the WKT literal, matching how an engine actually
//!   calls the function.
//! * `geo`: parses every WKT literal once at load time and runs
//!   `geo::Relate::relate(...).is_within()` directly on the parsed
//!   geometries. This is the lower bound that spargeo could reach if
//!   literal parsing were amortised (e.g. via the WKB storage proposed
//!   in oxigraph issue #1560).
//! * `index`: parses every WKT literal once, drops the points into
//!   `spargeo::index::SpatialIndex`, and calls `query_within` for each
//!   polygon. Exercises the ancestor walk plus Hilbert range scan path
//!   that gathers candidates before `geo::Relate` runs, so query_ms
//!   should stay near-constant in `points` for a fixed polygon set.
//! * `wktstore`: loads the fixture into an oxigraph store built with
//!   the `geosparql` feature so `wktLiteral` values are parsed once
//!   into WKB at insert time. At query time the bench pulls the
//!   point geometries back via `Store::object_geometries_for_pattern`,
//!   which skips the WKT lexer for inline WKB literals. This isolates
//!   the storage-side cost amortisation that oxigraph issue #1560
//!   proposes: identical geometry loop to `geo`, but the parse tax is
//!   paid in `parse_ms` once rather than on every `query_ms` call.
//!
//! Workload: for each polygon in the fixture, test every point in the
//! fixture. Total ops = num_polygons * num_points. The engine is timed
//! across the full loop; parsing the Turtle file into the (subject, wkt)
//! pairs happens before the timer starts and is reported separately as
//! `parse_ms`.
//!
//! Output is a single JSON line on stdout, matching the shape of the
//! reasoner bench binary so the Python driver stays the same:
//!
//! ```json
//! {"engine":"spargeo","parse_ms":12.3,"query_ms":45.6,"points":10000,"polygons":10,"matches":1234}
//! ```
//!
//! Note on fairness: both engines receive the same extracted WKT strings.
//! spargeo pays the per-call parsing cost because that is the shape of
//! the public API it exposes to the SPARQL evaluator. The `geo` baseline
//! measures the same algorithm without that tax.

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use geo::{Geometry, Relate};
use oxigraph::io::RdfFormat;
use oxigraph::model::NamedNodeRef as OxigraphNamedNodeRef;
use oxigraph::store::Store;
use oxrdf::{Literal, NamedNodeRef, Term};
use oxttl::TurtleParser;
use spargeo::GEOSPARQL_EXTENSION_FUNCTIONS;
use spargeo::index::SpatialIndex;
use wkt::TryFromWkt;

const SF_WITHIN_IRI: &str = "http://www.opengis.net/def/function/geosparql/sfWithin";
const AS_WKT_IRI: &str = "http://www.opengis.net/ont/geosparql#asWKT";
const WKT_LITERAL_IRI: &str = "http://www.opengis.net/ont/geosparql#wktLiteral";
const WKT_LITERAL: NamedNodeRef<'static> = NamedNodeRef::new_unchecked(WKT_LITERAL_IRI);

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: spargeo_bench <spargeo|geo|index|wktstore> <path-to-turtle>");
        return ExitCode::from(2);
    }
    let engine = args[1].as_str();
    let path = &args[2];

    let result = match engine {
        "spargeo" => run_spargeo(path),
        "geo" => run_geo(path),
        "index" => run_index(path),
        "wktstore" => run_wktstore(path),
        other => {
            eprintln!("unknown engine '{other}'; expected spargeo, geo, index or wktstore");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(r) => {
            println!(
                "{{\"engine\":\"{engine}\",\"parse_ms\":{parse:.3},\"query_ms\":{query:.3},\"points\":{points},\"polygons\":{polygons},\"matches\":{matches}}}",
                engine = r.engine,
                parse = r.parse_ms,
                query = r.query_ms,
                points = r.points,
                polygons = r.polygons,
                matches = r.matches,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("spargeo_bench failed: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Run {
    engine: &'static str,
    parse_ms: f64,
    query_ms: f64,
    points: usize,
    polygons: usize,
    matches: usize,
}

/// Extract every WKT string present as a `geo:asWKT` object in the Turtle file.
///
/// Classification into points and polygons is done on the caller side by
/// looking at the WKT prefix. The two engines below consume this pair the
/// same way.
fn extract_wkts(path: &str) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error>> {
    let file = File::open(Path::new(path))?;
    let reader = BufReader::new(file);
    let mut parser = TurtleParser::new().for_reader(reader);

    let mut points: Vec<String> = Vec::new();
    let mut polygons: Vec<String> = Vec::new();

    while let Some(triple) = parser.next() {
        let triple = triple?;
        if triple.predicate.as_str() != AS_WKT_IRI {
            continue;
        }
        let oxrdf::Term::Literal(lit) = triple.object else {
            continue;
        };
        if lit.datatype().as_str() != WKT_LITERAL_IRI {
            continue;
        }
        let value = lit.value().trim();
        if value.starts_with("POINT") {
            points.push(value.to_owned());
        } else if value.starts_with("POLYGON") {
            polygons.push(value.to_owned());
        }
    }

    Ok((points, polygons))
}

fn run_spargeo(path: &str) -> Result<Run, Box<dyn std::error::Error>> {
    let parse_start = Instant::now();
    let (points, polygons) = extract_wkts(path)?;
    let parse_ms = ms(parse_start.elapsed());

    let sf_within = GEOSPARQL_EXTENSION_FUNCTIONS
        .iter()
        .find(|(iri, _)| iri.as_str() == SF_WITHIN_IRI)
        .map(|(_, f)| *f)
        .ok_or("spargeo does not expose geof:sfWithin")?;

    // Prebuild Term::Literal wrappers once. The inner Arc<str> clone on each
    // invocation is cheap relative to the per call WKT parse cost.
    let point_terms: Vec<Term> = points
        .iter()
        .map(|p| Term::Literal(Literal::new_typed_literal(p.as_str(), WKT_LITERAL)))
        .collect();
    let polygon_terms: Vec<Term> = polygons
        .iter()
        .map(|p| Term::Literal(Literal::new_typed_literal(p.as_str(), WKT_LITERAL)))
        .collect();

    let query_start = Instant::now();
    let mut matches = 0usize;
    for polygon in &polygon_terms {
        for point in &point_terms {
            let args = [point.clone(), polygon.clone()];
            if let Some(Term::Literal(lit)) = sf_within(&args) {
                if lit.value() == "true" {
                    matches += 1;
                }
            }
        }
    }
    let query_ms = ms(query_start.elapsed());

    Ok(Run {
        engine: "spargeo",
        parse_ms,
        query_ms,
        points: points.len(),
        polygons: polygons.len(),
        matches,
    })
}

fn run_geo(path: &str) -> Result<Run, Box<dyn std::error::Error>> {
    let parse_start = Instant::now();
    let (points, polygons) = extract_wkts(path)?;
    let point_geoms: Vec<Geometry> = points
        .iter()
        .map(|s| Geometry::try_from_wkt_str(s).map_err(|e| format!("parse point: {e}")))
        .collect::<Result<_, _>>()?;
    let polygon_geoms: Vec<Geometry> = polygons
        .iter()
        .map(|s| Geometry::try_from_wkt_str(s).map_err(|e| format!("parse polygon: {e}")))
        .collect::<Result<_, _>>()?;
    let parse_ms = ms(parse_start.elapsed());

    let query_start = Instant::now();
    let mut matches = 0usize;
    for polygon in &polygon_geoms {
        for point in &point_geoms {
            if point.relate(polygon).is_within() {
                matches += 1;
            }
        }
    }
    let query_ms = ms(query_start.elapsed());

    Ok(Run {
        engine: "geo",
        parse_ms,
        query_ms,
        points: point_geoms.len(),
        polygons: polygon_geoms.len(),
        matches,
    })
}

fn run_index(path: &str) -> Result<Run, Box<dyn std::error::Error>> {
    let parse_start = Instant::now();
    let (points, polygons) = extract_wkts(path)?;

    let point_geoms: Vec<Geometry> = points
        .iter()
        .map(|s| Geometry::try_from_wkt_str(s).map_err(|e| format!("parse point: {e}")))
        .collect::<Result<_, _>>()?;
    let polygon_geoms: Vec<Geometry> = polygons
        .iter()
        .map(|s| Geometry::try_from_wkt_str(s).map_err(|e| format!("parse polygon: {e}")))
        .collect::<Result<_, _>>()?;

    // Index the points (the candidate set the query polygons are tested
    // against). Feature keys are synthetic but stable so the result set
    // remains deterministic across runs.
    let mut index = SpatialIndex::new();
    for (i, geom) in point_geoms.iter().enumerate() {
        index.insert(format!("p{i}"), geom.clone());
    }
    let parse_ms = ms(parse_start.elapsed());

    let query_start = Instant::now();
    let mut matches = 0usize;
    for polygon in &polygon_geoms {
        matches += index.query_within(polygon).len();
    }
    let query_ms = ms(query_start.elapsed());

    Ok(Run {
        engine: "index",
        parse_ms,
        query_ms,
        points: point_geoms.len(),
        polygons: polygon_geoms.len(),
        matches,
    })
}

/// Bench engine that uses the oxigraph store as the geometry source.
///
/// The fixture is loaded through [`Store::load_from_reader`] so
/// `wktLiteral` objects go through the WKB encode path on insert.
/// At query time point geometries are pulled back via
/// [`Store::object_geometries_for_pattern`], which returns parsed
/// `geo::Geometry<f64>` without re-running the WKT lexer for inline
/// literals. Polygons still come from the raw turtle file because the
/// `BigWktLiteral` side-CF path is not wired yet, so large geometries
/// take the generic typed-literal route which does not expose the
/// geometry directly.
fn run_wktstore(path: &str) -> Result<Run, Box<dyn std::error::Error>> {
    let parse_start = Instant::now();

    // Load the fixture into an in-memory store. The geosparql feature
    // drives the WKB-encoded path for every inline-sized wktLiteral.
    let store = Store::new()?;
    let file = File::open(Path::new(path))?;
    store.load_from_reader(RdfFormat::Turtle, BufReader::new(file))?;

    // Pull the point geometries out of the store via the fast
    // accessor. This is the path the bench is meant to measure.
    let as_wkt = OxigraphNamedNodeRef::new_unchecked(AS_WKT_IRI);
    let point_geoms: Vec<Geometry> = store
        .object_geometries_for_pattern(None, Some(as_wkt), None)
        .collect::<Result<Vec<_>, _>>()?;

    // Polygons are not covered by the inline path (their WKB
    // overflows SmallBytes::<32>::CAPACITY) and the BigWktLiteral
    // variant that now holds them only keeps the WKT lexical in
    // id2str, not the raw WKB. Until the side-CF work for polygon
    // WKB lands we still gather polygons from the source file and
    // parse once. The matching amortisation is tracked in task #71.
    let (_raw_points, polygons) = extract_wkts(path)?;
    let polygon_geoms: Vec<Geometry> = polygons
        .iter()
        .map(|s| Geometry::try_from_wkt_str(s).map_err(|e| format!("parse polygon: {e}")))
        .collect::<Result<_, _>>()?;
    let parse_ms = ms(parse_start.elapsed());

    let query_start = Instant::now();
    let mut matches = 0usize;
    for polygon in &polygon_geoms {
        for point in &point_geoms {
            if point.relate(polygon).is_within() {
                matches += 1;
            }
        }
    }
    let query_ms = ms(query_start.elapsed());

    Ok(Run {
        engine: "wktstore",
        parse_ms,
        query_ms,
        points: point_geoms.len(),
        polygons: polygon_geoms.len(),
        matches,
    })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
