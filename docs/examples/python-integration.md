# Python Integration Patterns

Complete integration examples for Pyoxigraph in Python applications, covering web frameworks, data science tools, CLI applications, and production deployments.

## Table of Contents

1. [Flask REST API](#flask-rest-api)
2. [FastAPI Async Service](#fastapi-async-service)
3. [Django Integration](#django-integration)
4. [Jupyter Notebook Usage](#jupyter-notebook-usage)
5. [Pandas Integration](#pandas-integration)
6. [CLI Tools with Click](#cli-tools-with-click)
7. [Celery Background Jobs](#celery-background-jobs)
8. [Production Deployment](#production-deployment)

## Flask REST API

Complete REST API with Flask, supporting SPARQL queries and RDF data management.

### requirements.txt

```
pyoxigraph>=0.3.20
flask>=3.0.0
flask-cors>=4.0.0
python-dotenv>=1.0.0
gunicorn>=21.2.0
```

### app.py

```python
from flask import Flask, request, jsonify
from flask_cors import CORS
from pyoxigraph import Store, NamedNode, Literal, Quad, Triple
import os
import logging
from functools import wraps
from typing import Optional

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Initialize Flask app
app = Flask(__name__)
CORS(app)

# Configuration
STORE_PATH = os.getenv('OXIGRAPH_PATH', './data/oxigraph')

# Initialize store
logger.info(f"Initializing Oxigraph store at {STORE_PATH}")
store = Store(path=STORE_PATH)

# Error handling decorator
def handle_errors(f):
    @wraps(f)
    def decorated_function(*args, **kwargs):
        try:
            return f(*args, **kwargs)
        except ValueError as e:
            logger.error(f"Validation error: {e}")
            return jsonify({"error": str(e)}), 400
        except Exception as e:
            logger.error(f"Internal error: {e}", exc_info=True)
            return jsonify({"error": "Internal server error"}), 500
    return decorated_function

# Helper functions
def term_to_dict(term):
    """Convert an RDF term to a JSON-serializable dictionary."""
    if isinstance(term, NamedNode):
        return {
            "type": "NamedNode",
            "value": str(term.value)
        }
    elif isinstance(term, Literal):
        result = {
            "type": "Literal",
            "value": term.value
        }
        if term.language:
            result["language"] = term.language
        elif term.datatype and str(term.datatype) != "http://www.w3.org/2001/XMLSchema#string":
            result["datatype"] = str(term.datatype)
        return result
    elif hasattr(term, '__class__') and term.__class__.__name__ == 'BlankNode':
        return {
            "type": "BlankNode",
            "value": str(term.value)
        }
    elif hasattr(term, '__class__') and term.__class__.__name__ == 'Triple':
        return {
            "type": "Triple",
            "subject": term_to_dict(term.subject),
            "predicate": term_to_dict(term.predicate),
            "object": term_to_dict(term.object)
        }
    else:
        return {
            "type": "Unknown",
            "value": str(term)
        }

def parse_term(term_data: dict):
    """Parse a term from JSON data."""
    term_type = term_data.get("type")
    value = term_data.get("value")

    if term_type == "NamedNode":
        return NamedNode(value)
    elif term_type == "Literal":
        if "language" in term_data:
            return Literal(value, language=term_data["language"])
        elif "datatype" in term_data:
            return Literal(value, datatype=NamedNode(term_data["datatype"]))
        else:
            return Literal(value)
    else:
        raise ValueError(f"Unsupported term type: {term_type}")

# Routes

@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({
        "status": "healthy",
        "service": "oxigraph-flask-api",
        "store_size": len(store)
    })

@app.route('/query', methods=['POST'])
@handle_errors
def execute_query():
    """Execute a SPARQL query."""
    data = request.get_json()

    if not data or 'query' not in data:
        return jsonify({"error": "Missing 'query' field"}), 400

    query = data['query']
    logger.info(f"Executing SPARQL query: {query[:100]}...")

    results = store.query(query)

    # Handle different result types
    if hasattr(results, '__iter__'):
        # SELECT or CONSTRUCT query
        output = []
        for result in results:
            if isinstance(result, dict):
                # SELECT result - binding
                binding = {}
                for var, term in result.items():
                    binding[str(var)] = term_to_dict(term)
                output.append(binding)
            elif hasattr(result, 'subject'):
                # CONSTRUCT result - triple
                output.append({
                    "subject": term_to_dict(result.subject),
                    "predicate": term_to_dict(result.predicate),
                    "object": term_to_dict(result.object)
                })
            else:
                output.append(str(result))

        return jsonify({
            "results": output,
            "count": len(output)
        })
    else:
        # ASK query - boolean
        return jsonify({
            "result": bool(results)
        })

@app.route('/update', methods=['POST'])
@handle_errors
def execute_update():
    """Execute a SPARQL UPDATE query."""
    data = request.get_json()

    if not data or 'update' not in data:
        return jsonify({"error": "Missing 'update' field"}), 400

    update = data['update']
    logger.info(f"Executing SPARQL update: {update[:100]}...")

    store.update(update)

    return jsonify({"message": "Update executed successfully"}), 200

@app.route('/triples', methods=['POST'])
@handle_errors
def add_triple():
    """Add a triple to the store."""
    data = request.get_json()

    required_fields = ['subject', 'predicate', 'object']
    if not all(field in data for field in required_fields):
        return jsonify({"error": "Missing required fields"}), 400

    subject = parse_term(data['subject'])
    predicate = parse_term(data['predicate'])
    obj = parse_term(data['object'])

    # Add to default graph
    store.add(Quad(subject, predicate, obj))

    logger.info(f"Added triple: {subject} {predicate} {obj}")

    return jsonify({"message": "Triple added successfully"}), 201

@app.route('/triples', methods=['GET'])
@handle_errors
def get_triples():
    """Get triples with optional pattern matching."""
    # Parse query parameters
    limit = int(request.args.get('limit', 100))
    offset = int(request.args.get('offset', 0))

    triples = []
    for i, quad in enumerate(store):
        if i < offset:
            continue
        if len(triples) >= limit:
            break

        triples.append({
            "subject": term_to_dict(quad.subject),
            "predicate": term_to_dict(quad.predicate),
            "object": term_to_dict(quad.object),
            "graph": term_to_dict(quad.graph_name) if quad.graph_name else None
        })

    return jsonify({
        "triples": triples,
        "count": len(triples),
        "limit": limit,
        "offset": offset
    })

@app.route('/triples/<path:subject_iri>', methods=['GET'])
@handle_errors
def get_triples_for_subject(subject_iri: str):
    """Get all triples for a specific subject."""
    logger.info(f"Getting triples for subject: {subject_iri}")

    query = f"""
        SELECT ?predicate ?object WHERE {{
            <{subject_iri}> ?predicate ?object .
        }}
    """

    results = store.query(query)
    triples = []

    for result in results:
        triples.append({
            "predicate": term_to_dict(result['predicate']),
            "object": term_to_dict(result['object'])
        })

    return jsonify({
        "subject": subject_iri,
        "triples": triples,
        "count": len(triples)
    })

@app.route('/load', methods=['POST'])
@handle_errors
def load_data():
    """Load RDF data from request body."""
    data = request.get_json()

    if not data or 'content' not in data or 'format' not in data:
        return jsonify({"error": "Missing 'content' or 'format' field"}), 400

    content = data['content']
    mime_type = data['format']
    base_iri = data.get('base_iri')

    logger.info(f"Loading data with format: {mime_type}")

    # Convert format name to MIME type if needed
    format_map = {
        'turtle': 'text/turtle',
        'ttl': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nt': 'application/n-triples',
        'rdfxml': 'application/rdf+xml',
        'xml': 'application/rdf+xml',
        'nquads': 'application/n-quads',
        'nq': 'application/n-quads',
        'trig': 'application/trig',
        'jsonld': 'application/ld+json'
    }

    mime_type = format_map.get(mime_type.lower(), mime_type)

    store.load(content.encode('utf-8'), mime_type=mime_type, base_iri=base_iri)

    return jsonify({
        "message": "Data loaded successfully",
        "store_size": len(store)
    }), 201

@app.route('/export', methods=['GET'])
@handle_errors
def export_data():
    """Export RDF data in specified format."""
    format_param = request.args.get('format', 'turtle')

    format_map = {
        'turtle': 'text/turtle',
        'ttl': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nt': 'application/n-triples',
        'nquads': 'application/n-quads',
        'nq': 'application/n-quads',
        'trig': 'application/trig'
    }

    mime_type = format_map.get(format_param.lower(), 'text/turtle')

    logger.info(f"Exporting data as {mime_type}")

    # Serialize the store
    data = store.dump(mime_type=mime_type)

    return data, 200, {'Content-Type': mime_type}

@app.route('/stats', methods=['GET'])
def get_stats():
    """Get store statistics."""
    total_quads = len(store)

    # Count unique subjects
    subjects = set()
    predicates = set()
    objects = set()

    for quad in store:
        subjects.add(str(quad.subject))
        predicates.add(str(quad.predicate))
        objects.add(str(quad.object))

    return jsonify({
        "total_quads": total_quads,
        "unique_subjects": len(subjects),
        "unique_predicates": len(predicates),
        "unique_objects": len(objects)
    })

@app.errorhandler(404)
def not_found(error):
    return jsonify({"error": "Not found"}), 404

@app.errorhandler(500)
def internal_error(error):
    logger.error(f"Internal server error: {error}")
    return jsonify({"error": "Internal server error"}), 500

if __name__ == '__main__':
    port = int(os.getenv('PORT', 5000))
    debug = os.getenv('DEBUG', 'False').lower() == 'true'

    logger.info(f"Starting Flask server on port {port}")
    app.run(host='0.0.0.0', port=port, debug=debug)
```

### Testing

```bash
# Install dependencies
pip install -r requirements.txt

# Run the server
python app.py

# Test endpoints
curl http://localhost:5000/health

# Add a triple
curl -X POST http://localhost:5000/triples \
  -H "Content-Type: application/json" \
  -d '{
    "subject": {"type": "NamedNode", "value": "http://example.org/alice"},
    "predicate": {"type": "NamedNode", "value": "http://schema.org/name"},
    "object": {"type": "Literal", "value": "Alice"}
  }'

# Execute query
curl -X POST http://localhost:5000/query \
  -H "Content-Type: application/json" \
  -d '{"query": "SELECT * WHERE { ?s ?p ?o } LIMIT 10"}'
```

## FastAPI Async Service

Modern async API with FastAPI, featuring automatic OpenAPI documentation.

### requirements.txt

```
pyoxigraph>=0.3.20
fastapi>=0.104.0
uvicorn[standard]>=0.24.0
pydantic>=2.0.0
python-multipart>=0.0.6
```

### main.py

```python
from fastapi import FastAPI, HTTPException, Query, BackgroundTasks
from fastapi.responses import PlainTextResponse
from pydantic import BaseModel, Field
from pyoxigraph import Store, NamedNode, Literal, Quad
from typing import Optional, List, Dict, Any
import asyncio
from concurrent.futures import ThreadPoolExecutor
import logging

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Initialize FastAPI app
app = FastAPI(
    title="Oxigraph SPARQL API",
    description="FastAPI-based SPARQL endpoint using Pyoxigraph",
    version="1.0.0"
)

# Thread pool for blocking operations
executor = ThreadPoolExecutor(max_workers=4)

# Initialize store
store = Store()

# Pydantic models

class TripleInput(BaseModel):
    subject: str = Field(..., description="Subject IRI")
    predicate: str = Field(..., description="Predicate IRI")
    object_value: str = Field(..., alias="object", description="Object value")
    object_type: str = Field(default="literal", description="Object type: literal, iri, typed_literal, lang_literal")
    datatype: Optional[str] = Field(default=None, description="Datatype IRI for typed literals")
    language: Optional[str] = Field(default=None, description="Language tag for language literals")

class QueryInput(BaseModel):
    query: str = Field(..., description="SPARQL query string")
    base_iri: Optional[str] = Field(default=None, description="Base IRI for query")

class UpdateInput(BaseModel):
    update: str = Field(..., description="SPARQL UPDATE string")

class LoadDataInput(BaseModel):
    content: str = Field(..., description="RDF data content")
    format: str = Field(..., description="RDF format (turtle, ntriples, etc.)")
    base_iri: Optional[str] = Field(default=None, description="Base IRI")

class QueryResponse(BaseModel):
    results: List[Dict[str, Any]]
    count: int

class StatsResponse(BaseModel):
    total_quads: int
    unique_subjects: int
    unique_predicates: int
    unique_objects: int

# Helper functions

async def run_in_executor(func, *args):
    """Run a blocking function in a thread pool."""
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(executor, func, *args)

def term_to_dict(term) -> Dict[str, Any]:
    """Convert RDF term to dictionary."""
    if isinstance(term, NamedNode):
        return {"type": "NamedNode", "value": str(term.value)}
    elif isinstance(term, Literal):
        result = {"type": "Literal", "value": term.value}
        if term.language:
            result["language"] = term.language
        elif term.datatype:
            result["datatype"] = str(term.datatype)
        return result
    else:
        return {"type": "Unknown", "value": str(term)}

# Routes

@app.get("/")
async def root():
    """Root endpoint with API information."""
    return {
        "service": "Oxigraph SPARQL API",
        "version": "1.0.0",
        "endpoints": {
            "health": "/health",
            "query": "/query",
            "update": "/update",
            "triples": "/triples",
            "stats": "/stats",
            "docs": "/docs"
        }
    }

@app.get("/health")
async def health_check():
    """Health check endpoint."""
    size = await run_in_executor(lambda s: len(s), store)
    return {
        "status": "healthy",
        "store_size": size
    }

@app.post("/query", response_model=QueryResponse)
async def execute_query(query_input: QueryInput):
    """Execute a SPARQL SELECT or CONSTRUCT query."""
    logger.info(f"Executing query: {query_input.query[:100]}...")

    def _execute():
        results = store.query(query_input.query)
        output = []

        if hasattr(results, '__iter__'):
            for result in results:
                if isinstance(result, dict):
                    # SELECT results
                    binding = {}
                    for var, term in result.items():
                        binding[str(var)] = term_to_dict(term)
                    output.append(binding)
                elif hasattr(result, 'subject'):
                    # CONSTRUCT results
                    output.append({
                        "subject": term_to_dict(result.subject),
                        "predicate": term_to_dict(result.predicate),
                        "object": term_to_dict(result.object)
                    })
        else:
            # ASK results
            output = [{"result": bool(results)}]

        return output

    try:
        results = await run_in_executor(_execute)
        return QueryResponse(results=results, count=len(results))
    except Exception as e:
        logger.error(f"Query error: {e}")
        raise HTTPException(status_code=400, detail=str(e))

@app.post("/update")
async def execute_update(update_input: UpdateInput):
    """Execute a SPARQL UPDATE query."""
    logger.info(f"Executing update: {update_input.update[:100]}...")

    def _execute():
        store.update(update_input.update)

    try:
        await run_in_executor(_execute)
        return {"message": "Update executed successfully"}
    except Exception as e:
        logger.error(f"Update error: {e}")
        raise HTTPException(status_code=400, detail=str(e))

@app.post("/triples", status_code=201)
async def add_triple(triple: TripleInput):
    """Add a triple to the store."""
    try:
        subject = NamedNode(triple.subject)
        predicate = NamedNode(triple.predicate)

        # Parse object based on type
        if triple.object_type == "iri":
            obj = NamedNode(triple.object_value)
        elif triple.object_type == "lang_literal" and triple.language:
            obj = Literal(triple.object_value, language=triple.language)
        elif triple.object_type == "typed_literal" and triple.datatype:
            obj = Literal(triple.object_value, datatype=NamedNode(triple.datatype))
        else:
            obj = Literal(triple.object_value)

        def _add():
            store.add(Quad(subject, predicate, obj))

        await run_in_executor(_add)

        return {"message": "Triple added successfully"}

    except Exception as e:
        logger.error(f"Error adding triple: {e}")
        raise HTTPException(status_code=400, detail=str(e))

@app.get("/triples")
async def get_triples(
    limit: int = Query(100, ge=1, le=10000),
    offset: int = Query(0, ge=0)
):
    """Get triples with pagination."""
    def _get():
        triples = []
        for i, quad in enumerate(store):
            if i < offset:
                continue
            if len(triples) >= limit:
                break

            triples.append({
                "subject": term_to_dict(quad.subject),
                "predicate": term_to_dict(quad.predicate),
                "object": term_to_dict(quad.object)
            })
        return triples

    triples = await run_in_executor(_get)

    return {
        "triples": triples,
        "count": len(triples),
        "limit": limit,
        "offset": offset
    }

@app.post("/load", status_code=201)
async def load_data(data: LoadDataInput, background_tasks: BackgroundTasks):
    """Load RDF data into the store."""
    format_map = {
        'turtle': 'text/turtle',
        'ttl': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nt': 'application/n-triples',
        'rdfxml': 'application/rdf+xml',
        'nquads': 'application/n-quads',
        'trig': 'application/trig'
    }

    mime_type = format_map.get(data.format.lower(), data.format)

    def _load():
        store.load(
            data.content.encode('utf-8'),
            mime_type=mime_type,
            base_iri=data.base_iri
        )
        logger.info(f"Data loaded successfully. Store size: {len(store)}")

    try:
        # For large datasets, run in background
        if len(data.content) > 100000:
            background_tasks.add_task(_load)
            return {"message": "Loading data in background"}
        else:
            await run_in_executor(_load)
            return {"message": "Data loaded successfully", "store_size": len(store)}

    except Exception as e:
        logger.error(f"Error loading data: {e}")
        raise HTTPException(status_code=400, detail=str(e))

@app.get("/export", response_class=PlainTextResponse)
async def export_data(format: str = Query("turtle", description="Export format")):
    """Export all data from the store."""
    format_map = {
        'turtle': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nquads': 'application/n-quads',
        'trig': 'application/trig'
    }

    mime_type = format_map.get(format.lower(), 'text/turtle')

    def _export():
        return store.dump(mime_type=mime_type)

    try:
        data = await run_in_executor(_export)
        return PlainTextResponse(content=data, media_type=mime_type)
    except Exception as e:
        logger.error(f"Error exporting data: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/stats", response_model=StatsResponse)
async def get_stats():
    """Get store statistics."""
    def _calculate():
        subjects = set()
        predicates = set()
        objects = set()

        for quad in store:
            subjects.add(str(quad.subject))
            predicates.add(str(quad.predicate))
            objects.add(str(quad.object))

        return {
            "total_quads": len(store),
            "unique_subjects": len(subjects),
            "unique_predicates": len(predicates),
            "unique_objects": len(objects)
        }

    stats = await run_in_executor(_calculate)
    return stats

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
```

### Running

```bash
# Install dependencies
pip install -r requirements.txt

# Run with uvicorn
uvicorn main:app --reload

# Access API documentation
open http://localhost:8000/docs
```

## Django Integration

Integrate Oxigraph into a Django project.

### models.py

```python
# myapp/models.py
from django.db import models
from django.conf import settings
from pyoxigraph import Store, NamedNode, Literal, Quad
import os

class RDFStoreManager:
    """Singleton manager for the RDF store."""

    _instance = None
    _store = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            store_path = getattr(settings, 'OXIGRAPH_STORE_PATH', './data/oxigraph')
            os.makedirs(os.path.dirname(store_path), exist_ok=True)
            cls._store = Store(path=store_path)
        return cls._instance

    @property
    def store(self):
        return self._store

# Singleton instance
rdf_store = RDFStoreManager().store
```

### views.py

```python
# myapp/views.py
from django.http import JsonResponse
from django.views.decorators.http import require_http_methods
from django.views.decorators.csrf import csrf_exempt
from .models import rdf_store
from pyoxigraph import NamedNode, Literal, Quad
import json
import logging

logger = logging.getLogger(__name__)

@require_http_methods(["GET"])
def health(request):
    """Health check endpoint."""
    return JsonResponse({
        "status": "healthy",
        "store_size": len(rdf_store)
    })

@csrf_exempt
@require_http_methods(["POST"])
def sparql_query(request):
    """Execute SPARQL query."""
    try:
        data = json.loads(request.body)
        query = data.get('query')

        if not query:
            return JsonResponse({"error": "Missing query parameter"}, status=400)

        results = rdf_store.query(query)
        output = []

        if hasattr(results, '__iter__'):
            for result in results:
                if isinstance(result, dict):
                    binding = {str(k): str(v) for k, v in result.items()}
                    output.append(binding)

        return JsonResponse({"results": output, "count": len(output)})

    except Exception as e:
        logger.error(f"Query error: {e}")
        return JsonResponse({"error": str(e)}, status=400)

@csrf_exempt
@require_http_methods(["POST"])
def add_triple(request):
    """Add a triple to the store."""
    try:
        data = json.loads(request.body)

        subject = NamedNode(data['subject'])
        predicate = NamedNode(data['predicate'])
        obj = Literal(data['object'])

        rdf_store.add(Quad(subject, predicate, obj))

        return JsonResponse({"message": "Triple added"}, status=201)

    except Exception as e:
        logger.error(f"Error adding triple: {e}")
        return JsonResponse({"error": str(e)}, status=400)
```

### settings.py

```python
# Add to settings.py
OXIGRAPH_STORE_PATH = os.path.join(BASE_DIR, 'data', 'oxigraph')
```

### urls.py

```python
# myapp/urls.py
from django.urls import path
from . import views

urlpatterns = [
    path('health/', views.health, name='health'),
    path('query/', views.sparql_query, name='query'),
    path('triples/', views.add_triple, name='add_triple'),
]
```

## Jupyter Notebook Usage

Using Oxigraph in Jupyter notebooks for interactive RDF exploration.

### Installation

```bash
pip install pyoxigraph jupyter pandas matplotlib
```

### Example Notebook

```python
# Cell 1: Setup
from pyoxigraph import Store, NamedNode, Literal, Quad
import pandas as pd
import matplotlib.pyplot as plt

# Create in-memory store
store = Store()

# Cell 2: Load sample data
data = """
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.org/> .

ex:alice schema:name "Alice" ;
         schema:age 30 ;
         schema:city "New York" .

ex:bob schema:name "Bob" ;
       schema:age 25 ;
       schema:city "London" .

ex:charlie schema:name "Charlie" ;
           schema:age 35 ;
           schema:city "New York" .
"""

store.load(data.encode('utf-8'), mime_type="text/turtle")
print(f"Loaded {len(store)} triples")

# Cell 3: Query and display
query = """
PREFIX schema: <http://schema.org/>
SELECT ?name ?age ?city WHERE {
    ?person schema:name ?name ;
            schema:age ?age ;
            schema:city ?city .
}
ORDER BY ?age
"""

results = list(store.query(query))

# Convert to pandas DataFrame
df = pd.DataFrame([
    {
        'name': r['name'].value,
        'age': int(r['age'].value),
        'city': r['city'].value
    }
    for r in results
])

display(df)

# Cell 4: Visualize
df.plot(x='name', y='age', kind='bar', title='Ages by Person')
plt.ylabel('Age')
plt.show()

# Cell 5: Aggregate statistics
city_counts = df['city'].value_counts()
city_counts.plot(kind='pie', title='People by City', autopct='%1.1f%%')
plt.show()

# Cell 6: Complex SPARQL query
query = """
PREFIX schema: <http://schema.org/>
SELECT ?city (AVG(?age) as ?avg_age) (COUNT(?person) as ?count)
WHERE {
    ?person schema:age ?age ;
            schema:city ?city .
}
GROUP BY ?city
ORDER BY DESC(?avg_age)
"""

agg_results = list(store.query(query))
for r in agg_results:
    print(f"City: {r['city'].value}, Avg Age: {float(r['avg_age'].value):.1f}, Count: {r['count'].value}")
```

## Pandas Integration

Convert between RDF and Pandas DataFrames.

```python
from pyoxigraph import Store, NamedNode, Literal, Quad
import pandas as pd
from typing import List, Dict

def sparql_to_dataframe(store: Store, query: str) -> pd.DataFrame:
    """Convert SPARQL SELECT results to pandas DataFrame."""
    results = store.query(query)

    data = []
    for result in results:
        row = {}
        for var, term in result.items():
            # Extract value from term
            if hasattr(term, 'value'):
                row[str(var)] = term.value
            else:
                row[str(var)] = str(term)
        data.append(row)

    return pd.DataFrame(data)

def dataframe_to_rdf(
    df: pd.DataFrame,
    store: Store,
    subject_column: str,
    namespace: str = "http://example.org/"
) -> None:
    """Convert pandas DataFrame to RDF quads."""

    for _, row in df.iterrows():
        subject_value = row[subject_column]
        subject = NamedNode(f"{namespace}{subject_value}")

        for column, value in row.items():
            if column == subject_column:
                continue

            predicate = NamedNode(f"{namespace}{column}")

            # Determine literal type
            if pd.isna(value):
                continue
            elif isinstance(value, (int, float)):
                obj = Literal(str(value), datatype=NamedNode("http://www.w3.org/2001/XMLSchema#decimal"))
            elif isinstance(value, bool):
                obj = Literal(str(value).lower(), datatype=NamedNode("http://www.w3.org/2001/XMLSchema#boolean"))
            else:
                obj = Literal(str(value))

            store.add(Quad(subject, predicate, obj))

# Example usage
if __name__ == "__main__":
    store = Store()

    # Create sample DataFrame
    df = pd.DataFrame({
        'id': ['alice', 'bob', 'charlie'],
        'name': ['Alice', 'Bob', 'Charlie'],
        'age': [30, 25, 35],
        'salary': [75000.0, 60000.0, 85000.0]
    })

    # Convert to RDF
    dataframe_to_rdf(df, store, subject_column='id', namespace="http://example.org/employee/")

    print(f"Store contains {len(store)} triples")

    # Query back to DataFrame
    query = """
    SELECT ?id ?name ?age ?salary WHERE {
        ?person <http://example.org/name> ?name ;
                <http://example.org/age> ?age ;
                <http://example.org/salary> ?salary .
        BIND(REPLACE(STR(?person), "http://example.org/employee/", "") AS ?id)
    }
    """

    result_df = sparql_to_dataframe(store, query)
    print(result_df)
```

## CLI Tools with Click

Create a command-line RDF tool using Click.

### rdf_tool.py

```python
import click
from pyoxigraph import Store, NamedNode, Literal, Quad
from pathlib import Path
import sys

@click.group()
@click.option('--store-path', default='./data/rdf_store', help='Path to RDF store')
@click.pass_context
def cli(ctx, store_path):
    """RDF command-line tool powered by Oxigraph."""
    ctx.ensure_object(dict)
    ctx.obj['store'] = Store(path=store_path)
    ctx.obj['store_path'] = store_path

@cli.command()
@click.pass_context
def info(ctx):
    """Display store information."""
    store = ctx.obj['store']
    click.echo(f"Store path: {ctx.obj['store_path']}")
    click.echo(f"Total quads: {len(store)}")

@cli.command()
@click.argument('file', type=click.Path(exists=True))
@click.option('--format', default='turtle', help='RDF format (turtle, ntriples, etc.)')
@click.pass_context
def load(ctx, file, format):
    """Load RDF data from a file."""
    store = ctx.obj['store']

    format_map = {
        'turtle': 'text/turtle',
        'ntriples': 'application/n-triples',
        'rdfxml': 'application/rdf+xml',
        'nquads': 'application/n-quads',
        'trig': 'application/trig'
    }

    mime_type = format_map.get(format, format)

    with open(file, 'rb') as f:
        data = f.read()
        store.load(data, mime_type=mime_type)

    click.echo(f"Loaded data from {file}")
    click.echo(f"Store now contains {len(store)} quads")

@cli.command()
@click.argument('query')
@click.option('--format', default='table', type=click.Choice(['table', 'json', 'csv']))
@click.pass_context
def query(ctx, query, format):
    """Execute a SPARQL query."""
    store = ctx.obj['store']

    try:
        results = store.query(query)

        if hasattr(results, '__iter__'):
            result_list = list(results)

            if format == 'json':
                import json
                output = []
                for r in result_list:
                    if isinstance(r, dict):
                        output.append({str(k): str(v) for k, v in r.items()})
                click.echo(json.dumps(output, indent=2))

            elif format == 'csv':
                if result_list and isinstance(result_list[0], dict):
                    import csv
                    import sys
                    writer = csv.DictWriter(sys.stdout, fieldnames=[str(k) for k in result_list[0].keys()])
                    writer.writeheader()
                    for r in result_list:
                        writer.writerow({str(k): str(v) for k, v in r.items()})

            else:  # table
                for r in result_list:
                    if isinstance(r, dict):
                        click.echo(" | ".join(f"{k}: {v}" for k, v in r.items()))
                    else:
                        click.echo(r)

        else:
            click.echo(f"Result: {results}")

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)

@cli.command()
@click.argument('subject')
@click.argument('predicate')
@click.argument('object')
@click.option('--object-type', default='literal', type=click.Choice(['literal', 'iri']))
@click.pass_context
def add(ctx, subject, predicate, object, object_type):
    """Add a triple to the store."""
    store = ctx.obj['store']

    try:
        s = NamedNode(subject)
        p = NamedNode(predicate)
        o = NamedNode(object) if object_type == 'iri' else Literal(object)

        store.add(Quad(s, p, o))
        click.echo(f"Added triple: {subject} {predicate} {object}")

    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)

@cli.command()
@click.argument('output', type=click.Path())
@click.option('--format', default='turtle', help='Output format')
@click.pass_context
def export(ctx, output, format):
    """Export store to a file."""
    store = ctx.obj['store']

    format_map = {
        'turtle': 'text/turtle',
        'ntriples': 'application/n-triples',
        'nquads': 'application/n-quads',
        'trig': 'application/trig'
    }

    mime_type = format_map.get(format, 'text/turtle')

    data = store.dump(mime_type=mime_type)

    with open(output, 'wb') as f:
        f.write(data.encode('utf-8'))

    click.echo(f"Exported {len(store)} quads to {output}")

if __name__ == '__main__':
    cli(obj={})
```

### Usage

```bash
# Install
pip install click pyoxigraph

# Use the tool
python rdf_tool.py info
python rdf_tool.py load data.ttl --format turtle
python rdf_tool.py query "SELECT * WHERE { ?s ?p ?o } LIMIT 10"
python rdf_tool.py add http://example.org/alice http://schema.org/name Alice
python rdf_tool.py export output.ttl --format turtle
```

## Celery Background Jobs

Process RDF data asynchronously with Celery.

### requirements.txt

```
pyoxigraph>=0.3.20
celery>=5.3.0
redis>=5.0.0
```

### tasks.py

```python
from celery import Celery
from pyoxigraph import Store, NamedNode, Literal, Quad
import logging

# Configure Celery
app = Celery('oxigraph_tasks', broker='redis://localhost:6379/0')

logger = logging.getLogger(__name__)

# Initialize persistent store
STORE = Store(path='./data/celery_store')

@app.task
def load_rdf_file(file_path: str, format: str = 'turtle'):
    """Load an RDF file in the background."""
    logger.info(f"Loading RDF file: {file_path}")

    format_map = {
        'turtle': 'text/turtle',
        'ntriples': 'application/n-triples',
        'rdfxml': 'application/rdf+xml',
        'nquads': 'application/n-quads'
    }

    mime_type = format_map.get(format, 'text/turtle')

    with open(file_path, 'rb') as f:
        data = f.read()
        STORE.load(data, mime_type=mime_type)

    logger.info(f"Loaded {file_path}. Store size: {len(STORE)}")
    return {"status": "success", "store_size": len(STORE)}

@app.task
def execute_sparql_query(query: str):
    """Execute a SPARQL query in the background."""
    logger.info(f"Executing query: {query[:100]}...")

    results = STORE.query(query)
    output = []

    if hasattr(results, '__iter__'):
        for result in results:
            if isinstance(result, dict):
                output.append({str(k): str(v) for k, v in result.items()})

    logger.info(f"Query returned {len(output)} results")
    return output

@app.task
def bulk_insert_triples(triples):
    """Insert multiple triples."""
    logger.info(f"Inserting {len(triples)} triples")

    for triple_data in triples:
        subject = NamedNode(triple_data['subject'])
        predicate = NamedNode(triple_data['predicate'])
        obj = Literal(triple_data['object'])

        STORE.add(Quad(subject, predicate, obj))

    return {"status": "success", "inserted": len(triples)}

# Usage example
if __name__ == "__main__":
    # Start worker: celery -A tasks worker --loglevel=info

    # Submit tasks
    result = load_rdf_file.delay('/path/to/data.ttl', 'turtle')
    print(f"Task ID: {result.id}")
```

## Production Deployment

### Dockerfile (Flask)

```dockerfile
FROM python:3.11-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY app.py .

ENV FLASK_APP=app.py
ENV PYTHONUNBUFFERED=1

EXPOSE 5000

CMD ["gunicorn", "--bind", "0.0.0.0:5000", "--workers", "4", "app:app"]
```

### docker-compose.yml

```yaml
version: '3.8'

services:
  oxigraph-api:
    build: .
    ports:
      - "5000:5000"
    volumes:
      - ./data:/app/data
    environment:
      - OXIGRAPH_PATH=/app/data/oxigraph
      - LOG_LEVEL=INFO
    restart: unless-stopped
```

### Systemd Service

```ini
# /etc/systemd/system/oxigraph-api.service
[Unit]
Description=Oxigraph SPARQL API
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/opt/oxigraph-api
Environment="PATH=/opt/oxigraph-api/venv/bin"
ExecStart=/opt/oxigraph-api/venv/bin/gunicorn --bind 0.0.0.0:5000 --workers 4 app:app
Restart=always

[Install]
WantedBy=multi-user.target
```

---

These examples provide production-ready patterns for integrating Pyoxigraph into Python applications across various frameworks and use cases!
