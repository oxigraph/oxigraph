//! Synthetic Polish geodata generator for the reasoning bench.
//!
//! Produces a small OWL 2 RL T-Box plus an ABox of Buildings (points),
//! Parcels (polygons) and Roads (linestrings) with attribute edges.
//! Coordinates are sampled uniformly from a box that roughly covers
//! Poland (lon 14.1 to 24.2, lat 49.0 to 54.9). The generator is
//! deterministic on a u64 seed so repeated runs produce the same graph.
//!
//! Shape proportions:
//!   60% Buildings (POINT)
//!   30% Parcels   (POLYGON, small axis-aligned quad)
//!   10% Roads     (LINESTRING, three-vertex)
//!
//! Each ABox entity contributes roughly 4 to 6 triples: rdf:type,
//! hasGeometry, at least one attribute edge, and depending on the shape
//! a standsIn/ownedBy/connects link into a separate identifier space.
//! Those extra identifiers are also typed so the T-Box domain/range
//! rules have something to fire on.

use oxrdf::{GraphName, Literal, NamedNode, Quad, Term};

pub const EX_NS: &str = "http://example.com/";
pub const GEO_NS: &str = "http://www.opengis.net/ont/geosparql#";
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const RDFS_SUB_CLASS_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";
pub const RDFS_SUB_PROPERTY_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf";
pub const RDFS_DOMAIN: &str = "http://www.w3.org/2000/01/rdf-schema#domain";
pub const RDFS_RANGE: &str = "http://www.w3.org/2000/01/rdf-schema#range";
pub const WKT_LITERAL: &str = "http://www.opengis.net/ont/geosparql#wktLiteral";

/// Generator configuration.
pub struct Config {
    /// Number of ABox entities to generate. The shape mix is fixed.
    pub entities: usize,
    /// PRNG seed. Fixed seed gives reproducible graphs across runs.
    pub seed: u64,
}

/// Splitmix64 PRNG. Small, deterministic, no dep.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

fn nn(iri: String) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

fn ex(local: &str) -> NamedNode {
    nn(format!("{EX_NS}{local}"))
}

fn wkt_literal(value: String) -> Literal {
    Literal::new_typed_literal(value, NamedNode::new_unchecked(WKT_LITERAL))
}

/// Emit the static OWL 2 RL T-Box. The ABox is generated on top of
/// this schema, and reasoning over the combined graph fires cax-sco,
/// prp-dom, prp-rng, and prp-spo1 at minimum.
pub fn tbox_quads() -> Vec<Quad> {
    let sub_class = nn(RDFS_SUB_CLASS_OF.to_owned());
    let sub_prop = nn(RDFS_SUB_PROPERTY_OF.to_owned());
    let domain = nn(RDFS_DOMAIN.to_owned());
    let range = nn(RDFS_RANGE.to_owned());

    let building = ex("Building");
    let structure = ex("Structure");
    let feature = ex("Feature");
    let parcel = ex("Parcel");
    let land_feature = ex("LandFeature");
    let road = ex("Road");
    let linear_feature = ex("LinearFeature");
    let owner = ex("Owner");
    let address = ex("Address");
    let intersection = ex("Intersection");

    let has_geometry = ex("hasGeometry");
    let has_address = ex("hasAddress");
    let has_attribute = ex("hasAttribute");
    let owned_by = ex("ownedBy");
    let stands_in = ex("standsIn");
    let connects = ex("connects");

    let g = GraphName::DefaultGraph;
    let q = |s: NamedNode, p: NamedNode, o: NamedNode| {
        Quad::new(s, p, Term::NamedNode(o), g.clone())
    };

    vec![
        // subClassOf chain: Building ⊑ Structure ⊑ Feature
        q(building.clone(), sub_class.clone(), structure.clone()),
        q(structure.clone(), sub_class.clone(), feature.clone()),
        // Parcel ⊑ LandFeature ⊑ Feature
        q(parcel.clone(), sub_class.clone(), land_feature.clone()),
        q(land_feature.clone(), sub_class.clone(), feature.clone()),
        // Road ⊑ LinearFeature ⊑ Feature
        q(road.clone(), sub_class.clone(), linear_feature.clone()),
        q(linear_feature, sub_class.clone(), feature.clone()),
        // subPropertyOf: hasAddress, ownedBy ⊑ hasAttribute
        q(has_address.clone(), sub_prop.clone(), has_attribute.clone()),
        q(owned_by.clone(), sub_prop, has_attribute),
        // hasGeometry rdfs:domain Feature
        q(has_geometry, domain.clone(), feature),
        // standsIn rdfs:domain Building; rdfs:range Parcel
        q(stands_in.clone(), domain.clone(), building),
        q(stands_in, range.clone(), parcel),
        // connects rdfs:domain Road; rdfs:range Intersection
        q(connects.clone(), domain.clone(), road),
        q(connects, range.clone(), intersection),
        // hasAddress rdfs:range Address
        q(has_address, range.clone(), address),
        // ownedBy rdfs:range Owner
        q(owned_by, range, owner),
    ]
}

/// Yield every ABox quad for `config.entities` entities. Consumed as an
/// iterator so the full graph never materialises in memory.
pub fn abox_iter(config: Config) -> impl Iterator<Item = Quad> {
    let mut rng = Rng::new(config.seed);
    (0..config.entities).flat_map(move |i| generate_entity(&mut rng, i).into_iter())
}

/// Combined T-Box + ABox iterator. The T-Box is emitted first so the
/// schema is present by the time the bulk loader starts batching ABox
/// triples.
pub fn full_iter(config: Config) -> impl Iterator<Item = Quad> {
    tbox_quads().into_iter().chain(abox_iter(config))
}

fn generate_entity(rng: &mut Rng, i: usize) -> Vec<Quad> {
    let bucket = i % 10;
    if bucket < 6 {
        generate_building(rng, i)
    } else if bucket < 9 {
        generate_parcel(rng, i)
    } else {
        generate_road(rng, i)
    }
}

/// Uniform longitude in Poland's bounding box.
fn sample_lon(rng: &mut Rng) -> f64 {
    14.1 + rng.next_f64() * (24.2 - 14.1)
}

/// Uniform latitude in Poland's bounding box.
fn sample_lat(rng: &mut Rng) -> f64 {
    49.0 + rng.next_f64() * (54.9 - 49.0)
}

fn rdf_type() -> NamedNode {
    nn(RDF_TYPE.to_owned())
}

fn generate_building(rng: &mut Rng, i: usize) -> Vec<Quad> {
    let subj = ex(&format!("b_{i}"));
    let lon = sample_lon(rng);
    let lat = sample_lat(rng);
    let wkt = format!("POINT({lon:.6} {lat:.6})");
    let addr = ex(&format!("addr_{i}"));
    let parcel_idx = rng.next_u64() as usize;
    let parcel = ex(&format!("parcel_{parcel_idx}"));

    let g = GraphName::DefaultGraph;
    vec![
        Quad::new(
            subj.clone(),
            rdf_type(),
            Term::NamedNode(ex("Building")),
            g.clone(),
        ),
        Quad::new(
            subj.clone(),
            ex("hasGeometry"),
            Term::Literal(wkt_literal(wkt)),
            g.clone(),
        ),
        Quad::new(
            subj.clone(),
            ex("hasAddress"),
            Term::NamedNode(addr),
            g.clone(),
        ),
        Quad::new(subj, ex("standsIn"), Term::NamedNode(parcel), g),
    ]
}

fn generate_parcel(rng: &mut Rng, i: usize) -> Vec<Quad> {
    let subj = ex(&format!("parcel_{i}"));
    // Small axis-aligned quad about 50 m across at Polish latitudes.
    let lon0 = sample_lon(rng);
    let lat0 = sample_lat(rng);
    let dlon = 0.0005;
    let dlat = 0.00045;
    let lon1 = lon0 + dlon;
    let lat1 = lat0 + dlat;
    let wkt = format!(
        "POLYGON(({lon0:.6} {lat0:.6}, {lon1:.6} {lat0:.6}, {lon1:.6} {lat1:.6}, {lon0:.6} {lat1:.6}, {lon0:.6} {lat0:.6}))"
    );
    let owner_idx = rng.next_u64() as usize;
    let owner = ex(&format!("owner_{owner_idx}"));

    let g = GraphName::DefaultGraph;
    vec![
        Quad::new(
            subj.clone(),
            rdf_type(),
            Term::NamedNode(ex("Parcel")),
            g.clone(),
        ),
        Quad::new(
            subj.clone(),
            ex("hasGeometry"),
            Term::Literal(wkt_literal(wkt)),
            g.clone(),
        ),
        Quad::new(subj, ex("ownedBy"), Term::NamedNode(owner), g),
    ]
}

fn generate_road(rng: &mut Rng, i: usize) -> Vec<Quad> {
    let subj = ex(&format!("road_{i}"));
    // Three-vertex linestring, two short segments.
    let lon0 = sample_lon(rng);
    let lat0 = sample_lat(rng);
    let dlon = (rng.next_f64() - 0.5) * 0.01;
    let dlat = (rng.next_f64() - 0.5) * 0.01;
    let lon1 = lon0 + dlon;
    let lat1 = lat0 + dlat;
    let lon2 = lon1 + dlon * 0.5;
    let lat2 = lat1 + dlat * 0.5;
    let wkt = format!(
        "LINESTRING({lon0:.6} {lat0:.6}, {lon1:.6} {lat1:.6}, {lon2:.6} {lat2:.6})"
    );
    let a = ex(&format!("intersection_{}", rng.next_u64() as usize));
    let b = ex(&format!("intersection_{}", rng.next_u64() as usize));

    let g = GraphName::DefaultGraph;
    vec![
        Quad::new(
            subj.clone(),
            rdf_type(),
            Term::NamedNode(ex("Road")),
            g.clone(),
        ),
        Quad::new(
            subj.clone(),
            ex("hasGeometry"),
            Term::Literal(wkt_literal(wkt)),
            g.clone(),
        ),
        Quad::new(
            subj.clone(),
            ex("connects"),
            Term::NamedNode(a),
            g.clone(),
        ),
        Quad::new(subj, ex("connects"), Term::NamedNode(b), g),
    ]
}

/// Bounding box for post-reasoning spatial filter. Roughly Warsaw:
/// (20.75, 52.05) to (21.30, 52.45). The generator samples uniformly
/// over Poland so a consistent fraction of buildings fall inside.
pub const WARSAW_LON_MIN: f64 = 20.75;
pub const WARSAW_LON_MAX: f64 = 21.30;
pub const WARSAW_LAT_MIN: f64 = 52.05;
pub const WARSAW_LAT_MAX: f64 = 52.45;
