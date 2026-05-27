//! Runtime regression guard for the OPTIONAL-on-FK quadratic-scaling bug
//! fixed in 5c7feb9a / 54826e5c.
//!
//! Builds a small store with N persons and M orders per person, then runs the
//! exact query shape that triggered the bug with execution statistics enabled.
//! Inspects the explanation tree and asserts:
//!   1. The OPTIONAL was rewritten to a `ForLoopLeftJoin` (Lateral form).
//!   2. The max `exec_count` across QuadPattern nodes under that
//!      ForLoopLeftJoin stays well below the quadratic floor.
//!
//! With the fix, the inner BGP is reordered so `?order schema:customer ?c`
//! runs first as an indexed FK lookup, yielding ~N * ORDERS_PER_PERSON inner
//! work per node. Without the BGP reorder, the inner BGP would start with
//! `?order a schema:Order` and each outer iteration scans every order,
//! yielding ~N * total_orders inner work — the quadratic regression we want
//! to catch.

#![cfg(test)]

use oxigraph::model::vocab::rdf;
use oxigraph::model::{Literal, NamedNode, NamedNodeRef, QuadRef};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde_json::Value;

const N_PERSONS: usize = 20;
const ORDERS_PER_PERSON: usize = 20;

const QUERY: &str = "
PREFIX schema: <http://schema.org/>
PREFIX ex: <http://example.org/>
SELECT * WHERE {
  ?c a schema:Person ;
     schema:addressCountry ?country ;
     ex:segment ?segment .
  OPTIONAL {
    ?order a schema:Order ;
           schema:customer ?c ;
           schema:totalPrice ?total .
  }
}
";

#[test]
fn optional_on_foreign_key_is_not_quadratic() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    let person_type = NamedNodeRef::new("http://schema.org/Person")?;
    let order_type = NamedNodeRef::new("http://schema.org/Order")?;
    let address_country = NamedNodeRef::new("http://schema.org/addressCountry")?;
    let segment = NamedNodeRef::new("http://example.org/segment")?;
    let customer = NamedNodeRef::new("http://schema.org/customer")?;
    let total_price = NamedNodeRef::new("http://schema.org/totalPrice")?;
    let country = Literal::new_simple_literal("FR");
    let seg = Literal::new_simple_literal("retail");

    let persons: Vec<NamedNode> = (0..N_PERSONS)
        .map(|i| NamedNode::new(format!("http://example.org/person/{i}")).unwrap())
        .collect();

    for p in &persons {
        store.insert(QuadRef::new(p, rdf::TYPE, person_type, oxigraph::model::GraphNameRef::DefaultGraph))?;
        store.insert(QuadRef::new(p, address_country, country.as_ref(), oxigraph::model::GraphNameRef::DefaultGraph))?;
        store.insert(QuadRef::new(p, segment, seg.as_ref(), oxigraph::model::GraphNameRef::DefaultGraph))?;
    }
    let mut order_idx = 0;
    for p in &persons {
        for _ in 0..ORDERS_PER_PERSON {
            let order = NamedNode::new(format!("http://example.org/order/{order_idx}"))?;
            order_idx += 1;
            let price = Literal::new_simple_literal(format!("{}", 10 + order_idx));
            store.insert(QuadRef::new(&order, rdf::TYPE, order_type, oxigraph::model::GraphNameRef::DefaultGraph))?;
            store.insert(QuadRef::new(&order, customer, p.as_ref(), oxigraph::model::GraphNameRef::DefaultGraph))?;
            store.insert(QuadRef::new(&order, total_price, price.as_ref(), oxigraph::model::GraphNameRef::DefaultGraph))?;
        }
    }

    let total_orders = N_PERSONS * ORDERS_PER_PERSON;
    // Inner BGP, when iterated per outer person via ForLoopLeftJoin, hits
    // ~total_orders work per person if it scans `?order a Order` first
    // (regression), and ~ORDERS_PER_PERSON work per person if it scans
    // `?order schema:customer ?c` first (optimized FK lookup).
    let quadratic_floor = N_PERSONS * total_orders; // regression plan order of magnitude

    let (results, explanation) = SparqlEvaluator::new()
        .parse_query(QUERY)?
        .on_store(&store)
        .compute_statistics()
        .explain();
    let QueryResults::Solutions(solutions) = results? else {
        panic!("expected solutions");
    };
    let row_count = solutions.count();
    assert_eq!(row_count, total_orders, "result row count mismatch");

    let mut buf = Vec::new();
    explanation.write_in_json(&mut buf)?;
    let json: Value = serde_json::from_slice(&buf)?;

    let plan = &json["plan"];
    let for_loop = find_label(plan, "ForLoopLeftJoin").unwrap_or_else(|| {
        panic!("expected the OPTIONAL to be rewritten as ForLoopLeftJoin (Lateral); plan was:\n{plan:#}")
    });

    // The inner BGP under the ForLoopLeftJoin is iterated per outer person.
    // With the FK-first reorder, each outer iteration's first inner pattern
    // (`?order schema:customer ?c`) is an indexed lookup yielding
    // ~ORDERS_PER_PERSON, so the max exec_count across the inner triple
    // patterns stays at ~N_PERSONS * ORDERS_PER_PERSON.
    // Regression: if the inner BGP starts with `?order a schema:Order` (or
    // any pattern not constrained by the outer bindings), each outer iteration
    // scans the full order set and exec_count blows up to
    // ~N_PERSONS * total_orders.
    let max_quad_pattern_work = max_quad_pattern_exec_count(for_loop);
    assert!(
        (max_quad_pattern_work as usize) < quadratic_floor / 4,
        "max inner QuadPattern exec_count = {max_quad_pattern_work} is in the quadratic regime (floor = {quadratic_floor}); plan:\n{plan:#}"
    );

    Ok(())
}

fn max_quad_pattern_exec_count(node: &Value) -> u64 {
    let label = node.get("name").and_then(Value::as_str).unwrap_or("");
    let here = if label.starts_with("QuadPattern(") {
        node.get("number of results")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    } else {
        0
    };
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .map(|c| c.iter().map(max_quad_pattern_exec_count).max().unwrap_or(0))
        .unwrap_or(0);
    here.max(children)
}

fn find_label<'a>(node: &'a Value, needle: &str) -> Option<&'a Value> {
    if node
        .get("name")
        .and_then(Value::as_str)
        .map(|n| n.contains(needle))
        .unwrap_or(false)
    {
        return Some(node);
    }
    node.get("children")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|c| find_label(c, needle))
}
