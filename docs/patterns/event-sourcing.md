# Event Sourcing Pattern

**Store events as immutable RDF triples for complete audit trails and temporal queries**

Event Sourcing captures all changes to application state as a sequence of events. Instead of storing just the current state, you store every event that led to that state. RDF's graph model is a natural fit for event sourcing, enabling powerful temporal queries and audit trails.

## When to Use

**Use Event Sourcing when:**
- Need complete audit trails for compliance (GDPR, SOX, HIPAA)
- Want to reconstruct state at any point in time
- Building event-driven architectures
- Require "time-travel" queries (what did the data look like on date X?)
- Need to replay events to rebuild state or fix bugs
- Working in domains with complex business logic requiring event replay

**Skip this pattern when:**
- Application is simple CRUD with no audit requirements
- Storage costs are a major concern (events accumulate)
- Team unfamiliar with event sourcing concepts
- Real-time queries on current state are only requirement

## Benefits

✅ **Complete Audit Trail** - Know who changed what, when, and why
✅ **Temporal Queries** - Query state at any point in history
✅ **Event Replay** - Rebuild state or fix bugs by replaying events
✅ **Debugging** - Understand exactly how state evolved
✅ **Compliance** - Meet regulatory requirements for data lineage
✅ **Event-Driven** - Publish events to trigger other processes

## Architecture

```
┌──────────────────────────────────────────────────┐
│  Command                                          │
│  (CreateUser, UpdateEmail, DeleteUser)           │
└────────────────┬─────────────────────────────────┘
                 │
                 ↓
┌──────────────────────────────────────────────────┐
│  Event Store                                      │
│  (Append-only events in RDF)                     │
└────────────────┬─────────────────────────────────┘
                 │
     ┌───────────┴───────────┐
     ↓                       ↓
┌──────────┐          ┌──────────────┐
│  Events  │          │  Projections │
│  (audit) │          │  (read model)│
└──────────┘          └──────────────┘
```

### Event Flow

1. **Command** arrives (e.g., "Update user email")
2. **Validate** command against business rules
3. **Create event** (UserEmailUpdated)
4. **Append event** to event store (immutable)
5. **Update projection** (current state / read model)
6. **Publish event** (optional) for other systems

---

## RDF Event Model

### Event Schema

Events are modeled as RDF triples with reification for metadata:

```turtle
# Event 1: User Created
<http://example.com/event/001> a ex:UserCreatedEvent ;
    ex:eventId "001" ;
    ex:eventType "UserCreated" ;
    ex:timestamp "2024-01-15T10:30:00Z"^^xsd:dateTime ;
    ex:userId "user-123" ;
    ex:userName "Alice Smith" ;
    ex:userEmail "alice@example.com" ;
    ex:performedBy <http://example.com/user/admin> ;
    ex:version 1 .

# Event 2: Email Updated
<http://example.com/event/002> a ex:UserEmailUpdatedEvent ;
    ex:eventId "002" ;
    ex:eventType "UserEmailUpdated" ;
    ex:timestamp "2024-01-16T14:22:00Z"^^xsd:dateTime ;
    ex:userId "user-123" ;
    ex:oldEmail "alice@example.com" ;
    ex:newEmail "alice.smith@newcompany.com" ;
    ex:performedBy <http://example.com/user/alice> ;
    ex:version 2 .

# Event 3: User Deleted
<http://example.com/event/003> a ex:UserDeletedEvent ;
    ex:eventId "003" ;
    ex:eventType "UserDeleted" ;
    ex:timestamp "2024-02-20T09:15:00Z"^^xsd:dateTime ;
    ex:userId "user-123" ;
    ex:reason "User requested account deletion" ;
    ex:performedBy <http://example.com/user/alice> ;
    ex:version 3 .
```

---

## Implementation Examples

### Rust Implementation

#### Event Types

```rust
// src/events/user_events.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UserEvent {
    UserCreated {
        event_id: String,
        timestamp: DateTime<Utc>,
        user_id: String,
        name: String,
        email: String,
        performed_by: String,
        version: u64,
    },
    UserEmailUpdated {
        event_id: String,
        timestamp: DateTime<Utc>,
        user_id: String,
        old_email: String,
        new_email: String,
        performed_by: String,
        version: u64,
    },
    UserDeleted {
        event_id: String,
        timestamp: DateTime<Utc>,
        user_id: String,
        reason: String,
        performed_by: String,
        version: u64,
    },
}

impl UserEvent {
    pub fn event_id(&self) -> &str {
        match self {
            UserEvent::UserCreated { event_id, .. } => event_id,
            UserEvent::UserEmailUpdated { event_id, .. } => event_id,
            UserEvent::UserDeleted { event_id, .. } => event_id,
        }
    }

    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            UserEvent::UserCreated { timestamp, .. } => timestamp,
            UserEvent::UserEmailUpdated { timestamp, .. } => timestamp,
            UserEvent::UserDeleted { timestamp, .. } => timestamp,
        }
    }

    pub fn user_id(&self) -> &str {
        match self {
            UserEvent::UserCreated { user_id, .. } => user_id,
            UserEvent::UserEmailUpdated { user_id, .. } => user_id,
            UserEvent::UserDeleted { user_id, .. } => user_id,
        }
    }

    pub fn version(&self) -> u64 {
        match self {
            UserEvent::UserCreated { version, .. } => *version,
            UserEvent::UserEmailUpdated { version, .. } => *version,
            UserEvent::UserDeleted { version, .. } => *version,
        }
    }
}
```

#### Event Store

```rust
// src/events/event_store.rs
use crate::events::UserEvent;
use chrono::{DateTime, Utc};
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;

pub struct EventStore {
    store: Arc<Store>,
    graph: NamedNode,
}

impl EventStore {
    pub fn new(store: Arc<Store>) -> Result<Self, Box<dyn Error>> {
        let graph = NamedNode::new("http://example.com/graph/events")?;
        Ok(Self { store, graph })
    }

    pub fn append(&self, event: &UserEvent) -> Result<(), Box<dyn Error>> {
        let quads = self.event_to_quads(event)?;

        // Ensure immutability - check if event already exists
        if self.event_exists(event.event_id())? {
            return Err("Event already exists - events are immutable".into());
        }

        for quad in quads {
            self.store.insert(&quad)?;
        }

        Ok(())
    }

    pub fn get_events_for_user(
        &self,
        user_id: &str,
        from_version: Option<u64>,
    ) -> Result<Vec<UserEvent>, Box<dyn Error>> {
        let version_filter = if let Some(version) = from_version {
            format!("FILTER (?version >= {})", version)
        } else {
            String::new()
        };

        let query = format!(
            r#"
            PREFIX ex: <http://example.com/>
            PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
            SELECT ?event ?eventType ?eventId ?timestamp ?version
            WHERE {{
                GRAPH <{}> {{
                    ?event ex:userId "{}" ;
                           ex:eventType ?eventType ;
                           ex:eventId ?eventId ;
                           ex:timestamp ?timestamp ;
                           ex:version ?version .
                    {}
                }}
            }}
            ORDER BY ?version
            "#,
            self.graph.as_str(),
            user_id,
            version_filter
        );

        let mut events = Vec::new();

        if let QueryResults::Solutions(solutions) = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&self.store)
            .execute()?
        {
            for solution in solutions {
                let bindings = solution?;
                let event_iri = bindings.get("event").ok_or("Missing event")?;
                events.push(self.load_event(event_iri)?);
            }
        }

        Ok(events)
    }

    pub fn get_events_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<UserEvent>, Box<dyn Error>> {
        let query = format!(
            r#"
            PREFIX ex: <http://example.com/>
            PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
            SELECT ?event
            WHERE {{
                GRAPH <{}> {{
                    ?event ex:timestamp ?timestamp .
                    FILTER (?timestamp >= "{}"^^xsd:dateTime)
                }}
            }}
            ORDER BY ?timestamp
            "#,
            self.graph.as_str(),
            since.to_rfc3339()
        );

        let mut events = Vec::new();

        if let QueryResults::Solutions(solutions) = SparqlEvaluator::new()
            .parse_query(&query)?
            .on_store(&self.store)
            .execute()?
        {
            for solution in solutions {
                let bindings = solution?;
                let event_iri = bindings.get("event").ok_or("Missing event")?;
                events.push(self.load_event(event_iri)?);
            }
        }

        Ok(events)
    }

    pub fn replay_to_state(&self, user_id: &str) -> Result<Option<UserState>, Box<dyn Error>> {
        let events = self.get_events_for_user(user_id, None)?;

        if events.is_empty() {
            return Ok(None);
        }

        let mut state = UserState::default();

        for event in events {
            state.apply(event);
        }

        Ok(Some(state))
    }

    pub fn replay_to_timestamp(
        &self,
        user_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<Option<UserState>, Box<dyn Error>> {
        let events = self.get_events_for_user(user_id, None)?;

        if events.is_empty() {
            return Ok(None);
        }

        let mut state = UserState::default();

        for event in events {
            if event.timestamp() <= &timestamp {
                state.apply(event);
            } else {
                break;
            }
        }

        if state.user_id.is_none() {
            return Ok(None);
        }

        Ok(Some(state))
    }

    fn event_to_quads(&self, event: &UserEvent) -> Result<Vec<Quad>, Box<dyn Error>> {
        let event_iri = NamedNode::new(&format!("http://example.com/event/{}", event.event_id()))?;
        let graph_name = GraphName::NamedNode(self.graph.clone());

        let mut quads = vec![
            Quad::new(
                event_iri.clone(),
                NamedNode::new("http://example.com/eventId")?,
                Literal::new_simple_literal(event.event_id()),
                graph_name.clone(),
            ),
            Quad::new(
                event_iri.clone(),
                NamedNode::new("http://example.com/timestamp")?,
                Literal::new_typed_literal(
                    &event.timestamp().to_rfc3339(),
                    NamedNode::new("http://www.w3.org/2001/XMLSchema#dateTime")?,
                ),
                graph_name.clone(),
            ),
            Quad::new(
                event_iri.clone(),
                NamedNode::new("http://example.com/userId")?,
                Literal::new_simple_literal(event.user_id()),
                graph_name.clone(),
            ),
            Quad::new(
                event_iri.clone(),
                NamedNode::new("http://example.com/version")?,
                Literal::new_typed_literal(
                    &event.version().to_string(),
                    NamedNode::new("http://www.w3.org/2001/XMLSchema#integer")?,
                ),
                graph_name.clone(),
            ),
        ];

        match event {
            UserEvent::UserCreated {
                name,
                email,
                performed_by,
                ..
            } => {
                quads.extend(vec![
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
                        NamedNode::new("http://example.com/UserCreatedEvent")?,
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/eventType")?,
                        Literal::new_simple_literal("UserCreated"),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/userName")?,
                        Literal::new_simple_literal(name),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/userEmail")?,
                        Literal::new_simple_literal(email),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri,
                        NamedNode::new("http://example.com/performedBy")?,
                        NamedNode::new(performed_by)?,
                        graph_name,
                    ),
                ]);
            }
            UserEvent::UserEmailUpdated {
                old_email,
                new_email,
                performed_by,
                ..
            } => {
                quads.extend(vec![
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
                        NamedNode::new("http://example.com/UserEmailUpdatedEvent")?,
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/eventType")?,
                        Literal::new_simple_literal("UserEmailUpdated"),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/oldEmail")?,
                        Literal::new_simple_literal(old_email),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/newEmail")?,
                        Literal::new_simple_literal(new_email),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri,
                        NamedNode::new("http://example.com/performedBy")?,
                        NamedNode::new(performed_by)?,
                        graph_name,
                    ),
                ]);
            }
            UserEvent::UserDeleted {
                reason,
                performed_by,
                ..
            } => {
                quads.extend(vec![
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
                        NamedNode::new("http://example.com/UserDeletedEvent")?,
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/eventType")?,
                        Literal::new_simple_literal("UserDeleted"),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri.clone(),
                        NamedNode::new("http://example.com/reason")?,
                        Literal::new_simple_literal(reason),
                        graph_name.clone(),
                    ),
                    Quad::new(
                        event_iri,
                        NamedNode::new("http://example.com/performedBy")?,
                        NamedNode::new(performed_by)?,
                        graph_name,
                    ),
                ]);
            }
        }

        Ok(quads)
    }

    fn event_exists(&self, event_id: &str) -> Result<bool, Box<dyn Error>> {
        let event_iri = NamedNode::new(&format!("http://example.com/event/{}", event_id))?;
        let graph_name = GraphName::NamedNode(self.graph.clone());

        let mut quads = self.store.quads_for_pattern(
            Some(event_iri.as_ref()),
            None,
            None,
            Some(graph_name.as_ref()),
        );

        Ok(quads.next().is_some())
    }

    fn load_event(&self, event_iri: &Term) -> Result<UserEvent, Box<dyn Error>> {
        // Load event details from store and reconstruct UserEvent
        // Implementation details omitted for brevity
        todo!("Load event from RDF representation")
    }
}

#[derive(Debug, Default, Clone)]
pub struct UserState {
    pub user_id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub deleted: bool,
}

impl UserState {
    pub fn apply(&mut self, event: UserEvent) {
        match event {
            UserEvent::UserCreated {
                user_id,
                name,
                email,
                ..
            } => {
                self.user_id = Some(user_id);
                self.name = Some(name);
                self.email = Some(email);
                self.deleted = false;
            }
            UserEvent::UserEmailUpdated { new_email, .. } => {
                self.email = Some(new_email);
            }
            UserEvent::UserDeleted { .. } => {
                self.deleted = true;
            }
        }
    }
}
```

#### Command Handler

```rust
// src/commands/user_commands.rs
use crate::events::{EventStore, UserEvent};
use chrono::Utc;
use std::error::Error;
use uuid::Uuid;

pub struct UserCommandHandler {
    event_store: EventStore,
}

impl UserCommandHandler {
    pub fn new(event_store: EventStore) -> Self {
        Self { event_store }
    }

    pub fn create_user(
        &self,
        name: String,
        email: String,
        performed_by: String,
    ) -> Result<String, Box<dyn Error>> {
        let user_id = Uuid::new_v4().to_string();
        let event_id = Uuid::new_v4().to_string();

        let event = UserEvent::UserCreated {
            event_id,
            timestamp: Utc::now(),
            user_id: user_id.clone(),
            name,
            email,
            performed_by,
            version: 1,
        };

        self.event_store.append(&event)?;

        Ok(user_id)
    }

    pub fn update_email(
        &self,
        user_id: String,
        new_email: String,
        performed_by: String,
    ) -> Result<(), Box<dyn Error>> {
        // Load current state to get old email and version
        let state = self
            .event_store
            .replay_to_state(&user_id)?
            .ok_or("User not found")?;

        if state.deleted {
            return Err("Cannot update deleted user".into());
        }

        let old_email = state.email.ok_or("User has no email")?;
        let events = self.event_store.get_events_for_user(&user_id, None)?;
        let version = events.len() as u64 + 1;

        let event = UserEvent::UserEmailUpdated {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            user_id,
            old_email,
            new_email,
            performed_by,
            version,
        };

        self.event_store.append(&event)?;

        Ok(())
    }

    pub fn delete_user(
        &self,
        user_id: String,
        reason: String,
        performed_by: String,
    ) -> Result<(), Box<dyn Error>> {
        let events = self.event_store.get_events_for_user(&user_id, None)?;

        if events.is_empty() {
            return Err("User not found".into());
        }

        let version = events.len() as u64 + 1;

        let event = UserEvent::UserDeleted {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            user_id,
            reason,
            performed_by,
            version,
        };

        self.event_store.append(&event)?;

        Ok(())
    }
}
```

---

### Python Implementation

#### Event Types

```python
# events/user_events.py
from dataclasses import dataclass
from datetime import datetime
from typing import Literal

@dataclass
class UserCreatedEvent:
    event_id: str
    timestamp: datetime
    user_id: str
    name: str
    email: str
    performed_by: str
    version: int
    event_type: Literal['UserCreated'] = 'UserCreated'

@dataclass
class UserEmailUpdatedEvent:
    event_id: str
    timestamp: datetime
    user_id: str
    old_email: str
    new_email: str
    performed_by: str
    version: int
    event_type: Literal['UserEmailUpdated'] = 'UserEmailUpdated'

@dataclass
class UserDeletedEvent:
    event_id: str
    timestamp: datetime
    user_id: str
    reason: str
    performed_by: str
    version: int
    event_type: Literal['UserDeleted'] = 'UserDeleted'

UserEvent = UserCreatedEvent | UserEmailUpdatedEvent | UserDeletedEvent
```

#### Event Store

```python
# events/event_store.py
from datetime import datetime
from typing import List, Optional
from pyoxigraph import Store, NamedNode, Literal, Quad
from events.user_events import (
    UserEvent,
    UserCreatedEvent,
    UserEmailUpdatedEvent,
    UserDeletedEvent,
)
import uuid

class EventStore:
    def __init__(self, store: Store):
        self.store = store
        self.graph = NamedNode("http://example.com/graph/events")

    def append(self, event: UserEvent) -> None:
        """Append an event to the event store (immutable)."""
        if self._event_exists(event.event_id):
            raise ValueError("Event already exists - events are immutable")

        quads = self._event_to_quads(event)
        for quad in quads:
            self.store.add(quad)

    def get_events_for_user(
        self, user_id: str, from_version: Optional[int] = None
    ) -> List[UserEvent]:
        """Get all events for a user, optionally from a specific version."""
        version_filter = f"FILTER (?version >= {from_version})" if from_version else ""

        query = f'''
            PREFIX ex: <http://example.com/>
            PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
            SELECT ?event ?eventType
            WHERE {{
                GRAPH <{self.graph.value}> {{
                    ?event ex:userId "{user_id}" ;
                           ex:eventType ?eventType ;
                           ex:version ?version .
                    {version_filter}
                }}
            }}
            ORDER BY ?version
        '''

        events = []
        results = self.store.query(query)

        for bindings in results:
            event_iri = bindings['event']
            events.append(self._load_event(event_iri))

        return events

    def get_events_since(self, since: datetime) -> List[UserEvent]:
        """Get all events since a timestamp."""
        query = f'''
            PREFIX ex: <http://example.com/>
            PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
            SELECT ?event
            WHERE {{
                GRAPH <{self.graph.value}> {{
                    ?event ex:timestamp ?timestamp .
                    FILTER (?timestamp >= "{since.isoformat()}"^^xsd:dateTime)
                }}
            }}
            ORDER BY ?timestamp
        '''

        events = []
        results = self.store.query(query)

        for bindings in results:
            event_iri = bindings['event']
            events.append(self._load_event(event_iri))

        return events

    def replay_to_state(self, user_id: str) -> Optional['UserState']:
        """Replay all events to reconstruct current state."""
        events = self.get_events_for_user(user_id)

        if not events:
            return None

        state = UserState()
        for event in events:
            state.apply(event)

        return state

    def replay_to_timestamp(
        self, user_id: str, timestamp: datetime
    ) -> Optional['UserState']:
        """Replay events up to a specific timestamp (time-travel query)."""
        events = self.get_events_for_user(user_id)

        if not events:
            return None

        state = UserState()
        for event in events:
            if event.timestamp <= timestamp:
                state.apply(event)
            else:
                break

        if state.user_id is None:
            return None

        return state

    def _event_to_quads(self, event: UserEvent) -> List[Quad]:
        """Convert event to RDF quads."""
        event_iri = NamedNode(f"http://example.com/event/{event.event_id}")

        quads = [
            Quad(
                event_iri,
                NamedNode("http://example.com/eventId"),
                Literal(event.event_id),
                self.graph,
            ),
            Quad(
                event_iri,
                NamedNode("http://example.com/timestamp"),
                Literal(
                    event.timestamp.isoformat(),
                    datatype=NamedNode("http://www.w3.org/2001/XMLSchema#dateTime"),
                ),
                self.graph,
            ),
            Quad(
                event_iri,
                NamedNode("http://example.com/userId"),
                Literal(event.user_id),
                self.graph,
            ),
            Quad(
                event_iri,
                NamedNode("http://example.com/eventType"),
                Literal(event.event_type),
                self.graph,
            ),
            Quad(
                event_iri,
                NamedNode("http://example.com/version"),
                Literal(
                    str(event.version),
                    datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"),
                ),
                self.graph,
            ),
        ]

        if isinstance(event, UserCreatedEvent):
            quads.extend([
                Quad(
                    event_iri,
                    NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                    NamedNode("http://example.com/UserCreatedEvent"),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/userName"),
                    Literal(event.name),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/userEmail"),
                    Literal(event.email),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/performedBy"),
                    NamedNode(event.performed_by),
                    self.graph,
                ),
            ])
        elif isinstance(event, UserEmailUpdatedEvent):
            quads.extend([
                Quad(
                    event_iri,
                    NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                    NamedNode("http://example.com/UserEmailUpdatedEvent"),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/oldEmail"),
                    Literal(event.old_email),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/newEmail"),
                    Literal(event.new_email),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/performedBy"),
                    NamedNode(event.performed_by),
                    self.graph,
                ),
            ])
        elif isinstance(event, UserDeletedEvent):
            quads.extend([
                Quad(
                    event_iri,
                    NamedNode("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
                    NamedNode("http://example.com/UserDeletedEvent"),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/reason"),
                    Literal(event.reason),
                    self.graph,
                ),
                Quad(
                    event_iri,
                    NamedNode("http://example.com/performedBy"),
                    NamedNode(event.performed_by),
                    self.graph,
                ),
            ])

        return quads

    def _event_exists(self, event_id: str) -> bool:
        """Check if event already exists."""
        event_iri = NamedNode(f"http://example.com/event/{event_id}")
        quads = list(self.store.quads_for_pattern(event_iri, None, None, self.graph))
        return len(quads) > 0

    def _load_event(self, event_iri: NamedNode) -> UserEvent:
        """Load event from RDF representation."""
        # Implementation omitted for brevity
        pass


class UserState:
    def __init__(self):
        self.user_id: Optional[str] = None
        self.name: Optional[str] = None
        self.email: Optional[str] = None
        self.deleted: bool = False

    def apply(self, event: UserEvent) -> None:
        """Apply an event to update state."""
        if isinstance(event, UserCreatedEvent):
            self.user_id = event.user_id
            self.name = event.name
            self.email = event.email
            self.deleted = False
        elif isinstance(event, UserEmailUpdatedEvent):
            self.email = event.new_email
        elif isinstance(event, UserDeletedEvent):
            self.deleted = True
```

---

### JavaScript Implementation

```javascript
// events/EventStore.js
import { NamedNode, Literal, Quad } from 'oxigraph';
import { v4 as uuidv4 } from 'uuid';

export class EventStore {
    constructor(store) {
        this.store = store;
        this.graph = new NamedNode('http://example.com/graph/events');
    }

    append(event) {
        if (this.eventExists(event.eventId)) {
            throw new Error('Event already exists - events are immutable');
        }

        const quads = this.eventToQuads(event);
        for (const quad of quads) {
            this.store.add(quad);
        }
    }

    getEventsForUser(userId, fromVersion = null) {
        const versionFilter = fromVersion
            ? `FILTER (?version >= ${fromVersion})`
            : '';

        const query = `
            PREFIX ex: <http://example.com/>
            SELECT ?event ?eventType
            WHERE {
                GRAPH <${this.graph.value}> {
                    ?event ex:userId "${userId}" ;
                           ex:eventType ?eventType ;
                           ex:version ?version .
                    ${versionFilter}
                }
            }
            ORDER BY ?version
        `;

        const events = [];
        const results = this.store.query(query);

        for (const bindings of results) {
            const eventIri = bindings.get('event');
            events.push(this.loadEvent(eventIri));
        }

        return events;
    }

    replayToState(userId) {
        const events = this.getEventsForUser(userId);

        if (events.length === 0) {
            return null;
        }

        const state = new UserState();
        for (const event of events) {
            state.apply(event);
        }

        return state;
    }

    replayToTimestamp(userId, timestamp) {
        const events = this.getEventsForUser(userId);

        if (events.length === 0) {
            return null;
        }

        const state = new UserState();
        for (const event of events) {
            if (event.timestamp <= timestamp) {
                state.apply(event);
            } else {
                break;
            }
        }

        if (!state.userId) {
            return null;
        }

        return state;
    }

    eventToQuads(event) {
        const eventIri = new NamedNode(`http://example.com/event/${event.eventId}`);
        const quads = [
            new Quad(
                eventIri,
                new NamedNode('http://example.com/eventId'),
                new Literal(event.eventId),
                this.graph
            ),
            new Quad(
                eventIri,
                new NamedNode('http://example.com/timestamp'),
                Literal.typedLiteral(
                    event.timestamp.toISOString(),
                    new NamedNode('http://www.w3.org/2001/XMLSchema#dateTime')
                ),
                this.graph
            ),
            new Quad(
                eventIri,
                new NamedNode('http://example.com/userId'),
                new Literal(event.userId),
                this.graph
            ),
            new Quad(
                eventIri,
                new NamedNode('http://example.com/eventType'),
                new Literal(event.eventType),
                this.graph
            ),
            new Quad(
                eventIri,
                new NamedNode('http://example.com/version'),
                Literal.integer(event.version),
                this.graph
            ),
        ];

        // Add event-specific fields based on type
        // Implementation similar to Rust/Python versions

        return quads;
    }

    eventExists(eventId) {
        const eventIri = new NamedNode(`http://example.com/event/${eventId}`);
        const quads = this.store.match(eventIri, null, null, this.graph);
        return quads.length > 0;
    }

    loadEvent(eventIri) {
        // Load event from RDF representation
        // Implementation omitted for brevity
    }
}

export class UserState {
    constructor() {
        this.userId = null;
        this.name = null;
        this.email = null;
        this.deleted = false;
    }

    apply(event) {
        switch (event.eventType) {
            case 'UserCreated':
                this.userId = event.userId;
                this.name = event.name;
                this.email = event.email;
                this.deleted = false;
                break;
            case 'UserEmailUpdated':
                this.email = event.newEmail;
                break;
            case 'UserDeleted':
                this.deleted = true;
                break;
        }
    }
}
```

---

## Temporal Queries

### Query: Who changed user's email?

```sparql
PREFIX ex: <http://example.com/>

SELECT ?timestamp ?performedBy ?oldEmail ?newEmail
WHERE {
    GRAPH <http://example.com/graph/events> {
        ?event a ex:UserEmailUpdatedEvent ;
               ex:userId "user-123" ;
               ex:timestamp ?timestamp ;
               ex:performedBy ?performedBy ;
               ex:oldEmail ?oldEmail ;
               ex:newEmail ?newEmail .
    }
}
ORDER BY ?timestamp
```

### Query: User state at specific date

```sparql
PREFIX ex: <http://example.com/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?event ?eventType ?timestamp
WHERE {
    GRAPH <http://example.com/graph/events> {
        ?event ex:userId "user-123" ;
               ex:eventType ?eventType ;
               ex:timestamp ?timestamp .
        FILTER (?timestamp <= "2024-01-31T23:59:59Z"^^xsd:dateTime)
    }
}
ORDER BY ?timestamp
```

### Query: Audit log for compliance

```sparql
PREFIX ex: <http://example.com/>

SELECT ?timestamp ?eventType ?performedBy
WHERE {
    GRAPH <http://example.com/graph/events> {
        ?event ex:timestamp ?timestamp ;
               ex:eventType ?eventType ;
               ex:performedBy ?performedBy .
    }
}
ORDER BY DESC(?timestamp)
LIMIT 100
```

---

## Best Practices

### ✅ DO:

**Immutable Events** - Never modify or delete events
**Versioning** - Include version number in each event
**Timestamps** - Always record when event occurred
**Metadata** - Track who performed the action
**Idempotency** - Prevent duplicate events
**Snapshots** - Periodically save state to avoid replaying millions of events

### ❌ DON'T:

**Delete Events** - Events are permanent audit trail
**Store Computed State** - Derive state from events, don't store it
**Complex Events** - Keep events simple and focused
**Skip Validation** - Validate before appending events

---

## Performance Optimization

1. **Snapshots** - Save state snapshots periodically to reduce replay cost
2. **Indexes** - Index userId, timestamp for fast queries
3. **Archival** - Move old events to cold storage
4. **CQRS** - Separate write (events) from read (projections) models

---

## Next Steps

- Combine with [Repository Pattern](./repository-pattern.md) for CQRS
- Add [Caching](./caching.md) for projections
- Implement [Multi-Tenancy](./multi-tenancy.md) with per-tenant event streams

---

## Summary

Event Sourcing with RDF provides:
- **Complete audit trails** for compliance
- **Temporal queries** to understand history
- **Event replay** for debugging and recovery
- **Natural fit** with RDF's graph model

Start with simple events for one aggregate, then expand as you master the pattern.
