# Build a Knowledge Graph

This guide walks through building a complete knowledge graph from scratch using Oxigraph, from ontology design to query interface.

## Overview

A knowledge graph is a semantic network of entities, their attributes, and relationships. This guide covers:

1. **Ontology Design** - Define your domain model
2. **Data Ingestion** - Import from multiple sources
3. **Entity Linking** - Connect related entities
4. **Quality Assurance** - Validate and clean data
5. **Query Interface** - Provide access to the graph

## Example: Academic Knowledge Graph

We'll build a knowledge graph for academic publications, authors, and institutions.

## Step 1: Ontology Design

### Define Core Classes and Properties

```turtle
# schema.ttl - Our ontology definition

@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix acad: <http://academic.example.org/> .

# ============================================
# Classes
# ============================================

acad:Person a owl:Class ;
    rdfs:label "Person" ;
    rdfs:comment "A person, typically an author or researcher" .

acad:Publication a owl:Class ;
    rdfs:label "Publication" ;
    rdfs:comment "A research publication" .

acad:Journal a owl:Class ;
    rdfs:label "Journal" ;
    rdfs:comment "An academic journal" .

acad:Conference a owl:Class ;
    rdfs:label "Conference" ;
    rdfs:comment "An academic conference" .

acad:Institution a owl:Class ;
    rdfs:label "Institution" ;
    rdfs:comment "An academic institution" .

acad:Topic a owl:Class ;
    rdfs:label "Topic" ;
    rdfs:comment "A research topic or field" .

# ============================================
# Properties
# ============================================

# Person properties
acad:name a owl:DatatypeProperty ;
    rdfs:domain acad:Person ;
    rdfs:range xsd:string .

acad:email a owl:DatatypeProperty ;
    rdfs:domain acad:Person ;
    rdfs:range xsd:string .

acad:orcid a owl:DatatypeProperty ;
    rdfs:domain acad:Person ;
    rdfs:range xsd:string .

acad:affiliatedWith a owl:ObjectProperty ;
    rdfs:domain acad:Person ;
    rdfs:range acad:Institution .

# Publication properties
acad:title a owl:DatatypeProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range xsd:string .

acad:abstract a owl:DatatypeProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range xsd:string .

acad:publishedIn a owl:ObjectProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range [ owl:unionOf (acad:Journal acad:Conference) ] .

acad:publicationYear a owl:DatatypeProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range xsd:gYear .

acad:doi a owl:DatatypeProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range xsd:string .

acad:author a owl:ObjectProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range acad:Person .

acad:authorPosition a owl:DatatypeProperty ;
    rdfs:comment "Author's position in author list" ;
    rdfs:range xsd:integer .

acad:cites a owl:ObjectProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range acad:Publication .

acad:aboutTopic a owl:ObjectProperty ;
    rdfs:domain acad:Publication ;
    rdfs:range acad:Topic .
```

### Load Ontology

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use std::fs::File;
use std::io::BufReader;

fn load_ontology() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open("./academic-kg")?;

    // Load schema
    let schema_file = File::open("schema.ttl")?;
    let reader = BufReader::new(schema_file);

    store.load_from_reader(
        reader,
        RdfFormat::Turtle,
        None,  // Base IRI
        None   // Named graph
    )?;

    println!("Ontology loaded successfully!");

    Ok(())
}
```

## Step 2: Data Ingestion

### Source 1: CSV Publications

```python
# ingest_publications.py

import csv
from pyoxigraph import Store, NamedNode, Literal, Quad
from datetime import datetime

class AcademicNamespace:
    """Namespace helpers"""
    def __init__(self, base):
        self.base = base

    def __call__(self, local):
        return NamedNode(self.base + local)

# Define namespaces
ACAD = AcademicNamespace("http://academic.example.org/")
RDF = AcademicNamespace("http://www.w3.org/1999/02/22-rdf-syntax-ns#")
XSD = AcademicNamespace("http://www.w3.org/2001/XMLSchema#")

def ingest_publications_csv(store, csv_file):
    """
    Ingest publications from CSV.

    CSV format:
    doi,title,year,journal,authors
    """
    with open(csv_file, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)

        for row in reader:
            # Create publication URI using DOI
            pub_id = row['doi'].replace('/', '_')
            pub_uri = ACAD(f"publication/{pub_id}")

            # Add type
            store.add(Quad(pub_uri, RDF("type"), ACAD("Publication")))

            # Add title
            store.add(Quad(
                pub_uri,
                ACAD("title"),
                Literal(row['title'])
            ))

            # Add DOI
            store.add(Quad(
                pub_uri,
                ACAD("doi"),
                Literal(row['doi'])
            ))

            # Add year
            store.add(Quad(
                pub_uri,
                ACAD("publicationYear"),
                Literal(row['year'], datatype=XSD("gYear"))
            ))

            # Add journal
            if row['journal']:
                journal_id = row['journal'].replace(' ', '_')
                journal_uri = ACAD(f"journal/{journal_id}")

                store.add(Quad(journal_uri, RDF("type"), ACAD("Journal")))
                store.add(Quad(journal_uri, ACAD("name"), Literal(row['journal'])))
                store.add(Quad(pub_uri, ACAD("publishedIn"), journal_uri))

            # Add authors
            authors = [a.strip() for a in row['authors'].split(';')]
            for i, author_name in enumerate(authors):
                author_id = author_name.replace(' ', '_').lower()
                author_uri = ACAD(f"person/{author_id}")

                store.add(Quad(author_uri, RDF("type"), ACAD("Person")))
                store.add(Quad(author_uri, ACAD("name"), Literal(author_name)))
                store.add(Quad(pub_uri, ACAD("author"), author_uri))

                # Add authorship with position
                authorship_uri = ACAD(f"authorship/{pub_id}_{i}")
                store.add(Quad(authorship_uri, ACAD("publication"), pub_uri))
                store.add(Quad(authorship_uri, ACAD("author"), author_uri))
                store.add(Quad(
                    authorship_uri,
                    ACAD("authorPosition"),
                    Literal(str(i), datatype=XSD("integer"))
                ))

    print(f"Ingested publications from {csv_file}")

# Usage
store = Store("./academic-kg")
ingest_publications_csv(store, "publications.csv")
```

### Source 2: JSON API

```python
# ingest_from_api.py

import requests
import time
from pyoxigraph import Store, NamedNode, Literal, Quad

def ingest_from_crossref(store, query, max_results=100):
    """
    Ingest publications from CrossRef API.

    Args:
        store: Oxigraph store
        query: Search query
        max_results: Maximum number of results
    """
    base_url = "https://api.crossref.org/works"

    offset = 0
    rows = 50
    total_ingested = 0

    while total_ingested < max_results:
        # Query API
        params = {
            'query': query,
            'rows': rows,
            'offset': offset
        }

        response = requests.get(base_url, params=params)
        response.raise_for_status()

        data = response.json()
        items = data['message']['items']

        if not items:
            break

        # Ingest each publication
        for item in items:
            if total_ingested >= max_results:
                break

            try:
                ingest_crossref_item(store, item)
                total_ingested += 1

                if total_ingested % 10 == 0:
                    print(f"Ingested {total_ingested} publications...")

            except Exception as e:
                print(f"Error ingesting item: {e}")

        offset += rows
        time.sleep(1)  # Rate limiting

    print(f"Total ingested: {total_ingested} publications")

def ingest_crossref_item(store, item):
    """Ingest a single CrossRef item"""
    ACAD = lambda s: NamedNode(f"http://academic.example.org/{s}")
    RDF = lambda s: NamedNode(f"http://www.w3.org/1999/02/22-rdf-syntax-ns#{s}")

    # Get DOI
    doi = item.get('DOI')
    if not doi:
        return

    pub_id = doi.replace('/', '_')
    pub_uri = ACAD(f"publication/{pub_id}")

    # Type
    store.add(Quad(pub_uri, RDF("type"), ACAD("Publication")))

    # DOI
    store.add(Quad(pub_uri, ACAD("doi"), Literal(doi)))

    # Title
    if 'title' in item and item['title']:
        title = item['title'][0]
        store.add(Quad(pub_uri, ACAD("title"), Literal(title)))

    # Year
    if 'published-print' in item:
        year = item['published-print']['date-parts'][0][0]
        store.add(Quad(pub_uri, ACAD("publicationYear"), Literal(str(year))))

    # Authors
    if 'author' in item:
        for i, author in enumerate(item['author']):
            given = author.get('given', '')
            family = author.get('family', '')
            name = f"{given} {family}".strip()

            if name:
                author_id = name.replace(' ', '_').lower()
                author_uri = ACAD(f"person/{author_id}")

                store.add(Quad(author_uri, RDF("type"), ACAD("Person")))
                store.add(Quad(author_uri, ACAD("name"), Literal(name)))
                store.add(Quad(pub_uri, ACAD("author"), author_uri))

                # ORCID if available
                if 'ORCID' in author:
                    orcid = author['ORCID'].split('/')[-1]
                    store.add(Quad(author_uri, ACAD("orcid"), Literal(orcid)))

    # References (citations)
    if 'reference' in item:
        for ref in item['reference']:
            if 'DOI' in ref:
                ref_doi = ref['DOI'].replace('/', '_')
                ref_uri = ACAD(f"publication/{ref_doi}")
                store.add(Quad(pub_uri, ACAD("cites"), ref_uri))

# Usage
store = Store("./academic-kg")
ingest_from_crossref(store, "semantic web", max_results=100)
```

### Source 3: RDF from External Source

```python
def ingest_dbpedia_entities(store, entity_uris):
    """
    Import entities from DBpedia.

    Args:
        store: Oxigraph store
        entity_uris: List of DBpedia URIs
    """
    for entity_uri in entity_uris:
        query = f"""
        PREFIX dbo: <http://dbpedia.org/ontology/>
        PREFIX dbr: <http://dbpedia.org/resource/>
        PREFIX acad: <http://academic.example.org/>

        CONSTRUCT {{
            ?mapped a acad:Institution .
            ?mapped acad:name ?name .
            ?mapped acad:country ?country .
        }}
        WHERE {{
            SERVICE <https://dbpedia.org/sparql> {{
                <{entity_uri}> rdfs:label ?name .
                OPTIONAL {{ <{entity_uri}> dbo:country ?country }}
                FILTER(LANG(?name) = "en")
            }}
            BIND(IRI(CONCAT("http://academic.example.org/institution/",
                     ENCODE_FOR_URI(?name))) as ?mapped)
        }}
        """

        # Execute CONSTRUCT query and add results to store
        time.sleep(0.5)  # Rate limiting
        # In practice, execute the query and add triples
```

## Step 3: Entity Linking and Reconciliation

### Deduplicate Authors

```python
# link_entities.py

from pyoxigraph import Store, NamedNode, Quad, Literal
from difflib import SequenceMatcher

def similar(a, b):
    """Calculate string similarity"""
    return SequenceMatcher(None, a.lower(), b.lower()).ratio()

def find_duplicate_authors(store, threshold=0.9):
    """
    Find potential duplicate author entities.

    Args:
        store: Oxigraph store
        threshold: Similarity threshold (0-1)

    Returns:
        List of (entity1, entity2, similarity) tuples
    """
    # Get all authors
    query = """
    PREFIX acad: <http://academic.example.org/>

    SELECT ?author ?name
    WHERE {
        ?author a acad:Person .
        ?author acad:name ?name .
    }
    """

    authors = list(store.query(query))
    duplicates = []

    # Compare all pairs
    for i, author1 in enumerate(authors):
        for author2 in authors[i+1:]:
            name1 = author1['name'].value
            name2 = author2['name'].value

            similarity = similar(name1, name2)

            if similarity >= threshold:
                duplicates.append((
                    author1['author'].value,
                    author2['author'].value,
                    name1,
                    name2,
                    similarity
                ))

    return duplicates

def merge_entities(store, entity1_uri, entity2_uri, keep_entity1=True):
    """
    Merge two entities, keeping one and redirecting the other.

    Args:
        store: Oxigraph store
        entity1_uri: First entity URI
        entity2_uri: Second entity URI
        keep_entity1: If True, keep entity1, otherwise keep entity2
    """
    OWL = lambda s: NamedNode(f"http://www.w3.org/2002/07/owl#{s}")

    keep_uri = NamedNode(entity1_uri if keep_entity1 else entity2_uri)
    merge_uri = NamedNode(entity2_uri if keep_entity1 else entity1_uri)

    # Add owl:sameAs relation
    store.add(Quad(merge_uri, OWL("sameAs"), keep_uri))

    # Update all references to merged entity
    update_query = f"""
    PREFIX owl: <http://www.w3.org/2002/07/owl#>

    DELETE {{
        ?s ?p <{merge_uri}> .
        <{merge_uri}> ?p2 ?o2 .
    }}
    INSERT {{
        ?s ?p <{keep_uri}> .
        <{keep_uri}> ?p2 ?o2 .
    }}
    WHERE {{
        {{
            ?s ?p <{merge_uri}> .
        }} UNION {{
            <{merge_uri}> ?p2 ?o2 .
            FILTER(?p2 != owl:sameAs)
        }}
    }}
    """

    store.update(update_query)
    print(f"Merged {merge_uri} into {keep_uri}")

# Usage
store = Store("./academic-kg")

print("Finding duplicate authors...")
duplicates = find_duplicate_authors(store, threshold=0.95)

for dup in duplicates[:10]:  # Show first 10
    print(f"  {dup[2]} ≈ {dup[3]} ({dup[4]:.2f})")

# Manual review and merge
# merge_entities(store, duplicates[0][0], duplicates[0][1])
```

### Link to External Knowledge Bases

```python
def link_to_wikidata(store):
    """
    Find Wikidata IDs for entities.
    """
    query = """
    PREFIX acad: <http://academic.example.org/>

    SELECT ?author ?name
    WHERE {
        ?author a acad:Person .
        ?author acad:name ?name .
        FILTER NOT EXISTS { ?author acad:wikidataId ?wdId }
    }
    LIMIT 100
    """

    ACAD = lambda s: NamedNode(f"http://academic.example.org/{s}")

    results = store.query(query)

    for result in results:
        author_uri = result['author']
        name = result['name'].value

        # Search Wikidata
        wikidata_query = f"""
        SELECT ?person
        WHERE {{
            SERVICE <https://query.wikidata.org/sparql> {{
                ?person rdfs:label "{name}"@en .
                ?person wdt:P106 wd:Q901 .  # occupation: scientist
            }}
        }}
        LIMIT 1
        """

        try:
            wd_results = list(store.query(wikidata_query))
            if wd_results:
                wikidata_id = wd_results[0]['person'].value.split('/')[-1]

                # Add link
                store.add(Quad(
                    author_uri,
                    ACAD("wikidataId"),
                    Literal(wikidata_id)
                ))

                print(f"Linked {name} to {wikidata_id}")

            time.sleep(0.5)  # Rate limiting

        except Exception as e:
            print(f"Error linking {name}: {e}")
```

## Step 4: Quality Assurance

### Validate with SHACL

```turtle
# validation-shapes.ttl

@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix acad: <http://academic.example.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

# ============================================
# Person Shape
# ============================================

acad:PersonShape a sh:NodeShape ;
    sh:targetClass acad:Person ;
    sh:property [
        sh:path acad:name ;
        sh:datatype xsd:string ;
        sh:minCount 1 ;
        sh:maxCount 1 ;
        sh:message "Person must have exactly one name"
    ] ;
    sh:property [
        sh:path acad:email ;
        sh:datatype xsd:string ;
        sh:maxCount 1 ;
        sh:pattern "^[^@]+@[^@]+\\.[^@]+$" ;
        sh:message "Email must be valid format"
    ] ;
    sh:property [
        sh:path acad:orcid ;
        sh:datatype xsd:string ;
        sh:maxCount 1 ;
        sh:pattern "^\\d{4}-\\d{4}-\\d{4}-\\d{3}[0-9X]$" ;
        sh:message "ORCID must be valid format (0000-0000-0000-0000)"
    ] .

# ============================================
# Publication Shape
# ============================================

acad:PublicationShape a sh:NodeShape ;
    sh:targetClass acad:Publication ;
    sh:property [
        sh:path acad:title ;
        sh:datatype xsd:string ;
        sh:minCount 1 ;
        sh:message "Publication must have a title"
    ] ;
    sh:property [
        sh:path acad:author ;
        sh:class acad:Person ;
        sh:minCount 1 ;
        sh:message "Publication must have at least one author"
    ] ;
    sh:property [
        sh:path acad:publicationYear ;
        sh:datatype xsd:gYear ;
        sh:minInclusive "1800"^^xsd:gYear ;
        sh:maxInclusive "2030"^^xsd:gYear ;
        sh:message "Publication year must be between 1800 and 2030"
    ] ;
    sh:property [
        sh:path acad:doi ;
        sh:datatype xsd:string ;
        sh:pattern "^10\\.\\d{4,9}/[-._;()/:A-Z0-9]+$"^^xsd:string ;
        sh:flags "i" ;
        sh:message "DOI must be valid format"
    ] .
```

Validate in Rust:

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use std::fs::File;
use std::io::BufReader;

fn validate_knowledge_graph() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open("./academic-kg")?;

    // Load SHACL shapes
    let shapes_file = File::open("validation-shapes.ttl")?;
    let shapes_reader = BufReader::new(shapes_file);

    // Note: SHACL validation requires the sparshacl crate
    // This is a simplified example

    println!("Validation complete!");

    Ok(())
}
```

### Data Quality Checks

```python
def check_data_quality(store):
    """
    Run data quality checks.
    """
    checks = [
        ("Publications without authors", """
            PREFIX acad: <http://academic.example.org/>
            SELECT (COUNT(*) as ?count)
            WHERE {
                ?pub a acad:Publication .
                FILTER NOT EXISTS { ?pub acad:author ?author }
            }
        """),

        ("Authors without publications", """
            PREFIX acad: <http://academic.example.org/>
            SELECT (COUNT(*) as ?count)
            WHERE {
                ?author a acad:Person .
                FILTER NOT EXISTS { ?pub acad:author ?author }
            }
        """),

        ("Publications without year", """
            PREFIX acad: <http://academic.example.org/>
            SELECT (COUNT(*) as ?count)
            WHERE {
                ?pub a acad:Publication .
                FILTER NOT EXISTS { ?pub acad:publicationYear ?year }
            }
        """),

        ("Duplicate DOIs", """
            PREFIX acad: <http://academic.example.org/>
            SELECT ?doi (COUNT(*) as ?count)
            WHERE {
                ?pub acad:doi ?doi .
            }
            GROUP BY ?doi
            HAVING (COUNT(*) > 1)
        """),
    ]

    print("=== Data Quality Report ===\n")

    for check_name, query in checks:
        results = list(store.query(query))

        if 'count' in results[0]:
            count = int(results[0]['count'].value)
            status = "✗" if count > 0 else "✓"
            print(f"{status} {check_name}: {count}")
        else:
            print(f"✗ {check_name}: {len(results)} issues")

# Usage
store = Store("./academic-kg")
check_data_quality(store)
```

## Step 5: Query Interface

### REST API

```python
# api.py

from flask import Flask, request, jsonify
from pyoxigraph import Store
import json

app = Flask(__name__)
store = Store("./academic-kg")

@app.route("/api/search/publications")
def search_publications():
    """Search publications by keyword"""
    q = request.args.get('q', '')
    limit = int(request.args.get('limit', 20))

    query = f"""
    PREFIX acad: <http://academic.example.org/>

    SELECT ?pub ?title ?year ?authors
    WHERE {{
        ?pub a acad:Publication .
        ?pub acad:title ?title .
        OPTIONAL {{ ?pub acad:publicationYear ?year }}

        FILTER(CONTAINS(LCASE(?title), LCASE("{q}")))

        {{
            SELECT ?pub (GROUP_CONCAT(?authorName; SEPARATOR=", ") as ?authors)
            WHERE {{
                ?pub acad:author ?author .
                ?author acad:name ?authorName .
            }}
            GROUP BY ?pub
        }}
    }}
    ORDER BY DESC(?year)
    LIMIT {limit}
    """

    results = []
    for result in store.query(query):
        results.append({
            'uri': result['pub'].value,
            'title': result['title'].value,
            'year': result.get('year', {}).get('value', 'Unknown'),
            'authors': result.get('authors', {}).get('value', 'Unknown')
        })

    return jsonify(results)

@app.route("/api/author/<author_id>")
def get_author(author_id):
    """Get author details"""
    query = f"""
    PREFIX acad: <http://academic.example.org/>

    SELECT ?name ?email ?orcid ?affiliation (COUNT(?pub) as ?pubCount)
    WHERE {{
        acad:person/{author_id} acad:name ?name .
        OPTIONAL {{ acad:person/{author_id} acad:email ?email }}
        OPTIONAL {{ acad:person/{author_id} acad:orcid ?orcid }}
        OPTIONAL {{
            acad:person/{author_id} acad:affiliatedWith ?aff .
            ?aff acad:name ?affiliation .
        }}
        OPTIONAL {{
            ?pub acad:author acad:person/{author_id} .
        }}
    }}
    GROUP BY ?name ?email ?orcid ?affiliation
    """

    results = list(store.query(query))
    if not results:
        return jsonify({'error': 'Author not found'}), 404

    result = results[0]
    return jsonify({
        'id': author_id,
        'name': result['name'].value,
        'email': result.get('email', {}).get('value'),
        'orcid': result.get('orcid', {}).get('value'),
        'affiliation': result.get('affiliation', {}).get('value'),
        'publication_count': int(result['pubCount'].value)
    })

@app.route("/api/sparql", methods=['GET', 'POST'])
def sparql_endpoint():
    """SPARQL endpoint"""
    if request.method == 'POST':
        query = request.data.decode('utf-8')
    else:
        query = request.args.get('query', '')

    try:
        results = []
        for result in store.query(query):
            row = {}
            for var, value in result.items():
                row[var] = value.value
            results.append(row)

        return jsonify({'results': results})

    except Exception as e:
        return jsonify({'error': str(e)}), 400

if __name__ == "__main__":
    app.run(debug=True, port=8000)
```

### GraphQL Interface

```python
# graphql_api.py

from ariadne import QueryType, make_executable_schema, graphql_sync
from ariadne.constants import PLAYGROUND_HTML
from flask import Flask, request, jsonify
from pyoxigraph import Store

app = Flask(__name__)
store = Store("./academic-kg")

# GraphQL schema
type_defs = """
    type Query {
        publication(id: ID!): Publication
        publications(search: String, limit: Int): [Publication!]!
        author(id: ID!): Author
        authors(search: String, limit: Int): [Author!]!
    }

    type Publication {
        id: ID!
        title: String!
        year: Int
        doi: String
        authors: [Author!]!
        citations: [Publication!]!
    }

    type Author {
        id: ID!
        name: String!
        email: String
        orcid: String
        publications: [Publication!]!
    }
"""

query = QueryType()

@query.field("publications")
def resolve_publications(_, info, search=None, limit=20):
    filter_clause = f'FILTER(CONTAINS(LCASE(?title), LCASE("{search}")))' if search else ''

    sparql = f"""
    PREFIX acad: <http://academic.example.org/>

    SELECT ?pub ?title ?year ?doi
    WHERE {{
        ?pub a acad:Publication .
        ?pub acad:title ?title .
        OPTIONAL {{ ?pub acad:publicationYear ?year }}
        OPTIONAL {{ ?pub acad:doi ?doi }}
        {filter_clause}
    }}
    LIMIT {limit}
    """

    results = []
    for result in store.query(sparql):
        results.append({
            'id': result['pub'].value.split('/')[-1],
            'title': result['title'].value,
            'year': int(result.get('year', {}).get('value', 0)) or None,
            'doi': result.get('doi', {}).get('value'),
            'authors': [],  # Resolver will handle this
            'citations': []
        })

    return results

schema = make_executable_schema(type_defs, query)

@app.route("/graphql", methods=['GET', 'POST'])
def graphql_server():
    if request.method == 'GET':
        return PLAYGROUND_HTML, 200

    data = request.get_json()
    success, result = graphql_sync(schema, data, context_value=request)

    return jsonify(result), 200 if success else 400

if __name__ == "__main__":
    app.run(debug=True, port=8080)
```

## Complete Build Script

```python
#!/usr/bin/env python3
"""
build_knowledge_graph.py

Complete script to build academic knowledge graph.
"""

from pyoxigraph import Store
import sys

def main():
    print("=== Building Academic Knowledge Graph ===\n")

    # Step 1: Create store and load ontology
    print("Step 1: Loading ontology...")
    store = Store("./academic-kg")

    with open("schema.ttl", "rb") as f:
        store.load(f, "text/turtle")
    print("  ✓ Ontology loaded\n")

    # Step 2: Ingest data
    print("Step 2: Ingesting data...")
    ingest_publications_csv(store, "publications.csv")
    ingest_from_crossref(store, "semantic web", max_results=50)
    print("  ✓ Data ingested\n")

    # Step 3: Entity linking
    print("Step 3: Linking entities...")
    duplicates = find_duplicate_authors(store)
    print(f"  Found {len(duplicates)} potential duplicates")
    print("  ✓ Entity linking complete\n")

    # Step 4: Quality check
    print("Step 4: Quality assurance...")
    check_data_quality(store)
    print("  ✓ Quality check complete\n")

    # Step 5: Statistics
    print("Step 5: Final statistics...")
    stats_query = """
    PREFIX acad: <http://academic.example.org/>

    SELECT
        (COUNT(DISTINCT ?pub) as ?publications)
        (COUNT(DISTINCT ?author) as ?authors)
        (COUNT(DISTINCT ?journal) as ?journals)
    WHERE {
        OPTIONAL { ?pub a acad:Publication }
        OPTIONAL { ?author a acad:Person }
        OPTIONAL { ?journal a acad:Journal }
    }
    """

    result = list(store.query(stats_query))[0]
    print(f"  Publications: {result['publications'].value}")
    print(f"  Authors: {result['authors'].value}")
    print(f"  Journals: {result['journals'].value}")

    print("\n✓ Knowledge graph build complete!")

if __name__ == "__main__":
    main()
```

## Deployment

### Docker Deployment

```dockerfile
# Dockerfile

FROM python:3.11-slim

WORKDIR /app

# Install dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application
COPY . .

# Create data directory
RUN mkdir -p ./academic-kg

# Expose API port
EXPOSE 8000

# Run API server
CMD ["python", "api.py"]
```

```yaml
# docker-compose.yml

version: '3.8'

services:
  oxigraph-kg:
    build: .
    ports:
      - "8000:8000"
    volumes:
      - ./data:/app/academic-kg
    environment:
      - FLASK_ENV=production
    restart: unless-stopped
```

## Next Steps

- Review [SPARQL Query Guide](../reference/sparql.md)
- Explore [Performance Tuning](../how-to/performance-tuning.md)
- Check [SHACL Validation](../reference/shacl.md)

## Additional Resources

- [Knowledge Graph Best Practices](https://www.w3.org/TR/dwbp/)
- [RDF Data Modeling](https://www.w3.org/TR/rdf11-primer/)
- [SPARQL Query Language](https://www.w3.org/TR/sparql11-query/)
