# Multi-Tenancy Pattern

**Isolate data for multiple customers in one database using named graphs**

The Multi-Tenancy Pattern enables a single Oxigraph instance to serve multiple customers (tenants) with strong data isolation. Named graphs provide natural boundaries between tenant data while maintaining efficient queries and shared infrastructure.

## When to Use

**Use Multi-Tenancy when:**
- Building SaaS (Software as a Service) applications
- Need to isolate customer data securely
- Want cost-effective infrastructure (shared database)
- Have many customers with similar data models
- Require tenant-specific queries and analytics
- Need to scale horizontally while maintaining isolation

**Skip this pattern when:**
- Single customer/organization only
- Different customers need completely different schemas
- Regulatory requirements mandate physical separation
- Number of tenants is very small (< 5)

## Benefits

✅ **Cost Efficiency** - Share infrastructure across customers
✅ **Strong Isolation** - Named graphs provide clear boundaries
✅ **Easy Onboarding** - Add new tenants without schema changes
✅ **Tenant-Aware Queries** - SPARQL GRAPH clause for filtering
✅ **Compliance** - Audit and isolate data per customer
✅ **Scalability** - Horizontal sharding by tenant ID

## Architecture

### Named Graph Isolation

```
Oxigraph Store
├── <http://app.com/tenant/acme-corp>
│   ├── User triples
│   ├── Order triples
│   └── Product triples
├── <http://app.com/tenant/globex-inc>
│   ├── User triples
│   ├── Order triples
│   └── Product triples
└── <http://app.com/tenant/initech>
    ├── User triples
    ├── Order triples
    └── Product triples
```

### Request Flow

```
HTTP Request
    ↓
Extract Tenant ID (from JWT, header, subdomain)
    ↓
Tenant Context (middleware)
    ↓
Repository (tenant-scoped queries)
    ↓
Oxigraph Store (GRAPH <tenant-graph>)
```

---

## Implementation Examples

### Rust Implementation

#### Tenant Context

```rust
// src/tenancy/tenant_context.rs
use std::sync::Arc;
use oxigraph::model::NamedNode;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(id: String) -> Result<Self, Box<dyn Error>> {
        // Validate tenant ID format
        if id.is_empty() || !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err("Invalid tenant ID format".into());
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_graph_name(&self) -> Result<NamedNode, Box<dyn Error>> {
        NamedNode::new(&format!("http://example.com/tenant/{}", self.0))
            .map_err(|e| e.into())
    }
}

#[derive(Clone)]
pub struct TenantContext {
    tenant_id: TenantId,
    graph: NamedNode,
}

impl TenantContext {
    pub fn new(tenant_id: TenantId) -> Result<Self, Box<dyn Error>> {
        let graph = tenant_id.to_graph_name()?;
        Ok(Self { tenant_id, graph })
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn graph(&self) -> &NamedNode {
        &self.graph
    }
}
```

#### Tenant-Aware Repository

```rust
// src/repositories/multi_tenant_person_repository.rs
use crate::domain::Person;
use crate::repositories::PersonRepository;
use crate::tenancy::TenantContext;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::error::Error;
use std::sync::Arc;

pub struct MultiTenantPersonRepository {
    store: Arc<Store>,
    tenant_context: TenantContext,
}

impl MultiTenantPersonRepository {
    pub fn new(store: Arc<Store>, tenant_context: TenantContext) -> Self {
        Self {
            store,
            tenant_context,
        }
    }

    fn person_to_quads(&self, person: &Person) -> Result<Vec<Quad>, Box<dyn Error>> {
        let subject = NamedNode::new(&format!("http://example.com/person/{}", person.id))?;
        // IMPORTANT: Use tenant graph for isolation
        let graph_name = GraphName::NamedNode(self.tenant_context.graph().clone());

        let mut quads = vec![
            Quad::new(
                subject.clone(),
                NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
                NamedNode::new("http://example.com/Person")?,
                graph_name.clone(),
            ),
            Quad::new(
                subject.clone(),
                NamedNode::new("http://example.com/name")?,
                Literal::new_simple_literal(&person.name),
                graph_name.clone(),
            ),
            Quad::new(
                subject.clone(),
                NamedNode::new("http://example.com/email")?,
                Literal::new_simple_literal(&person.email),
                graph_name.clone(),
            ),
        ];

        if let Some(age) = person.age {
            quads.push(Quad::new(
                subject,
                NamedNode::new("http://example.com/age")?,
                Literal::new_typed_literal(
                    &age.to_string(),
                    NamedNode::new("http://www.w3.org/2001/XMLSchema#integer")?,
                ),
                graph_name,
            ));
        }

        Ok(quads)
    }

    fn parse_person(&self, bindings: &QuerySolution) -> Result<Person, Box<dyn Error>> {
        let id = bindings
            .get("id")?
            .ok_or("Missing id")?
            .as_str()
            .split('/')
            .last()
            .ok_or("Invalid ID format")?
            .to_string();

        let name = bindings
            .get("name")?
            .ok_or("Missing name")?
            .as_str()
            .to_string();

        let email = bindings
            .get("email")?
            .ok_or("Missing email")?
            .as_str()
            .to_string();

        let age = bindings
            .get("age")
            .and_then(|term| term.as_str().parse::<u32>().ok());

        Ok(Person {
            id,
            name,
            email,
            age,
        })
    }
}

impl PersonRepository for MultiTenantPersonRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Person>, Box<dyn Error>> {
        // IMPORTANT: Query scoped to tenant's graph
        let query = format!(
            r#"
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{}> {{
                    BIND(<http://example.com/person/{}> AS ?id)
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
            "#,
            self.tenant_context.graph().as_str(),
            id
        );

        if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&self.store)
            .execute()?
        {
            if let Some(solution) = solutions.next() {
                return Ok(Some(self.parse_person(&solution?)?));
            }
        }

        Ok(None)
    }

    fn find_all(&self) -> Result<Vec<Person>, Box<dyn Error>> {
        let query = format!(
            r#"
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{}> {{
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
            ORDER BY ?name
            "#,
            self.tenant_context.graph().as_str()
        );

        let mut persons = Vec::new();

        if let QueryResults::Solutions(solutions) = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&self.store)
            .execute()?
        {
            for solution in solutions {
                persons.push(self.parse_person(&solution?)?);
            }
        }

        Ok(persons)
    }

    fn save(&self, person: &Person) -> Result<(), Box<dyn Error>> {
        // Delete existing data first (within tenant graph only)
        self.delete(&person.id)?;

        // Insert new data (scoped to tenant graph)
        let quads = self.person_to_quads(person)?;
        for quad in quads {
            self.store.insert(&quad)?;
        }

        Ok(())
    }

    fn delete(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        let subject = NamedNode::new(&format!("http://example.com/person/{}", id))?;
        let graph_name = GraphName::NamedNode(self.tenant_context.graph().clone());

        // Only delete from this tenant's graph
        let quads: Vec<_> = self
            .store
            .quads_for_pattern(
                Some(subject.as_ref()),
                None,
                None,
                Some(graph_name.as_ref()),
            )
            .collect::<Result<_, _>>()?;

        let found = !quads.is_empty();

        for quad in quads {
            self.store.remove(&quad)?;
        }

        Ok(found)
    }

    fn count(&self) -> Result<usize, Box<dyn Error>> {
        let query = format!(
            r#"
            PREFIX ex: <http://example.com/>
            SELECT (COUNT(?id) AS ?count)
            WHERE {{
                GRAPH <{}> {{
                    ?id a ex:Person .
                }}
            }}
            "#,
            self.tenant_context.graph().as_str()
        );

        if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&self.store)
            .execute()?
        {
            if let Some(solution) = solutions.next() {
                let binding = solution?;
                if let Some(count_term) = binding.get("count") {
                    return Ok(count_term.as_str().parse()?);
                }
            }
        }

        Ok(0)
    }

    // Additional tenant-specific methods
    fn find_by_email(&self, email: &str) -> Result<Option<Person>, Box<dyn Error>> {
        let query = format!(
            r#"
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{}> {{
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email "{}" .
                    BIND("{}" AS ?email)
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
            "#,
            self.tenant_context.graph().as_str(),
            email,
            email
        );

        if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&self.store)
            .execute()?
        {
            if let Some(solution) = solutions.next() {
                return Ok(Some(self.parse_person(&solution?)?));
            }
        }

        Ok(None)
    }
}
```

#### Tenant Middleware (Actix-web example)

```rust
// src/web/tenant_middleware.rs
use crate::tenancy::{TenantContext, TenantId};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};

pub struct TenantMiddleware;

impl<S, B> Transform<S, ServiceRequest> for TenantMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TenantMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TenantMiddlewareService { service }))
    }
}

pub struct TenantMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for TenantMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Extract tenant ID from header, JWT, or subdomain
        let tenant_id = extract_tenant_id(&req);

        match tenant_id {
            Ok(tenant_id) => {
                // Create tenant context
                match TenantContext::new(tenant_id) {
                    Ok(context) => {
                        req.extensions_mut().insert(context);
                        Box::pin(self.service.call(req))
                    }
                    Err(_) => Box::pin(async {
                        Err(actix_web::error::ErrorBadRequest("Invalid tenant ID"))
                    }),
                }
            }
            Err(e) => Box::pin(async move {
                Err(actix_web::error::ErrorUnauthorized(e))
            }),
        }
    }
}

fn extract_tenant_id(req: &ServiceRequest) -> Result<TenantId, String> {
    // Option 1: From header
    if let Some(header) = req.headers().get("X-Tenant-ID") {
        if let Ok(tenant_str) = header.to_str() {
            return TenantId::new(tenant_str.to_string())
                .map_err(|e| e.to_string());
        }
    }

    // Option 2: From JWT claims
    // if let Some(claims) = req.extensions().get::<JwtClaims>() {
    //     return TenantId::new(claims.tenant_id.clone());
    // }

    // Option 3: From subdomain
    if let Some(host) = req.headers().get("Host") {
        if let Ok(host_str) = host.to_str() {
            if let Some(tenant_str) = host_str.split('.').next() {
                if tenant_str != "www" && tenant_str != "api" {
                    return TenantId::new(tenant_str.to_string())
                        .map_err(|e| e.to_string());
                }
            }
        }
    }

    Err("No tenant ID found".to_string())
}
```

#### Usage in Handler

```rust
// src/web/handlers.rs
use actix_web::{web, HttpRequest, HttpResponse, Result};
use crate::repositories::MultiTenantPersonRepository;
use crate::tenancy::TenantContext;

pub async fn get_persons(
    req: HttpRequest,
    store: web::Data<Arc<Store>>,
) -> Result<HttpResponse> {
    // Extract tenant context from request extensions
    let tenant_context = req
        .extensions()
        .get::<TenantContext>()
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("No tenant context"))?
        .clone();

    // Create tenant-scoped repository
    let repo = MultiTenantPersonRepository::new(store.get_ref().clone(), tenant_context);

    // All queries automatically scoped to tenant
    let persons = repo.find_all()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    Ok(HttpResponse::Ok().json(persons))
}
```

---

### Python Implementation

#### Tenant Context

```python
# tenancy/tenant_context.py
import re
from pyoxigraph import NamedNode

class TenantId:
    def __init__(self, id: str):
        if not id or not re.match(r'^[a-zA-Z0-9_-]+$', id):
            raise ValueError("Invalid tenant ID format")
        self.id = id

    def to_graph_name(self) -> NamedNode:
        return NamedNode(f"http://example.com/tenant/{self.id}")

    def __str__(self) -> str:
        return self.id

    def __eq__(self, other) -> bool:
        return isinstance(other, TenantId) and self.id == other.id

    def __hash__(self) -> int:
        return hash(self.id)


class TenantContext:
    def __init__(self, tenant_id: TenantId):
        self.tenant_id = tenant_id
        self.graph = tenant_id.to_graph_name()

    def __eq__(self, other) -> bool:
        return isinstance(other, TenantContext) and self.tenant_id == other.tenant_id
```

#### Tenant-Aware Repository

```python
# repositories/multi_tenant_person_repository.py
from typing import List, Optional
from pyoxigraph import Store, NamedNode, Literal, Quad
from domain.person import Person
from repositories.person_repository import PersonRepository
from tenancy.tenant_context import TenantContext

class MultiTenantPersonRepository(PersonRepository):
    def __init__(self, store: Store, tenant_context: TenantContext):
        self.store = store
        self.tenant_context = tenant_context

    def _person_to_quads(self, person: Person) -> List[Quad]:
        """Convert person to quads scoped to tenant graph."""
        subject = NamedNode(f"http://example.com/person/{person.id}")

        quads = [
            Quad(
                subject,
                NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                NamedNode("http://example.com/Person"),
                self.tenant_context.graph,  # Tenant isolation
            ),
            Quad(
                subject,
                NamedNode("http://example.com/name"),
                Literal(person.name),
                self.tenant_context.graph,
            ),
            Quad(
                subject,
                NamedNode("http://example.com/email"),
                Literal(person.email),
                self.tenant_context.graph,
            ),
        ]

        if person.age is not None:
            quads.append(Quad(
                subject,
                NamedNode("http://example.com/age"),
                Literal(
                    str(person.age),
                    datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer")
                ),
                self.tenant_context.graph,
            ))

        return quads

    def _parse_person(self, bindings: dict) -> Person:
        person_id = bindings['id'].value.split('/')[-1]
        name = bindings['name'].value
        email = bindings['email'].value
        age = int(bindings['age'].value) if 'age' in bindings else None

        return Person(id=person_id, name=name, email=email, age=age)

    def find_by_id(self, id: str) -> Optional[Person]:
        """Find person by ID within tenant's graph."""
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{self.tenant_context.graph.value}> {{
                    BIND(<http://example.com/person/{id}> AS ?id)
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
        '''

        results = self.store.query(query)
        for bindings in results:
            return self._parse_person(bindings)

        return None

    def find_all(self) -> List[Person]:
        """Find all persons within tenant's graph."""
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{self.tenant_context.graph.value}> {{
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
            ORDER BY ?name
        '''

        persons = []
        results = self.store.query(query)
        for bindings in results:
            persons.append(self._parse_person(bindings))

        return persons

    def save(self, person: Person) -> None:
        """Save person to tenant's graph."""
        # Delete existing data
        self.delete(person.id)

        # Insert new data in tenant's graph
        for quad in self._person_to_quads(person):
            self.store.add(quad)

    def delete(self, id: str) -> bool:
        """Delete person from tenant's graph."""
        subject = NamedNode(f"http://example.com/person/{id}")

        # Only delete from this tenant's graph
        quads = list(self.store.quads_for_pattern(
            subject, None, None, self.tenant_context.graph
        ))
        found = len(quads) > 0

        for quad in quads:
            self.store.remove(quad)

        return found

    def count(self) -> int:
        """Count persons in tenant's graph."""
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT (COUNT(?id) AS ?count)
            WHERE {{
                GRAPH <{self.tenant_context.graph.value}> {{
                    ?id a ex:Person .
                }}
            }}
        '''

        results = self.store.query(query)
        for bindings in results:
            return int(bindings['count'].value)

        return 0

    def find_by_email(self, email: str) -> Optional[Person]:
        """Find person by email within tenant's graph."""
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{self.tenant_context.graph.value}> {{
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email "{email}" .
                    BIND("{email}" AS ?email)
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
        '''

        results = self.store.query(query)
        for bindings in results:
            return self._parse_person(bindings)

        return None
```

#### Flask Middleware

```python
# web/tenant_middleware.py
from flask import Flask, request, g
from functools import wraps
from tenancy.tenant_context import TenantId, TenantContext

def extract_tenant_id() -> TenantId:
    """Extract tenant ID from request headers or subdomain."""
    # Option 1: From header
    tenant_header = request.headers.get('X-Tenant-ID')
    if tenant_header:
        return TenantId(tenant_header)

    # Option 2: From subdomain
    host = request.headers.get('Host', '')
    subdomain = host.split('.')[0]
    if subdomain and subdomain not in ['www', 'api']:
        return TenantId(subdomain)

    # Option 3: From JWT (if using authentication)
    # if hasattr(g, 'jwt_claims'):
    #     return TenantId(g.jwt_claims['tenant_id'])

    raise ValueError("No tenant ID found")


def require_tenant_context(f):
    """Decorator to extract and validate tenant context."""
    @wraps(f)
    def decorated_function(*args, **kwargs):
        try:
            tenant_id = extract_tenant_id()
            g.tenant_context = TenantContext(tenant_id)
            return f(*args, **kwargs)
        except ValueError as e:
            return {'error': str(e)}, 401

    return decorated_function


def init_tenant_middleware(app: Flask):
    """Initialize tenant middleware for Flask app."""
    @app.before_request
    def load_tenant_context():
        try:
            tenant_id = extract_tenant_id()
            g.tenant_context = TenantContext(tenant_id)
        except ValueError:
            # Don't fail here - let route handlers decide if tenant is required
            g.tenant_context = None
```

#### Flask Route Example

```python
# web/routes.py
from flask import Blueprint, g, jsonify
from repositories.multi_tenant_person_repository import MultiTenantPersonRepository
from web.tenant_middleware import require_tenant_context

persons_bp = Blueprint('persons', __name__)

@persons_bp.route('/persons', methods=['GET'])
@require_tenant_context
def get_persons():
    """Get all persons for current tenant."""
    # Repository automatically scoped to tenant
    repo = MultiTenantPersonRepository(app.config['STORE'], g.tenant_context)

    persons = repo.find_all()
    return jsonify([p.__dict__ for p in persons])

@persons_bp.route('/persons/<person_id>', methods=['GET'])
@require_tenant_context
def get_person(person_id):
    """Get person by ID within tenant's data."""
    repo = MultiTenantPersonRepository(app.config['STORE'], g.tenant_context)

    person = repo.find_by_id(person_id)
    if person:
        return jsonify(person.__dict__)
    else:
        return {'error': 'Person not found'}, 404
```

---

### JavaScript Implementation

#### Tenant Context

```javascript
// tenancy/TenantContext.js
import { NamedNode } from 'oxigraph';

export class TenantId {
    constructor(id) {
        if (!id || !/^[a-zA-Z0-9_-]+$/.test(id)) {
            throw new Error('Invalid tenant ID format');
        }
        this.id = id;
    }

    toGraphName() {
        return new NamedNode(`http://example.com/tenant/${this.id}`);
    }

    toString() {
        return this.id;
    }
}

export class TenantContext {
    constructor(tenantId) {
        this.tenantId = tenantId;
        this.graph = tenantId.toGraphName();
    }
}
```

#### Tenant-Aware Repository

```javascript
// repositories/MultiTenantPersonRepository.js
import { NamedNode, Literal, Quad } from 'oxigraph';
import { Person } from '../domain/Person.js';
import { PersonRepository } from './PersonRepository.js';

export class MultiTenantPersonRepository extends PersonRepository {
    constructor(store, tenantContext) {
        super();
        this.store = store;
        this.tenantContext = tenantContext;
    }

    personToQuads(person) {
        const subject = new NamedNode(`http://example.com/person/${person.id}`);

        const quads = [
            new Quad(
                subject,
                new NamedNode('http://www.w3.org/1999/02/22-rdf-syntax-ns#type'),
                new NamedNode('http://example.com/Person'),
                this.tenantContext.graph  // Tenant isolation
            ),
            new Quad(
                subject,
                new NamedNode('http://example.com/name'),
                new Literal(person.name),
                this.tenantContext.graph
            ),
            new Quad(
                subject,
                new NamedNode('http://example.com/email'),
                new Literal(person.email),
                this.tenantContext.graph
            ),
        ];

        if (person.age !== null) {
            quads.push(new Quad(
                subject,
                new NamedNode('http://example.com/age'),
                Literal.integer(person.age),
                this.tenantContext.graph
            ));
        }

        return quads;
    }

    parsePerson(bindings) {
        const id = bindings.get('id').value.split('/').pop();
        const name = bindings.get('name').value;
        const email = bindings.get('email').value;
        const age = bindings.has('age') ? parseInt(bindings.get('age').value, 10) : null;

        return new Person(id, name, email, age);
    }

    async findById(id) {
        const query = `
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {
                GRAPH <${this.tenantContext.graph.value}> {
                    BIND(<http://example.com/person/${id}> AS ?id)
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL { ?id ex:age ?age }
                }
            }
        `;

        const results = this.store.query(query);

        for (const bindings of results) {
            return this.parsePerson(bindings);
        }

        return null;
    }

    async findAll() {
        const query = `
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {
                GRAPH <${this.tenantContext.graph.value}> {
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL { ?id ex:age ?age }
                }
            }
            ORDER BY ?name
        `;

        const persons = [];
        const results = this.store.query(query);

        for (const bindings of results) {
            persons.push(this.parsePerson(bindings));
        }

        return persons;
    }

    async save(person) {
        await this.delete(person.id);

        for (const quad of this.personToQuads(person)) {
            this.store.add(quad);
        }
    }

    async delete(id) {
        const subject = new NamedNode(`http://example.com/person/${id}`);

        const quads = this.store.match(subject, null, null, this.tenantContext.graph);
        const found = quads.length > 0;

        for (const quad of quads) {
            this.store.delete(quad);
        }

        return found;
    }

    async count() {
        const query = `
            PREFIX ex: <http://example.com/>
            SELECT (COUNT(?id) AS ?count)
            WHERE {
                GRAPH <${this.tenantContext.graph.value}> {
                    ?id a ex:Person .
                }
            }
        `;

        const results = this.store.query(query);

        for (const bindings of results) {
            return parseInt(bindings.get('count').value, 10);
        }

        return 0;
    }

    async findByEmail(email) {
        const query = `
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {
                GRAPH <${this.tenantContext.graph.value}> {
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email "${email}" .
                    BIND("${email}" AS ?email)
                    OPTIONAL { ?id ex:age ?age }
                }
            }
        `;

        const results = this.store.query(query);

        for (const bindings of results) {
            return this.parsePerson(bindings);
        }

        return null;
    }
}
```

#### Express Middleware

```javascript
// web/tenantMiddleware.js
import { TenantId, TenantContext } from '../tenancy/TenantContext.js';

function extractTenantId(req) {
    // Option 1: From header
    const tenantHeader = req.headers['x-tenant-id'];
    if (tenantHeader) {
        return new TenantId(tenantHeader);
    }

    // Option 2: From subdomain
    const host = req.headers.host || '';
    const subdomain = host.split('.')[0];
    if (subdomain && !['www', 'api'].includes(subdomain)) {
        return new TenantId(subdomain);
    }

    // Option 3: From JWT
    // if (req.user && req.user.tenantId) {
    //     return new TenantId(req.user.tenantId);
    // }

    throw new Error('No tenant ID found');
}

export function tenantMiddleware(req, res, next) {
    try {
        const tenantId = extractTenantId(req);
        req.tenantContext = new TenantContext(tenantId);
        next();
    } catch (error) {
        res.status(401).json({ error: error.message });
    }
}
```

#### Express Route Example

```javascript
// web/routes.js
import express from 'express';
import { MultiTenantPersonRepository } from '../repositories/MultiTenantPersonRepository.js';
import { tenantMiddleware } from './tenantMiddleware.js';

const router = express.Router();

// Apply tenant middleware to all routes
router.use(tenantMiddleware);

router.get('/persons', async (req, res) => {
    try {
        const repo = new MultiTenantPersonRepository(
            req.app.get('store'),
            req.tenantContext
        );

        const persons = await repo.findAll();
        res.json(persons.map(p => p.toJSON()));
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

router.get('/persons/:id', async (req, res) => {
    try {
        const repo = new MultiTenantPersonRepository(
            req.app.get('store'),
            req.tenantContext
        );

        const person = await repo.findById(req.params.id);

        if (person) {
            res.json(person.toJSON());
        } else {
            res.status(404).json({ error: 'Person not found' });
        }
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

export default router;
```

---

## Cross-Tenant Queries (Admin)

For admin dashboards that need to query across tenants:

```sparql
PREFIX ex: <http://example.com/>

SELECT ?tenant ?userName ?email
WHERE {
    GRAPH ?tenant {
        ?user a ex:Person ;
              ex:name ?userName ;
              ex:email ?email .
    }
    FILTER (STRSTARTS(STR(?tenant), "http://example.com/tenant/"))
}
ORDER BY ?tenant ?userName
```

---

## Security Considerations

### 1. Always Validate Tenant ID

```rust
// BAD - SQL injection equivalent
let tenant_id = user_input; // Dangerous!

// GOOD - Validate format
let tenant_id = TenantId::new(user_input)?; // Validates format
```

### 2. Never Trust Client-Provided Tenant ID

```javascript
// BAD - User can access other tenants
const tenantId = req.query.tenant; // Dangerous!

// GOOD - Extract from authenticated session
const tenantId = req.user.tenantId; // From JWT or session
```

### 3. Prevent Cross-Tenant Data Leaks

```rust
// Ensure ALL queries include GRAPH clause
let query = format!(
    "SELECT * WHERE {{ GRAPH <{}> {{ ... }} }}",
    tenant_context.graph()
);
```

### 4. Audit Cross-Tenant Access

```rust
if is_cross_tenant_query(&query) {
    audit_log(&format!(
        "Admin {} accessed tenant {}",
        user_id, tenant_id
    ));
}
```

---

## Performance Optimization

### 1. Index Tenant Graphs

Oxigraph automatically indexes named graphs, but ensure your queries use them:

```sparql
# Efficient - uses graph index
SELECT * WHERE { GRAPH <tenant-123> { ?s ?p ?o } }

# Inefficient - scans all graphs
SELECT * WHERE { ?s ?p ?o . FILTER(...) }
```

### 2. Tenant-Aware Caching

Cache per tenant to avoid cache pollution:

```rust
let cache_key = format!("{}:{}", tenant_id, query_key);
```

### 3. Batch Operations

When loading data for multiple tenants, use transactions:

```rust
store.transaction(|tx| {
    for tenant in tenants {
        for quad in tenant.quads {
            tx.insert(&quad)?;
        }
    }
    Ok(())
})?;
```

---

## Tenant Lifecycle Management

### Onboarding New Tenant

```rust
pub fn onboard_tenant(store: &Store, tenant_id: &TenantId) -> Result<(), Box<dyn Error>> {
    let context = TenantContext::new(tenant_id.clone())?;

    // Create tenant metadata
    let tenant_iri = NamedNode::new(&format!("http://example.com/tenant/{}", tenant_id.as_str()))?;
    let metadata_graph = NamedNode::new("http://example.com/graph/metadata")?;

    store.insert(&Quad::new(
        tenant_iri.clone(),
        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
        NamedNode::new("http://example.com/Tenant")?,
        GraphName::NamedNode(metadata_graph.clone()),
    ))?;

    store.insert(&Quad::new(
        tenant_iri,
        NamedNode::new("http://example.com/createdAt")?,
        Literal::new_typed_literal(
            &chrono::Utc::now().to_rfc3339(),
            NamedNode::new("http://www.w3.org/2001/XMLSchema#dateTime")?,
        ),
        GraphName::NamedNode(metadata_graph),
    ))?;

    Ok(())
}
```

### Deleting Tenant Data

```rust
pub fn delete_tenant(store: &Store, tenant_id: &TenantId) -> Result<(), Box<dyn Error>> {
    let context = TenantContext::new(tenant_id.clone())?;
    let graph_name = GraphName::NamedNode(context.graph().clone());

    // Delete all quads in tenant's graph
    let quads: Vec<_> = store
        .quads_for_pattern(None, None, None, Some(graph_name.as_ref()))
        .collect::<Result<_, _>>()?;

    for quad in quads {
        store.remove(&quad)?;
    }

    Ok(())
}
```

---

## Best Practices

### ✅ DO:

**Use Named Graphs** - One graph per tenant for isolation
**Validate Tenant ID** - Always validate format and permissions
**Extract from Auth** - Get tenant from JWT/session, not query params
**GRAPH Clause** - Every query must scope to tenant graph
**Audit Access** - Log all cross-tenant access
**Test Isolation** - Verify tenants can't access each other's data

### ❌ DON'T:

**Trust Client Input** - Never use user-provided tenant ID directly
**Skip GRAPH Clause** - Always explicitly scope queries
**Share IDs Across Tenants** - Entity IDs should be unique per tenant
**Forget Cleanup** - Remove tenant data completely on deletion

---

## Testing Multi-Tenancy

```rust
#[test]
fn test_tenant_isolation() {
    let store = Arc::new(Store::new().unwrap());

    // Create two tenants
    let tenant1 = TenantContext::new(TenantId::new("tenant1".to_string()).unwrap()).unwrap();
    let tenant2 = TenantContext::new(TenantId::new("tenant2".to_string()).unwrap()).unwrap();

    // Create repositories for each tenant
    let repo1 = MultiTenantPersonRepository::new(store.clone(), tenant1);
    let repo2 = MultiTenantPersonRepository::new(store.clone(), tenant2);

    // Add person to tenant1
    let person1 = Person::new("1".to_string(), "Alice".to_string(), "alice@tenant1.com".to_string());
    repo1.save(&person1).unwrap();

    // Verify tenant1 can see it
    assert_eq!(repo1.find_by_id("1").unwrap(), Some(person1));

    // Verify tenant2 CANNOT see it
    assert_eq!(repo2.find_by_id("1").unwrap(), None);

    // Verify counts are isolated
    assert_eq!(repo1.count().unwrap(), 1);
    assert_eq!(repo2.count().unwrap(), 0);
}
```

---

## Next Steps

- Combine with [Repository Pattern](./repository-pattern.md) for clean architecture
- Add [Caching](./caching.md) with tenant-aware keys
- Use [Event Sourcing](./event-sourcing.md) with tenant-scoped events

---

## Summary

The Multi-Tenancy Pattern provides:
- **Strong data isolation** using named graphs
- **Cost-efficient** shared infrastructure
- **Tenant-aware queries** with SPARQL GRAPH clause
- **Easy onboarding** without schema changes
- **Compliance** with audit trails per tenant

Start with a simple tenant context middleware, then expand with per-tenant caching and analytics.
