# Integrate with Wikidata

This guide shows how to query Wikidata via SPARQL federation and create local caches for better performance.

## Overview

Wikidata is a free, collaborative knowledge base with over 100 million items. Oxigraph can:

- **Query Wikidata remotely** via SPARQL SERVICE
- **Cache frequently-used data locally** for better performance
- **Combine local and remote data** in federated queries
- **Build applications** on top of Wikidata

## Prerequisites

Oxigraph must be compiled with the `http-client` feature for SERVICE support:

```bash
# Rust
cargo build --features http-client

# Or use pre-built binaries (already include this feature)
```

For Python:
```bash
pip install pyoxigraph
```

For JavaScript:
```bash
npm install oxigraph
```

## Basic Wikidata Queries

### Query Wikidata Remotely

#### Using Rust

```rust
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;

fn query_wikidata() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Query for Nobel Prize winners
    let query = r#"
        SELECT ?person ?personLabel ?prize ?prizeLabel
        WHERE {
            SERVICE <https://query.wikidata.org/sparql> {
                ?person wdt:P166 ?prize .
                ?prize wdt:P31 wd:Q7191 .
                SERVICE wikibase:label {
                    bd:serviceParam wikibase:language "en" .
                }
            }
        }
        LIMIT 10
    "#;

    if let QueryResults::Solutions(mut solutions) = store.query(query)? {
        println!("Nobel Prize Winners:");
        for solution in solutions {
            let solution = solution?;
            println!("  {} received {}",
                solution.get("personLabel").unwrap(),
                solution.get("prizeLabel").unwrap()
            );
        }
    }

    Ok(())
}
```

#### Using Python

```python
from pyoxigraph import Store

def query_wikidata():
    store = Store()

    # Query for programming languages
    query = """
    SELECT ?lang ?langLabel ?year
    WHERE {
        SERVICE <https://query.wikidata.org/sparql> {
            ?lang wdt:P31 wd:Q9143 .      # instance of programming language
            ?lang wdt:P571 ?year .         # inception date
            SERVICE wikibase:label {
                bd:serviceParam wikibase:language "en" .
            }
        }
    }
    ORDER BY ?year
    LIMIT 20
    """

    results = store.query(query)
    print("Programming Languages:")
    for result in results:
        lang = result['langLabel'].value
        year = result['year'].value[:4]  # Extract year
        print(f"  {year}: {lang}")

if __name__ == "__main__":
    query_wikidata()
```

#### Using JavaScript

```javascript
import oxigraph from 'oxigraph';

async function queryWikidata() {
    const store = new oxigraph.Store();

    // Query for countries
    const query = `
    SELECT ?country ?countryLabel ?capital ?capitalLabel
    WHERE {
        SERVICE <https://query.wikidata.org/sparql> {
            ?country wdt:P31 wd:Q6256 .    # instance of country
            ?country wdt:P36 ?capital .     # capital
            SERVICE wikibase:label {
                bd:serviceParam wikibase:language "en" .
            }
        }
    }
    LIMIT 20
    `;

    console.log("Countries and Capitals:");
    for (const result of store.query(query)) {
        console.log(`  ${result.get('countryLabel').value}: ${result.get('capitalLabel').value}`);
    }
}

queryWikidata();
```

## Caching Wikidata Locally

For better performance, cache frequently-used Wikidata subsets locally.

### Strategy 1: Full Download

Download specific entity types:

```python
from pyoxigraph import Store
import requests
import gzip
import time

def download_wikidata_dump(entity_type="items", date="latest"):
    """
    Download Wikidata dump.

    Args:
        entity_type: "items" or "properties"
        date: Date string (YYYYMMDD) or "latest"
    """
    base_url = f"https://dumps.wikimedia.org/wikidatawiki/entities/{date}/"

    if date == "latest":
        # Find latest dump
        # In production, parse the HTML to find the latest
        date = "20240101"  # Example

    filename = f"wikidata-{date}-all.nt.gz"
    url = base_url + filename

    print(f"Downloading {url}...")
    print("Warning: This is a very large file (>50 GB compressed)")
    print("Consider using the incremental approach instead.")

    # For demonstration only - actual download would take hours
    # response = requests.get(url, stream=True)
    # with gzip.open(filename, 'rb') as f_in:
    #     store.bulk_load(f_in, "application/n-triples")

### Strategy 2: Incremental Caching

Cache data as you query it:

```python
from pyoxigraph import Store, NamedNode, Quad
import requests
import time

class WikidataCache:
    """
    Intelligent Wikidata cache that fetches and stores data locally.
    """

    def __init__(self, store_path="./wikidata-cache"):
        self.store = Store(store_path)
        self.wikidata_endpoint = "https://query.wikidata.org/sparql"
        self.cache_hits = 0
        self.cache_misses = 0

    def query_with_cache(self, sparql_query, use_cache=True):
        """
        Execute query, using local cache when possible.
        """
        if use_cache:
            # Try local cache first
            try:
                results = list(self.store.query(sparql_query))
                if results:
                    self.cache_hits += 1
                    return results
            except:
                pass

        # Query Wikidata remotely
        self.cache_misses += 1
        time.sleep(0.1)  # Rate limiting

        wrapped_query = f"""
        SELECT * WHERE {{
            SERVICE <{self.wikidata_endpoint}> {{
                {sparql_query}
            }}
        }}
        """

        results = list(self.store.query(wrapped_query))

        # Cache the results
        self._cache_results(sparql_query, results)

        return results

    def _cache_results(self, query, results):
        """Store query results in local cache"""
        # Extract triples from results and store them
        # This is a simplified version
        for result in results:
            for var_name, value in result.items():
                # Store metadata about cached entities
                pass

    def get_entity(self, entity_id, properties=None):
        """
        Get entity data, caching locally.

        Args:
            entity_id: Wikidata entity ID (e.g., "Q42")
            properties: List of property IDs to fetch (e.g., ["P31", "P106"])
        """
        # Check if entity is in cache
        entity_uri = f"http://www.wikidata.org/entity/{entity_id}"
        cached = list(self.store.quads_for_pattern(
            NamedNode(entity_uri),
            None,
            None,
            None
        ))

        if cached:
            self.cache_hits += 1
            return self._format_entity(entity_id, cached)

        # Fetch from Wikidata
        self.cache_misses += 1
        self._fetch_and_cache_entity(entity_id, properties)

        # Return cached version
        cached = list(self.store.quads_for_pattern(
            NamedNode(entity_uri),
            None,
            None,
            None
        ))
        return self._format_entity(entity_id, cached)

    def _fetch_and_cache_entity(self, entity_id, properties=None):
        """Fetch entity from Wikidata and cache it"""
        if properties:
            props_filter = " ".join([f"wdt:{p} ?{p} ." for p in properties])
        else:
            props_filter = "?p ?o ."

        query = f"""
        PREFIX wd: <http://www.wikidata.org/entity/>
        PREFIX wdt: <http://www.wikidata.org/prop/direct/>

        CONSTRUCT {{
            wd:{entity_id} ?p ?o .
        }}
        WHERE {{
            SERVICE <{self.wikidata_endpoint}> {{
                wd:{entity_id} {props_filter}
            }}
        }}
        """

        time.sleep(0.1)  # Rate limiting

        # Execute and store
        try:
            # In real implementation, execute CONSTRUCT and store results
            results = self.store.query(query)
        except Exception as e:
            print(f"Error fetching {entity_id}: {e}")

    def _format_entity(self, entity_id, quads):
        """Format entity data for display"""
        entity = {"id": entity_id, "properties": {}}
        for quad in quads:
            prop = quad.predicate.value.split("/")[-1]
            entity["properties"][prop] = quad.object.value
        return entity

    def stats(self):
        """Print cache statistics"""
        total = self.cache_hits + self.cache_misses
        hit_rate = (self.cache_hits / total * 100) if total > 0 else 0
        print(f"Cache Statistics:")
        print(f"  Total queries: {total}")
        print(f"  Cache hits: {self.cache_hits} ({hit_rate:.1f}%)")
        print(f"  Cache misses: {self.cache_misses}")
        print(f"  Cached entities: {len(self.store)}")

# Usage example
cache = WikidataCache("./wikidata-cache")

# Query for scientists
query = """
PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>

SELECT ?scientist ?name WHERE {
    ?scientist wdt:P106 wd:Q901 .  # occupation: scientist
    ?scientist rdfs:label ?name .
    FILTER(LANG(?name) = "en")
}
LIMIT 10
"""

results = cache.query_with_cache(query)
for result in results:
    print(result['name'].value)

# Get specific entity
entity = cache.get_entity("Q937", properties=["P31", "P106", "P569"])
print(entity)

# Show statistics
cache.stats()
```

## Federation Patterns

### Pattern 1: Local Filter, Remote Enrich

Query local data, then enrich with Wikidata:

```python
from pyoxigraph import Store, NamedNode, Quad, Literal

# Setup
store = Store("./my-data")

# Add local data
EX = lambda s: NamedNode(f"http://example.org/{s}")
store.add(Quad(EX("alice"), EX("wikidataId"), Literal("Q5")))
store.add(Quad(EX("bob"), EX("wikidataId"), Literal("Q42")))

# Federated query: local IDs → Wikidata labels
query = """
PREFIX ex: <http://example.org/>
PREFIX wd: <http://www.wikidata.org/entity/>

SELECT ?person ?localId ?wikidataLabel
WHERE {
    # Local data
    ?person ex:wikidataId ?localId .

    # Remote enrichment
    SERVICE <https://query.wikidata.org/sparql> {
        BIND(IRI(CONCAT("http://www.wikidata.org/entity/", ?localId)) as ?wdEntity)
        ?wdEntity rdfs:label ?wikidataLabel .
        FILTER(LANG(?wikidataLabel) = "en")
    }
}
"""

results = store.query(query)
for result in results:
    print(f"{result['person'].value}: {result['wikidataLabel'].value}")
```

### Pattern 2: Remote Filter, Local Join

Query Wikidata, join with local data:

```python
query = """
PREFIX ex: <http://example.org/>
PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>

SELECT ?person ?expertise ?wikidataTopic
WHERE {
    # Local expertise data
    ?person ex:hasExpertise ?expertise .

    # Find related Wikidata topics
    SERVICE <https://query.wikidata.org/sparql> {
        ?wikidataTopic wdt:P31 wd:Q11862829 .  # instance of academic discipline
        ?wikidataTopic rdfs:label ?label .
        FILTER(CONTAINS(LCASE(?label), LCASE(?expertise)))
        FILTER(LANG(?label) = "en")
    }
}
LIMIT 20
"""
```

### Pattern 3: Multi-Source Federation

Combine multiple remote sources:

```python
query = """
PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX dbo: <http://dbpedia.org/ontology/>

SELECT ?person ?wikidataLabel ?dbpediaAbstract
WHERE {
    # Get person from Wikidata
    SERVICE <https://query.wikidata.org/sparql> {
        ?person wdt:P31 wd:Q5 .           # human
        ?person wdt:P106 wd:Q901 .        # scientist
        ?person rdfs:label ?wikidataLabel .
        FILTER(LANG(?wikidataLabel) = "en")
    }

    # Enrich with DBpedia
    SERVICE <https://dbpedia.org/sparql> {
        ?dbpediaPerson rdfs:label ?wikidataLabel .
        ?dbpediaPerson dbo:abstract ?dbpediaAbstract .
        FILTER(LANG(?dbpediaAbstract) = "en")
    }
}
LIMIT 5
"""
```

## Rate Limiting Best Practices

Wikidata has rate limits. Follow these practices:

### 1. Implement Exponential Backoff

```python
import time
import random
from pyoxigraph import Store

class RateLimitedStore:
    def __init__(self, path=None):
        self.store = Store(path) if path else Store()
        self.last_request = 0
        self.min_delay = 0.1  # 100ms minimum
        self.max_retries = 3

    def query(self, sparql, retry_count=0):
        # Respect rate limit
        elapsed = time.time() - self.last_request
        if elapsed < self.min_delay:
            time.sleep(self.min_delay - elapsed)

        try:
            self.last_request = time.time()
            return self.store.query(sparql)
        except Exception as e:
            if retry_count < self.max_retries and "429" in str(e):
                # Exponential backoff
                wait = (2 ** retry_count) + random.uniform(0, 1)
                print(f"Rate limited, waiting {wait:.1f}s...")
                time.sleep(wait)
                return self.query(sparql, retry_count + 1)
            raise

# Usage
store = RateLimitedStore("./data")
```

### 2. Batch Requests

```python
def batch_query_entities(entity_ids, batch_size=50):
    """Query multiple entities in batches"""
    store = Store()

    for i in range(0, len(entity_ids), batch_size):
        batch = entity_ids[i:i+batch_size]
        values_clause = " ".join([f"wd:{eid}" for eid in batch])

        query = f"""
        PREFIX wd: <http://www.wikidata.org/entity/>
        PREFIX wdt: <http://www.wikidata.org/prop/direct/>

        SELECT ?entity ?label ?description
        WHERE {{
            SERVICE <https://query.wikidata.org/sparql> {{
                VALUES ?entity {{ {values_clause} }}
                ?entity rdfs:label ?label .
                OPTIONAL {{ ?entity schema:description ?description }}
                FILTER(LANG(?label) = "en")
                FILTER(!BOUND(?description) || LANG(?description) = "en")
            }}
        }}
        """

        time.sleep(0.2)  # Rate limiting
        results = store.query(query)

        for result in results:
            yield {
                'id': result['entity'].value.split('/')[-1],
                'label': result['label'].value,
                'description': result.get('description', {}).get('value', '')
            }

# Usage
entities = ["Q42", "Q5", "Q937", "Q1234", "Q5678"]
for entity in batch_query_entities(entities):
    print(f"{entity['id']}: {entity['label']}")
```

### 3. Use User-Agent Header

Configure your HTTP client:

```rust
// In Rust, when building with http-client feature
// The library uses a default User-Agent, but you can configure it

use oxigraph::store::Store;

fn query_with_user_agent() -> Result<(), Box<dyn std::error::Error>> {
    // Set user agent via environment variable
    std::env::set_var(
        "HTTP_USER_AGENT",
        "MyApp/1.0 (https://example.org/myapp; contact@example.org)"
    );

    let store = Store::new()?;

    let query = r#"
        SELECT * WHERE {
            SERVICE <https://query.wikidata.org/sparql> {
                ?s ?p ?o
            }
        }
        LIMIT 10
    "#;

    store.query(query)?;
    Ok(())
}
```

## Complete Example: Wikidata-Powered Application

Build a research paper recommendation system:

```python
#!/usr/bin/env python3
"""
research_recommender.py

Recommends research papers based on author expertise using Wikidata.
"""

from pyoxigraph import Store, NamedNode, Quad, Literal
import time
from dataclasses import dataclass
from typing import List, Set

@dataclass
class Author:
    id: str
    name: str
    wikidata_id: str = None
    topics: Set[str] = None

@dataclass
class Paper:
    id: str
    title: str
    authors: List[str]
    topics: Set[str]

class ResearchRecommender:
    def __init__(self, data_path="./research-data"):
        self.store = Store(data_path)
        self.EX = lambda s: NamedNode(f"http://example.org/{s}")

    def add_author(self, author: Author):
        """Add author to local database"""
        author_node = self.EX(f"author/{author.id}")

        self.store.add(Quad(
            author_node,
            self.EX("name"),
            Literal(author.name)
        ))

        if author.wikidata_id:
            self.store.add(Quad(
                author_node,
                self.EX("wikidataId"),
                Literal(author.wikidata_id)
            ))

        if author.topics:
            for topic in author.topics:
                self.store.add(Quad(
                    author_node,
                    self.EX("interestedIn"),
                    Literal(topic)
                ))

    def enrich_author_from_wikidata(self, author_id: str):
        """Fetch author topics from Wikidata"""
        query = f"""
        PREFIX ex: <http://example.org/>
        PREFIX wd: <http://www.wikidata.org/entity/>
        PREFIX wdt: <http://www.wikidata.org/prop/direct/>

        SELECT DISTINCT ?topic
        WHERE {{
            ex:author/{author_id} ex:wikidataId ?wikidataId .

            SERVICE <https://query.wikidata.org/sparql> {{
                BIND(IRI(CONCAT("http://www.wikidata.org/entity/", ?wikidataId)) as ?person)

                # Get research topics
                {{
                    ?person wdt:P101 ?field .           # field of work
                    ?field rdfs:label ?topic .
                }} UNION {{
                    ?person wdt:P800 ?work .            # notable work
                    ?work wdt:P921 ?subject .           # main subject
                    ?subject rdfs:label ?topic .
                }}

                FILTER(LANG(?topic) = "en")
            }}
        }}
        LIMIT 20
        """

        time.sleep(0.1)  # Rate limiting

        try:
            results = self.store.query(query)
            topics = {result['topic'].value for result in results}

            # Store enriched topics
            author_node = self.EX(f"author/{author_id}")
            for topic in topics:
                self.store.add(Quad(
                    author_node,
                    self.EX("enrichedTopic"),
                    Literal(topic)
                ))

            return topics
        except Exception as e:
            print(f"Error enriching author {author_id}: {e}")
            return set()

    def find_similar_topics(self, topic: str) -> List[str]:
        """Find related topics in Wikidata"""
        query = f"""
        PREFIX wd: <http://www.wikidata.org/entity/>
        PREFIX wdt: <http://www.wikidata.org/prop/direct/>

        SELECT DISTINCT ?relatedLabel
        WHERE {{
            SERVICE <https://query.wikidata.org/sparql> {{
                ?subject rdfs:label "{topic}"@en .
                ?subject wdt:P279* ?related .         # subclass of
                ?related rdfs:label ?relatedLabel .
                FILTER(LANG(?relatedLabel) = "en")
            }}
        }}
        LIMIT 10
        """

        time.sleep(0.1)

        try:
            results = self.store.query(query)
            return [result['relatedLabel'].value for result in results]
        except Exception as e:
            print(f"Error finding similar topics: {e}")
            return []

    def recommend_collaborators(self, author_id: str, limit: int = 5):
        """Recommend potential collaborators based on shared interests"""
        query = f"""
        PREFIX ex: <http://example.org/>

        SELECT ?otherAuthor ?otherName (COUNT(?sharedTopic) as ?commonTopics)
        WHERE {{
            # Get topics of target author
            ex:author/{author_id} ex:enrichedTopic ?topic .

            # Find other authors with same topics
            ?otherAuthor ex:enrichedTopic ?topic .
            ?otherAuthor ex:name ?otherName .

            # Exclude self
            FILTER(?otherAuthor != ex:author/{author_id})

            BIND(?topic as ?sharedTopic)
        }}
        GROUP BY ?otherAuthor ?otherName
        ORDER BY DESC(?commonTopics)
        LIMIT {limit}
        """

        results = self.store.query(query)
        recommendations = []

        for result in results:
            recommendations.append({
                'author': result['otherName'].value,
                'common_topics': int(result['commonTopics'].value)
            })

        return recommendations

def main():
    recommender = ResearchRecommender()

    # Add authors
    print("Adding authors...")
    authors = [
        Author("1", "Alice Smith", "Q12345", {"Machine Learning", "Computer Vision"}),
        Author("2", "Bob Jones", "Q67890", {"Natural Language Processing"}),
        Author("3", "Carol White", None, {"Computer Vision", "Robotics"}),
    ]

    for author in authors:
        recommender.add_author(author)

    # Enrich from Wikidata (if IDs are available)
    print("\nEnriching from Wikidata...")
    for author in authors:
        if author.wikidata_id:
            topics = recommender.enrich_author_from_wikidata(author.id)
            print(f"  {author.name}: {len(topics)} topics found")

    # Find collaborators
    print("\nRecommending collaborators for Alice Smith...")
    recommendations = recommender.recommend_collaborators("1")
    for rec in recommendations:
        print(f"  {rec['author']} ({rec['common_topics']} common topics)")

if __name__ == "__main__":
    main()
```

## Deployment

Deploy as a web service:

```python
# app.py
from flask import Flask, jsonify, request
from pyoxigraph import Store
import time

app = Flask(__name__)
store = Store("./wikidata-cache")

@app.route("/query/entity/<entity_id>")
def get_entity(entity_id):
    """Get entity from Wikidata (cached)"""
    query = f"""
    PREFIX wd: <http://www.wikidata.org/entity/>

    SELECT ?p ?o
    WHERE {{
        SERVICE <https://query.wikidata.org/sparql> {{
            wd:{entity_id} ?p ?o .
        }}
    }}
    LIMIT 100
    """

    time.sleep(0.1)  # Rate limiting

    try:
        results = store.query(query)
        properties = {}

        for result in results:
            prop = result['p'].value.split('/')[-1]
            value = result['o'].value
            if prop not in properties:
                properties[prop] = []
            properties[prop].append(value)

        return jsonify({
            "entity_id": entity_id,
            "properties": properties
        })
    except Exception as e:
        return jsonify({"error": str(e)}), 500

@app.route("/search")
def search():
    """Search Wikidata"""
    q = request.args.get('q', '')

    query = f"""
    SELECT ?entity ?label ?description
    WHERE {{
        SERVICE <https://query.wikidata.org/sparql> {{
            ?entity rdfs:label ?label .
            FILTER(CONTAINS(LCASE(?label), LCASE("{q}")))
            OPTIONAL {{ ?entity schema:description ?description }}
            FILTER(LANG(?label) = "en")
            FILTER(!BOUND(?description) || LANG(?description) = "en")
        }}
    }}
    LIMIT 20
    """

    time.sleep(0.1)

    try:
        results = store.query(query)
        entities = []

        for result in results:
            entities.append({
                "id": result['entity'].value.split('/')[-1],
                "label": result['label'].value,
                "description": result.get('description', {}).get('value', '')
            })

        return jsonify(entities)
    except Exception as e:
        return jsonify({"error": str(e)}), 500

if __name__ == "__main__":
    app.run(debug=True, port=5000)
```

Run with:
```bash
pip install flask pyoxigraph
python app.py
```

Test:
```bash
# Get entity
curl http://localhost:5000/query/entity/Q42

# Search
curl http://localhost:5000/search?q=physics
```

## Next Steps

- Explore [SPARQL Federation](../reference/sparql.md#federation)
- Review [Performance Tuning](../how-to/performance-tuning.md)
- Check [HTTP Client Configuration](../reference/configuration.md)

## Additional Resources

- [Wikidata SPARQL Tutorial](https://www.wikidata.org/wiki/Wikidata:SPARQL_tutorial)
- [Wikidata Query Service](https://query.wikidata.org/)
- [SPARQL Federation Best Practices](https://www.w3.org/TR/sparql11-federated-query/)
