# Repository Pattern

**Abstract data access behind a clean, testable interface**

The Repository Pattern creates a layer between your business logic and data storage, abstracting Oxigraph Store operations behind a domain-focused interface. This makes code easier to test, maintain, and evolve.

## When to Use

**Use the Repository Pattern when:**
- Building applications with complex business logic
- Need to write unit tests without a real database
- Want to hide SPARQL complexity from domain code
- Planning to support multiple storage backends
- Working in a team with separation of concerns
- Following Domain-Driven Design principles

**Skip this pattern when:**
- Building simple scripts or one-off data migrations
- SPARQL queries are the primary business logic
- Application is purely a SPARQL endpoint proxy
- Team is small and fully comfortable with SPARQL

## Benefits

✅ **Testability** - Mock repository for unit tests without database
✅ **Maintainability** - Domain logic separate from data access
✅ **Flexibility** - Swap storage implementations easily
✅ **Clarity** - Business-focused method names, not raw SPARQL
✅ **Reusability** - Share repository across multiple services

## Architecture

```
┌─────────────────────────────────────────┐
│  Application / Business Logic Layer     │
│  (Controllers, Services, Use Cases)     │
└───────────────┬─────────────────────────┘
                │ Uses domain objects
                ↓
┌─────────────────────────────────────────┐
│  Repository Interface                   │
│  (find, save, delete, query)            │
└───────────────┬─────────────────────────┘
                │ Implemented by
                ↓
┌─────────────────────────────────────────┐
│  Oxigraph Repository Implementation     │
│  (SPARQL queries, Store operations)     │
└───────────────┬─────────────────────────┘
                │ Uses
                ↓
┌─────────────────────────────────────────┐
│  Oxigraph Store                          │
│  (RDF triples, SPARQL engine)           │
└─────────────────────────────────────────┘
```

---

## Implementation Examples

### Rust Implementation

#### Domain Model

```rust
// src/domain/person.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub email: String,
    pub age: Option<u32>,
}

impl Person {
    pub fn new(id: String, name: String, email: String) -> Self {
        Self {
            id,
            name,
            email,
            age: None,
        }
    }

    pub fn with_age(mut self, age: u32) -> Self {
        self.age = Some(age);
        self
    }
}
```

#### Repository Trait

```rust
// src/repositories/person_repository.rs
use crate::domain::Person;
use std::error::Error;

pub trait PersonRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Person>, Box<dyn Error>>;
    fn find_by_email(&self, email: &str) -> Result<Option<Person>, Box<dyn Error>>;
    fn find_all(&self) -> Result<Vec<Person>, Box<dyn Error>>;
    fn save(&self, person: &Person) -> Result<(), Box<dyn Error>>;
    fn delete(&self, id: &str) -> Result<bool, Box<dyn Error>>;
    fn count(&self) -> Result<usize, Box<dyn Error>>;
}
```

#### Oxigraph Implementation

```rust
// src/repositories/oxigraph_person_repository.rs
use crate::domain::Person;
use crate::repositories::PersonRepository;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::error::Error;
use std::sync::Arc;

pub struct OxigraphPersonRepository {
    store: Arc<Store>,
    graph: NamedNode,
}

impl OxigraphPersonRepository {
    pub fn new(store: Arc<Store>) -> Result<Self, Box<dyn Error>> {
        let graph = NamedNode::new("http://example.com/graph/persons")?;
        Ok(Self { store, graph })
    }

    fn person_to_quads(&self, person: &Person) -> Result<Vec<Quad>, Box<dyn Error>> {
        let subject = NamedNode::new(&format!("http://example.com/person/{}", person.id))?;
        let graph_name = GraphName::NamedNode(self.graph.clone());

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

impl PersonRepository for OxigraphPersonRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Person>, Box<dyn Error>> {
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
            self.graph.as_str(),
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
                    BIND(?id AS ?id)
                    BIND("{}" AS ?email)
                    OPTIONAL {{ ?id ex:age ?age }}
                }}
            }}
            "#,
            self.graph.as_str(),
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
            self.graph.as_str()
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
        // Delete existing data first
        self.delete(&person.id)?;

        // Insert new data
        let quads = self.person_to_quads(person)?;
        for quad in quads {
            self.store.insert(&quad)?;
        }

        Ok(())
    }

    fn delete(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        let subject = NamedNode::new(&format!("http://example.com/person/{}", id))?;
        let graph_name = GraphName::NamedNode(self.graph.clone());

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
            self.graph.as_str()
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
}
```

#### Mock Repository for Testing

```rust
// src/repositories/mock_person_repository.rs
use crate::domain::Person;
use crate::repositories::PersonRepository;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct MockPersonRepository {
    data: Arc<RwLock<HashMap<String, Person>>>,
}

impl MockPersonRepository {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_data(persons: Vec<Person>) -> Self {
        let data = persons
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        Self {
            data: Arc::new(RwLock::new(data)),
        }
    }
}

impl PersonRepository for MockPersonRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Person>, Box<dyn Error>> {
        Ok(self.data.read().unwrap().get(id).cloned())
    }

    fn find_by_email(&self, email: &str) -> Result<Option<Person>, Box<dyn Error>> {
        Ok(self
            .data
            .read()
            .unwrap()
            .values()
            .find(|p| p.email == email)
            .cloned())
    }

    fn find_all(&self) -> Result<Vec<Person>, Box<dyn Error>> {
        let mut persons: Vec<_> = self.data.read().unwrap().values().cloned().collect();
        persons.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(persons)
    }

    fn save(&self, person: &Person) -> Result<(), Box<dyn Error>> {
        self.data
            .write()
            .unwrap()
            .insert(person.id.clone(), person.clone());
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        Ok(self.data.write().unwrap().remove(id).is_some())
    }

    fn count(&self) -> Result<usize, Box<dyn Error>> {
        Ok(self.data.read().unwrap().len())
    }
}
```

#### Unit Tests

```rust
// tests/repositories/person_repository_test.rs
use myapp::domain::Person;
use myapp::repositories::{MockPersonRepository, PersonRepository};

#[test]
fn test_find_by_id() {
    let person = Person::new("1".to_string(), "Alice".to_string(), "alice@example.com".to_string());
    let repo = MockPersonRepository::with_data(vec![person.clone()]);

    let found = repo.find_by_id("1").unwrap();
    assert_eq!(found, Some(person));

    let not_found = repo.find_by_id("999").unwrap();
    assert_eq!(not_found, None);
}

#[test]
fn test_save_and_count() {
    let repo = MockPersonRepository::new();
    assert_eq!(repo.count().unwrap(), 0);

    let person = Person::new("1".to_string(), "Bob".to_string(), "bob@example.com".to_string());
    repo.save(&person).unwrap();

    assert_eq!(repo.count().unwrap(), 1);
    assert_eq!(repo.find_by_id("1").unwrap(), Some(person));
}

#[test]
fn test_delete() {
    let person = Person::new("1".to_string(), "Charlie".to_string(), "charlie@example.com".to_string());
    let repo = MockPersonRepository::with_data(vec![person]);

    assert!(repo.delete("1").unwrap());
    assert_eq!(repo.count().unwrap(), 0);
    assert!(!repo.delete("1").unwrap());
}
```

---

### Python Implementation

#### Domain Model

```python
# domain/person.py
from dataclasses import dataclass
from typing import Optional

@dataclass
class Person:
    id: str
    name: str
    email: str
    age: Optional[int] = None

    def with_age(self, age: int) -> 'Person':
        self.age = age
        return self
```

#### Repository Interface

```python
# repositories/person_repository.py
from abc import ABC, abstractmethod
from typing import List, Optional
from domain.person import Person

class PersonRepository(ABC):
    @abstractmethod
    def find_by_id(self, id: str) -> Optional[Person]:
        pass

    @abstractmethod
    def find_by_email(self, email: str) -> Optional[Person]:
        pass

    @abstractmethod
    def find_all(self) -> List[Person]:
        pass

    @abstractmethod
    def save(self, person: Person) -> None:
        pass

    @abstractmethod
    def delete(self, id: str) -> bool:
        pass

    @abstractmethod
    def count(self) -> int:
        pass
```

#### Oxigraph Implementation

```python
# repositories/oxigraph_person_repository.py
from typing import List, Optional
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph
from domain.person import Person
from repositories.person_repository import PersonRepository

class OxigraphPersonRepository(PersonRepository):
    def __init__(self, store: Store):
        self.store = store
        self.graph = NamedNode("http://example.com/graph/persons")

    def _person_to_quads(self, person: Person) -> List[Quad]:
        subject = NamedNode(f"http://example.com/person/{person.id}")

        quads = [
            Quad(
                subject,
                NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                NamedNode("http://example.com/Person"),
                self.graph
            ),
            Quad(
                subject,
                NamedNode("http://example.com/name"),
                Literal(person.name),
                self.graph
            ),
            Quad(
                subject,
                NamedNode("http://example.com/email"),
                Literal(person.email),
                self.graph
            ),
        ]

        if person.age is not None:
            quads.append(Quad(
                subject,
                NamedNode("http://example.com/age"),
                Literal(str(person.age), datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer")),
                self.graph
            ))

        return quads

    def _parse_person(self, bindings: dict) -> Person:
        person_id = bindings['id'].value.split('/')[-1]
        name = bindings['name'].value
        email = bindings['email'].value
        age = int(bindings['age'].value) if 'age' in bindings else None

        return Person(id=person_id, name=name, email=email, age=age)

    def find_by_id(self, id: str) -> Optional[Person]:
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{self.graph.value}> {{
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

    def find_by_email(self, email: str) -> Optional[Person]:
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{self.graph.value}> {{
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

    def find_all(self) -> List[Person]:
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{self.graph.value}> {{
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
        # Delete existing data
        self.delete(person.id)

        # Insert new data
        for quad in self._person_to_quads(person):
            self.store.add(quad)

    def delete(self, id: str) -> bool:
        subject = NamedNode(f"http://example.com/person/{id}")

        # Find all quads for this person
        quads = list(self.store.quads_for_pattern(subject, None, None, self.graph))
        found = len(quads) > 0

        # Remove them
        for quad in quads:
            self.store.remove(quad)

        return found

    def count(self) -> int:
        query = f'''
            PREFIX ex: <http://example.com/>
            SELECT (COUNT(?id) AS ?count)
            WHERE {{
                GRAPH <{self.graph.value}> {{
                    ?id a ex:Person .
                }}
            }}
        '''

        results = self.store.query(query)
        for bindings in results:
            return int(bindings['count'].value)

        return 0
```

#### Mock Repository for Testing

```python
# repositories/mock_person_repository.py
from typing import List, Optional, Dict
from domain.person import Person
from repositories.person_repository import PersonRepository

class MockPersonRepository(PersonRepository):
    def __init__(self, initial_data: Optional[List[Person]] = None):
        self.data: Dict[str, Person] = {}
        if initial_data:
            for person in initial_data:
                self.data[person.id] = person

    def find_by_id(self, id: str) -> Optional[Person]:
        return self.data.get(id)

    def find_by_email(self, email: str) -> Optional[Person]:
        for person in self.data.values():
            if person.email == email:
                return person
        return None

    def find_all(self) -> List[Person]:
        return sorted(self.data.values(), key=lambda p: p.name)

    def save(self, person: Person) -> None:
        self.data[person.id] = person

    def delete(self, id: str) -> bool:
        if id in self.data:
            del self.data[id]
            return True
        return False

    def count(self) -> int:
        return len(self.data)
```

#### Unit Tests

```python
# tests/test_person_repository.py
import unittest
from domain.person import Person
from repositories.mock_person_repository import MockPersonRepository

class TestPersonRepository(unittest.TestCase):
    def test_find_by_id(self):
        person = Person("1", "Alice", "alice@example.com")
        repo = MockPersonRepository([person])

        found = repo.find_by_id("1")
        self.assertEqual(found, person)

        not_found = repo.find_by_id("999")
        self.assertIsNone(not_found)

    def test_save_and_count(self):
        repo = MockPersonRepository()
        self.assertEqual(repo.count(), 0)

        person = Person("1", "Bob", "bob@example.com")
        repo.save(person)

        self.assertEqual(repo.count(), 1)
        self.assertEqual(repo.find_by_id("1"), person)

    def test_delete(self):
        person = Person("1", "Charlie", "charlie@example.com")
        repo = MockPersonRepository([person])

        self.assertTrue(repo.delete("1"))
        self.assertEqual(repo.count(), 0)
        self.assertFalse(repo.delete("1"))

if __name__ == '__main__':
    unittest.main()
```

---

### JavaScript Implementation

#### Domain Model

```javascript
// domain/Person.js
export class Person {
    constructor(id, name, email, age = null) {
        this.id = id;
        this.name = name;
        this.email = email;
        this.age = age;
    }

    withAge(age) {
        this.age = age;
        return this;
    }

    toJSON() {
        return {
            id: this.id,
            name: this.name,
            email: this.email,
            age: this.age,
        };
    }
}
```

#### Repository Interface

```javascript
// repositories/PersonRepository.js
export class PersonRepository {
    async findById(id) {
        throw new Error('Not implemented');
    }

    async findByEmail(email) {
        throw new Error('Not implemented');
    }

    async findAll() {
        throw new Error('Not implemented');
    }

    async save(person) {
        throw new Error('Not implemented');
    }

    async delete(id) {
        throw new Error('Not implemented');
    }

    async count() {
        throw new Error('Not implemented');
    }
}
```

#### Oxigraph Implementation

```javascript
// repositories/OxigraphPersonRepository.js
import { NamedNode, Literal, Quad, DefaultGraph } from 'oxigraph';
import { Person } from '../domain/Person.js';
import { PersonRepository } from './PersonRepository.js';

export class OxigraphPersonRepository extends PersonRepository {
    constructor(store) {
        super();
        this.store = store;
        this.graph = new NamedNode('http://example.com/graph/persons');
    }

    personToQuads(person) {
        const subject = new NamedNode(`http://example.com/person/${person.id}`);

        const quads = [
            new Quad(
                subject,
                new NamedNode('http://www.w3.org/1999/02/22-rdf-syntax-ns#type'),
                new NamedNode('http://example.com/Person'),
                this.graph
            ),
            new Quad(
                subject,
                new NamedNode('http://example.com/name'),
                new Literal(person.name),
                this.graph
            ),
            new Quad(
                subject,
                new NamedNode('http://example.com/email'),
                new Literal(person.email),
                this.graph
            ),
        ];

        if (person.age !== null) {
            quads.push(new Quad(
                subject,
                new NamedNode('http://example.com/age'),
                Literal.integer(person.age),
                this.graph
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
                GRAPH <${this.graph.value}> {
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

    async findByEmail(email) {
        const query = `
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {
                GRAPH <${this.graph.value}> {
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

    async findAll() {
        const query = `
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {
                GRAPH <${this.graph.value}> {
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
        // Delete existing data
        await this.delete(person.id);

        // Insert new data
        for (const quad of this.personToQuads(person)) {
            this.store.add(quad);
        }
    }

    async delete(id) {
        const subject = new NamedNode(`http://example.com/person/${id}`);

        const quads = this.store.match(subject, null, null, this.graph);
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
                GRAPH <${this.graph.value}> {
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
}
```

#### Mock Repository for Testing

```javascript
// repositories/MockPersonRepository.js
import { PersonRepository } from './PersonRepository.js';

export class MockPersonRepository extends PersonRepository {
    constructor(initialData = []) {
        super();
        this.data = new Map();
        for (const person of initialData) {
            this.data.set(person.id, person);
        }
    }

    async findById(id) {
        return this.data.get(id) || null;
    }

    async findByEmail(email) {
        for (const person of this.data.values()) {
            if (person.email === email) {
                return person;
            }
        }
        return null;
    }

    async findAll() {
        return Array.from(this.data.values())
            .sort((a, b) => a.name.localeCompare(b.name));
    }

    async save(person) {
        this.data.set(person.id, person);
    }

    async delete(id) {
        return this.data.delete(id);
    }

    async count() {
        return this.data.size;
    }
}
```

#### Unit Tests

```javascript
// tests/PersonRepository.test.js
import { describe, it, expect } from 'vitest';
import { Person } from '../domain/Person.js';
import { MockPersonRepository } from '../repositories/MockPersonRepository.js';

describe('PersonRepository', () => {
    it('should find person by id', async () => {
        const person = new Person('1', 'Alice', 'alice@example.com');
        const repo = new MockPersonRepository([person]);

        const found = await repo.findById('1');
        expect(found).toEqual(person);

        const notFound = await repo.findById('999');
        expect(notFound).toBeNull();
    });

    it('should save and count', async () => {
        const repo = new MockPersonRepository();
        expect(await repo.count()).toBe(0);

        const person = new Person('1', 'Bob', 'bob@example.com');
        await repo.save(person);

        expect(await repo.count()).toBe(1);
        expect(await repo.findById('1')).toEqual(person);
    });

    it('should delete', async () => {
        const person = new Person('1', 'Charlie', 'charlie@example.com');
        const repo = new MockPersonRepository([person]);

        expect(await repo.delete('1')).toBe(true);
        expect(await repo.count()).toBe(0);
        expect(await repo.delete('1')).toBe(false);
    });
});
```

---

## Query Builder Pattern

For complex queries, add a query builder:

### Rust Query Builder

```rust
pub struct PersonQuery {
    min_age: Option<u32>,
    max_age: Option<u32>,
    name_contains: Option<String>,
    limit: Option<usize>,
}

impl PersonQuery {
    pub fn new() -> Self {
        Self {
            min_age: None,
            max_age: None,
            name_contains: None,
            limit: None,
        }
    }

    pub fn with_min_age(mut self, age: u32) -> Self {
        self.min_age = Some(age);
        self
    }

    pub fn with_max_age(mut self, age: u32) -> Self {
        self.max_age = Some(age);
        self
    }

    pub fn with_name_containing(mut self, text: String) -> Self {
        self.name_contains = Some(text);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn to_sparql(&self, graph_iri: &str) -> String {
        let mut filters = Vec::new();

        if let Some(min_age) = self.min_age {
            filters.push(format!("?age >= {}", min_age));
        }

        if let Some(max_age) = self.max_age {
            filters.push(format!("?age <= {}", max_age));
        }

        if let Some(name_contains) = &self.name_contains {
            filters.push(format!(r#"CONTAINS(LCASE(?name), LCASE("{}"))"#, name_contains));
        }

        let filter_clause = if !filters.is_empty() {
            format!("FILTER ({})", filters.join(" && "))
        } else {
            String::new()
        };

        let limit_clause = if let Some(limit) = self.limit {
            format!("LIMIT {}", limit)
        } else {
            String::new()
        };

        format!(
            r#"
            PREFIX ex: <http://example.com/>
            SELECT ?id ?name ?email ?age
            WHERE {{
                GRAPH <{}> {{
                    ?id a ex:Person ;
                        ex:name ?name ;
                        ex:email ?email .
                    OPTIONAL {{ ?id ex:age ?age }}
                    {}
                }}
            }}
            ORDER BY ?name
            {}
            "#,
            graph_iri, filter_clause, limit_clause
        )
    }
}

// Usage
let query = PersonQuery::new()
    .with_min_age(18)
    .with_max_age(65)
    .with_name_containing("john".to_string())
    .with_limit(10);

let results = repository.query(query)?;
```

---

## Best Practices

### ✅ DO:

**Use Domain Objects** - Return domain models, not RDF terms
```rust
// Good
fn find_by_id(&self, id: &str) -> Result<Option<Person>>;

// Bad
fn find_by_id(&self, id: &str) -> Result<Option<Vec<Quad>>>;
```

**Abstract SPARQL** - Hide query complexity behind methods
```rust
// Good
repo.find_active_users()?;

// Bad - business logic shouldn't write SPARQL
store.query("SELECT ?user WHERE { ?user ex:active true }")?;
```

**Use Transactions** - Ensure consistency
```rust
repo.save_with_address(person, address)?; // Single transaction
```

**Handle Errors Gracefully** - Provide context
```rust
self.store.insert(&quad)
    .map_err(|e| RepositoryError::SaveFailed(person.id.clone(), e))?;
```

### ❌ DON'T:

**Leak RDF Abstractions**
```rust
// Bad - exposing Quad in domain layer
fn get_user_quads(&self, id: &str) -> Vec<Quad>;
```

**Mix Concerns**
```rust
// Bad - HTTP handling in repository
fn save_and_notify_webhooks(&self, person: Person) -> Result<()>;
```

**Over-Generalize**
```rust
// Bad - too generic
fn query(&self, sparql: &str) -> Vec<HashMap<String, String>>;

// Good - specific to use case
fn find_by_age_range(&self, min: u32, max: u32) -> Vec<Person>;
```

---

## Performance Tips

1. **Bulk Operations** - Batch saves/deletes
2. **Streaming** - Return iterators for large result sets
3. **Pagination** - Add offset/limit to queries
4. **Caching** - Cache frequently accessed entities
5. **Connection Pooling** - Reuse Store instances

---

## Next Steps

- Add [Caching](./caching.md) layer to repository
- Implement [Multi-Tenancy](./multi-tenancy.md) in repository
- Use [Event Sourcing](./event-sourcing.md) with repository pattern
- Read [Explanation: Architecture](../explanation/architecture.md)

---

## Summary

The Repository Pattern provides:
- **Clean separation** between business logic and data access
- **Testability** through mock implementations
- **Flexibility** to change storage backends
- **Maintainability** with domain-focused interfaces

Start with a simple repository for one entity, then expand as your application grows.
