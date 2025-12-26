# Oxigraph in Microservices Architecture

This guide shows how to integrate Oxigraph into a modern microservices architecture with service discovery, event-driven updates, and API gateway integration.

## Overview

Microservices architecture breaks applications into small, independent services. Oxigraph fits well as:

- **Data Service** - Centralized knowledge graph service
- **Query Service** - SPARQL endpoint for data access
- **Graph Service** - Multiple independent graph instances
- **Cache Layer** - Fast semantic data cache

## Architecture Patterns

### Pattern 1: Centralized Knowledge Graph Service

Single Oxigraph instance serving multiple microservices.

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Service   │────▶│             │◀────│   Service   │
│      A      │     │  Oxigraph   │     │      B      │
└─────────────┘     │   Service   │     └─────────────┘
                    │             │
┌─────────────┐     │  (Central   │     ┌─────────────┐
│   Service   │────▶│   Graph)    │◀────│   Service   │
│      C      │     │             │     │      D      │
└─────────────┘     └─────────────┘     └─────────────┘
```

### Pattern 2: Distributed Graph Instances

Each service has its own Oxigraph instance for local data.

```
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│    Service A     │  │    Service B     │  │    Service C     │
│  ┌────────────┐  │  │  ┌────────────┐  │  │  ┌────────────┐  │
│  │ Oxigraph A │  │  │  │ Oxigraph B │  │  │  │ Oxigraph C │  │
│  └────────────┘  │  │  └────────────┘  │  │  └────────────┘  │
└──────────────────┘  └──────────────────┘  └──────────────────┘
         │                     │                     │
         └─────────────────────┴─────────────────────┘
                               │
                       ┌───────▼────────┐
                       │  Message Bus   │
                       │  (Event Sync)  │
                       └────────────────┘
```

### Pattern 3: Hybrid Architecture

Combination of centralized and distributed.

```
┌──────────────────────────────────────────────────────┐
│                   API Gateway                         │
└────┬─────────────────────┬────────────────────┬──────┘
     │                     │                    │
┌────▼─────┐      ┌────────▼──────┐    ┌───────▼──────┐
│ Service  │      │   Oxigraph    │    │   Service    │
│ (Local)  │      │   (Central)   │    │   (Local)    │
│ ┌──────┐ │      │               │    │  ┌──────┐    │
│ │OxiGph│ │      │  Master Graph │    │  │OxiGph│    │
│ └──────┘ │      │               │    │  └──────┘    │
└──────────┘      └───────────────┘    └──────────────┘
```

## Implementation: Centralized Service

### Oxigraph Service (Rust)

```rust
// src/main.rs - Oxigraph microservice

use actix_web::{web, App, HttpResponse, HttpServer, middleware};
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
}

#[derive(Deserialize)]
struct QueryRequest {
    sparql: String,
}

#[derive(Serialize)]
struct QueryResponse {
    results: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    sparql: String,
}

// Health check endpoint
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "oxigraph-service"
    }))
}

// SPARQL query endpoint
async fn query(
    state: web::Data<AppState>,
    query_req: web::Json<QueryRequest>,
) -> HttpResponse {
    match state.store.query(&query_req.sparql) {
        Ok(QueryResults::Solutions(solutions)) => {
            let mut results = Vec::new();

            for solution in solutions {
                match solution {
                    Ok(sol) => {
                        let mut row = serde_json::Map::new();
                        for (var, value) in sol.iter() {
                            row.insert(
                                var.as_str().to_string(),
                                serde_json::Value::String(value.to_string())
                            );
                        }
                        results.push(serde_json::Value::Object(row));
                    }
                    Err(e) => {
                        return HttpResponse::InternalServerError()
                            .json(serde_json::json!({"error": e.to_string()}));
                    }
                }
            }

            HttpResponse::Ok().json(QueryResponse { results })
        }
        Ok(QueryResults::Boolean(b)) => {
            HttpResponse::Ok().json(serde_json::json!({"boolean": b}))
        }
        Ok(QueryResults::Graph(_)) => {
            HttpResponse::Ok().json(serde_json::json!({"type": "graph"}))
        }
        Err(e) => {
            HttpResponse::BadRequest().json(serde_json::json!({"error": e.to_string()}))
        }
    }
}

// SPARQL update endpoint
async fn update(
    state: web::Data<AppState>,
    update_req: web::Json<UpdateRequest>,
) -> HttpResponse {
    match state.store.update(&update_req.sparql) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "success"})),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({"error": e.to_string()})),
    }
}

// Get store statistics
async fn stats(state: web::Data<AppState>) -> HttpResponse {
    match state.store.len() {
        Ok(count) => {
            HttpResponse::Ok().json(serde_json::json!({
                "triple_count": count,
                "service": "oxigraph-service"
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": e.to_string()}))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Initialize store
    let store_path = std::env::var("STORE_PATH").unwrap_or_else(|_| "./data".to_string());
    let store = Store::open(&store_path)
        .expect("Failed to open Oxigraph store");

    log::info!("Oxigraph service starting with store at {}", store_path);

    let state = AppState {
        store: Arc::new(store),
    };

    // Start HTTP server
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    log::info!("Listening on {}", bind_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(middleware::Logger::default())
            .route("/health", web::get().to(health_check))
            .route("/query", web::post().to(query))
            .route("/update", web::post().to(update))
            .route("/stats", web::get().to(stats))
    })
    .bind(&bind_addr)?
    .run()
    .await
}
```

### Service Discovery with Consul

```rust
// src/service_discovery.rs

use reqwest::Client;
use serde_json::json;

pub struct ConsulClient {
    consul_addr: String,
    client: Client,
}

impl ConsulClient {
    pub fn new(consul_addr: String) -> Self {
        Self {
            consul_addr,
            client: Client::new(),
        }
    }

    pub async fn register_service(
        &self,
        service_id: &str,
        service_name: &str,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/v1/agent/service/register", self.consul_addr);

        let registration = json!({
            "ID": service_id,
            "Name": service_name,
            "Port": port,
            "Check": {
                "HTTP": format!("http://localhost:{}/health", port),
                "Interval": "10s",
                "Timeout": "5s"
            }
        });

        self.client
            .put(&url)
            .json(&registration)
            .send()
            .await?;

        log::info!("Registered service {} with Consul", service_name);

        Ok(())
    }

    pub async fn discover_service(
        &self,
        service_name: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/v1/catalog/service/{}",
            self.consul_addr, service_name
        );

        let response = self.client.get(&url).send().await?;
        let services: Vec<serde_json::Value> = response.json().await?;

        let addresses = services
            .iter()
            .filter_map(|s| {
                let addr = s["ServiceAddress"].as_str()?;
                let port = s["ServicePort"].as_u64()?;
                Some(format!("http://{}:{}", addr, port))
            })
            .collect();

        Ok(addresses)
    }
}

// Usage in main.rs
async fn register_with_consul() -> Result<(), Box<dyn std::error::Error>> {
    let consul = ConsulClient::new("http://consul:8500".to_string());

    consul.register_service(
        "oxigraph-1",
        "oxigraph-service",
        8080,
    ).await?;

    Ok(())
}
```

### Python Client Service

```python
# client_service.py - Microservice that uses Oxigraph

import requests
import consul
from flask import Flask, jsonify

app = Flask(__name__)

class OxigraphClient:
    """Client for Oxigraph microservice"""

    def __init__(self):
        self.consul = consul.Consul(host='consul', port=8500)
        self.service_name = 'oxigraph-service'

    def get_service_url(self):
        """Discover Oxigraph service via Consul"""
        _, services = self.consul.catalog.service(self.service_name)

        if not services:
            raise Exception(f"No instances of {self.service_name} found")

        # Simple round-robin (in production, use proper load balancing)
        service = services[0]
        return f"http://{service['ServiceAddress']}:{service['ServicePort']}"

    def query(self, sparql):
        """Execute SPARQL query"""
        url = f"{self.get_service_url()}/query"

        response = requests.post(
            url,
            json={'sparql': sparql},
            headers={'Content-Type': 'application/json'},
            timeout=30
        )

        response.raise_for_status()
        return response.json()

    def update(self, sparql):
        """Execute SPARQL update"""
        url = f"{self.get_service_url()}/update"

        response = requests.post(
            url,
            json={'sparql': sparql},
            headers={'Content-Type': 'application/json'},
            timeout=30
        )

        response.raise_for_status()
        return response.json()

# Initialize client
oxigraph = OxigraphClient()

@app.route('/api/entities/<entity_id>')
def get_entity(entity_id):
    """Get entity from knowledge graph"""
    query = f"""
    PREFIX ex: <http://example.org/>

    SELECT ?p ?o
    WHERE {{
        ex:{entity_id} ?p ?o .
    }}
    """

    try:
        results = oxigraph.query(query)
        return jsonify(results)
    except Exception as e:
        return jsonify({'error': str(e)}), 500

@app.route('/api/entities', methods=['POST'])
def create_entity():
    """Create new entity"""
    # Simplified example
    update = """
    PREFIX ex: <http://example.org/>

    INSERT DATA {
        ex:new_entity ex:property "value" .
    }
    """

    try:
        result = oxigraph.update(update)
        return jsonify(result)
    except Exception as e:
        return jsonify({'error': str(e)}), 500

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
```

## Event-Driven Updates

### Message Queue Integration

```python
# event_publisher.py - Publish graph updates to message queue

import pika
import json
from pyoxigraph import Store, NamedNode, Quad, Literal

class EventPublisher:
    """Publish graph changes to RabbitMQ"""

    def __init__(self, rabbitmq_host='rabbitmq'):
        self.connection = pika.BlockingConnection(
            pika.ConnectionParameters(host=rabbitmq_host)
        )
        self.channel = self.connection.channel()
        self.channel.exchange_declare(
            exchange='graph_updates',
            exchange_type='topic',
            durable=True
        )

    def publish_insert(self, subject, predicate, obj, graph=None):
        """Publish insert event"""
        event = {
            'type': 'insert',
            'subject': str(subject),
            'predicate': str(predicate),
            'object': str(obj),
            'graph': str(graph) if graph else None
        }

        self.channel.basic_publish(
            exchange='graph_updates',
            routing_key='graph.insert',
            body=json.dumps(event),
            properties=pika.BasicProperties(
                delivery_mode=2,  # Make message persistent
                content_type='application/json'
            )
        )

    def publish_delete(self, subject, predicate, obj, graph=None):
        """Publish delete event"""
        event = {
            'type': 'delete',
            'subject': str(subject),
            'predicate': str(predicate),
            'object': str(obj),
            'graph': str(graph) if graph else None
        }

        self.channel.basic_publish(
            exchange='graph_updates',
            routing_key='graph.delete',
            body=json.dumps(event),
            properties=pika.BasicProperties(
                delivery_mode=2,
                content_type='application/json'
            )
        )

    def close(self):
        self.connection.close()

# Usage
publisher = EventPublisher()

# Publish insert
publisher.publish_insert(
    NamedNode("http://example.org/entity1"),
    NamedNode("http://example.org/name"),
    Literal("New Entity")
)
```

```python
# event_subscriber.py - Subscribe to graph updates

import pika
import json
from pyoxigraph import Store, NamedNode, Quad, Literal, BlankNode

class EventSubscriber:
    """Subscribe to graph update events"""

    def __init__(self, store_path, rabbitmq_host='rabbitmq'):
        self.store = Store(store_path)

        self.connection = pika.BlockingConnection(
            pika.ConnectionParameters(host=rabbitmq_host)
        )
        self.channel = self.connection.channel()

        # Declare exchange
        self.channel.exchange_declare(
            exchange='graph_updates',
            exchange_type='topic',
            durable=True
        )

        # Create queue
        result = self.channel.queue_declare(queue='', exclusive=True)
        self.queue_name = result.method.queue

        # Bind to all graph updates
        self.channel.queue_bind(
            exchange='graph_updates',
            queue=self.queue_name,
            routing_key='graph.*'
        )

    def _parse_term(self, term_str):
        """Parse term from string representation"""
        if term_str.startswith('http://') or term_str.startswith('https://'):
            return NamedNode(term_str)
        elif term_str.startswith('_:'):
            return BlankNode(term_str[2:])
        else:
            return Literal(term_str)

    def handle_event(self, ch, method, properties, body):
        """Handle incoming event"""
        event = json.loads(body)

        try:
            subject = self._parse_term(event['subject'])
            predicate = NamedNode(event['predicate'])
            obj = self._parse_term(event['object'])

            quad = Quad(subject, predicate, obj)

            if event['type'] == 'insert':
                self.store.add(quad)
                print(f"Inserted: {quad}")

            elif event['type'] == 'delete':
                self.store.remove(quad)
                print(f"Deleted: {quad}")

            ch.basic_ack(delivery_tag=method.delivery_tag)

        except Exception as e:
            print(f"Error handling event: {e}")
            # Reject and requeue
            ch.basic_nack(delivery_tag=method.delivery_tag, requeue=True)

    def start(self):
        """Start consuming events"""
        self.channel.basic_consume(
            queue=self.queue_name,
            on_message_callback=self.handle_event
        )

        print("Waiting for graph update events...")
        self.channel.start_consuming()

# Usage
if __name__ == "__main__":
    subscriber = EventSubscriber("./local-store")
    subscriber.start()
```

## API Gateway Integration

### Kong API Gateway Configuration

```yaml
# kong.yml - Kong declarative configuration

_format_version: "3.0"

services:
  - name: oxigraph-service
    url: http://oxigraph:8080
    routes:
      - name: oxigraph-query
        paths:
          - /api/graph/query
        methods:
          - POST
        plugins:
          - name: rate-limiting
            config:
              minute: 100
              hour: 1000
          - name: cors
            config:
              origins:
                - "*"
              methods:
                - GET
                - POST
              headers:
                - Accept
                - Content-Type
          - name: request-transformer
            config:
              add:
                headers:
                  - "X-Service-Name: oxigraph"

      - name: oxigraph-update
        paths:
          - /api/graph/update
        methods:
          - POST
        plugins:
          - name: key-auth
            config: {}
          - name: rate-limiting
            config:
              minute: 10
              hour: 100

      - name: oxigraph-stats
        paths:
          - /api/graph/stats
        methods:
          - GET
        plugins:
          - name: response-ratelimiting
            config:
              limits:
                minute: 60

# Create API keys for authenticated endpoints
consumers:
  - username: service-a
    keyauth_credentials:
      - key: service-a-secret-key

  - username: service-b
    keyauth_credentials:
      - key: service-b-secret-key
```

### NGINX API Gateway

```nginx
# nginx.conf - NGINX as API gateway

upstream oxigraph_backend {
    least_conn;
    server oxigraph-1:8080 max_fails=3 fail_timeout=30s;
    server oxigraph-2:8080 max_fails=3 fail_timeout=30s;
    server oxigraph-3:8080 max_fails=3 fail_timeout=30s;
}

# Rate limiting
limit_req_zone $binary_remote_addr zone=query_limit:10m rate=10r/s;
limit_req_zone $binary_remote_addr zone=update_limit:10m rate=2r/s;

server {
    listen 80;
    server_name api.example.com;

    # Health check
    location /health {
        access_log off;
        proxy_pass http://oxigraph_backend/health;
    }

    # SPARQL query endpoint
    location /api/graph/query {
        limit_req zone=query_limit burst=20 nodelay;

        proxy_pass http://oxigraph_backend/query;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;

        # Timeout settings
        proxy_connect_timeout 30s;
        proxy_send_timeout 30s;
        proxy_read_timeout 60s;

        # CORS
        add_header Access-Control-Allow-Origin *;
        add_header Access-Control-Allow-Methods "POST, GET, OPTIONS";
        add_header Access-Control-Allow-Headers "Content-Type";

        if ($request_method = 'OPTIONS') {
            return 204;
        }
    }

    # SPARQL update endpoint (requires authentication)
    location /api/graph/update {
        limit_req zone=update_limit burst=5 nodelay;

        # Simple API key authentication
        if ($http_x_api_key = "") {
            return 401;
        }

        proxy_pass http://oxigraph_backend/update;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # Statistics endpoint
    location /api/graph/stats {
        proxy_pass http://oxigraph_backend/stats;
        proxy_cache stats_cache;
        proxy_cache_valid 200 5m;
    }
}
```

## Complete Docker Compose Example

```yaml
# docker-compose.yml - Complete microservices stack

version: '3.8'

services:
  # Service Discovery
  consul:
    image: consul:latest
    ports:
      - "8500:8500"
    command: agent -server -ui -node=server-1 -bootstrap-expect=1 -client=0.0.0.0

  # Message Queue
  rabbitmq:
    image: rabbitmq:3-management
    ports:
      - "5672:5672"
      - "15672:15672"
    environment:
      - RABBITMQ_DEFAULT_USER=admin
      - RABBITMQ_DEFAULT_PASS=admin

  # Oxigraph Services (3 instances for HA)
  oxigraph-1:
    build: ./oxigraph-service
    environment:
      - STORE_PATH=/data
      - BIND_ADDR=0.0.0.0:8080
      - CONSUL_ADDR=consul:8500
    volumes:
      - oxigraph-data-1:/data
    depends_on:
      - consul
      - rabbitmq
    restart: unless-stopped

  oxigraph-2:
    build: ./oxigraph-service
    environment:
      - STORE_PATH=/data
      - BIND_ADDR=0.0.0.0:8080
      - CONSUL_ADDR=consul:8500
    volumes:
      - oxigraph-data-2:/data
    depends_on:
      - consul
      - rabbitmq
    restart: unless-stopped

  oxigraph-3:
    build: ./oxigraph-service
    environment:
      - STORE_PATH=/data
      - BIND_ADDR=0.0.0.0:8080
      - CONSUL_ADDR=consul:8500
    volumes:
      - oxigraph-data-3:/data
    depends_on:
      - consul
      - rabbitmq
    restart: unless-stopped

  # Event Sync Service
  event-sync:
    build: ./event-sync
    environment:
      - RABBITMQ_HOST=rabbitmq
      - STORE_PATH=/data
    volumes:
      - sync-data:/data
    depends_on:
      - rabbitmq
    restart: unless-stopped

  # API Gateway
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
    depends_on:
      - oxigraph-1
      - oxigraph-2
      - oxigraph-3
    restart: unless-stopped

  # Application Services
  service-a:
    build: ./services/service-a
    environment:
      - CONSUL_HOST=consul
      - RABBITMQ_HOST=rabbitmq
    depends_on:
      - consul
      - rabbitmq
      - nginx
    restart: unless-stopped

  service-b:
    build: ./services/service-b
    environment:
      - CONSUL_HOST=consul
      - RABBITMQ_HOST=rabbitmq
    depends_on:
      - consul
      - rabbitmq
      - nginx
    restart: unless-stopped

  # Monitoring
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-data:/var/lib/grafana
    depends_on:
      - prometheus

volumes:
  oxigraph-data-1:
  oxigraph-data-2:
  oxigraph-data-3:
  sync-data:
  prometheus-data:
  grafana-data:
```

## Monitoring and Metrics

### Prometheus Metrics Exporter

```rust
// src/metrics.rs

use prometheus::{
    Counter, Histogram, HistogramOpts, IntGauge, Opts, Registry, TextEncoder, Encoder
};
use actix_web::{HttpResponse, web};

pub struct Metrics {
    pub query_counter: Counter,
    pub update_counter: Counter,
    pub query_duration: Histogram,
    pub triple_count: IntGauge,
    pub registry: Registry,
}

impl Metrics {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let registry = Registry::new();

        let query_counter = Counter::with_opts(
            Opts::new("oxigraph_queries_total", "Total SPARQL queries")
        )?;
        registry.register(Box::new(query_counter.clone()))?;

        let update_counter = Counter::with_opts(
            Opts::new("oxigraph_updates_total", "Total SPARQL updates")
        )?;
        registry.register(Box::new(update_counter.clone()))?;

        let query_duration = Histogram::with_opts(
            HistogramOpts::new(
                "oxigraph_query_duration_seconds",
                "SPARQL query duration"
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0])
        )?;
        registry.register(Box::new(query_duration.clone()))?;

        let triple_count = IntGauge::with_opts(
            Opts::new("oxigraph_triple_count", "Total triples in store")
        )?;
        registry.register(Box::new(triple_count.clone()))?;

        Ok(Metrics {
            query_counter,
            update_counter,
            query_duration,
            triple_count,
            registry,
        })
    }
}

pub async fn metrics_handler(metrics: web::Data<Metrics>) -> HttpResponse {
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();

    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(buffer)
}
```

### Prometheus Configuration

```yaml
# prometheus.yml

global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'oxigraph'
    consul_sd_configs:
      - server: 'consul:8500'
        services: ['oxigraph-service']
    relabel_configs:
      - source_labels: [__meta_consul_service]
        target_label: job
    metrics_path: /metrics

  - job_name: 'services'
    consul_sd_configs:
      - server: 'consul:8500'
    metrics_path: /metrics
```

## Deployment Commands

```bash
# Build and start all services
docker-compose up -d

# Scale Oxigraph instances
docker-compose up -d --scale oxigraph-service=5

# View logs
docker-compose logs -f oxigraph-1

# Check service health
curl http://localhost:8500/v1/health/service/oxigraph-service

# Query via API gateway
curl -X POST http://localhost/api/graph/query \
  -H "Content-Type: application/json" \
  -d '{
    "sparql": "SELECT * WHERE { ?s ?p ?o } LIMIT 10"
  }'

# Update via API gateway
curl -X POST http://localhost/api/graph/update \
  -H "Content-Type: application/json" \
  -H "X-API-Key: service-a-secret-key" \
  -d '{
    "sparql": "INSERT DATA { <http://example.org/s> <http://example.org/p> \"o\" }"
  }'

# View metrics
curl http://localhost:9090/metrics

# Access Grafana
open http://localhost:3000
```

## Testing the Architecture

### Load Testing

```python
# load_test.py

import asyncio
import aiohttp
import time
from statistics import mean, stdev

async def query_oxigraph(session, query_id):
    """Execute single query"""
    url = "http://localhost/api/graph/query"
    payload = {
        "sparql": "SELECT * WHERE { ?s ?p ?o } LIMIT 10"
    }

    start = time.time()
    async with session.post(url, json=payload) as response:
        await response.json()
        duration = time.time() - start
        return duration, response.status

async def load_test(num_queries=100, concurrency=10):
    """Run load test"""
    print(f"Running {num_queries} queries with concurrency {concurrency}")

    async with aiohttp.ClientSession() as session:
        tasks = []
        for i in range(num_queries):
            task = query_oxigraph(session, i)
            tasks.append(task)

        results = await asyncio.gather(*tasks)

    durations = [r[0] for r in results]
    statuses = [r[1] for r in results]

    print(f"\nResults:")
    print(f"  Total queries: {num_queries}")
    print(f"  Successful: {statuses.count(200)}")
    print(f"  Failed: {num_queries - statuses.count(200)}")
    print(f"  Average duration: {mean(durations):.3f}s")
    print(f"  Std deviation: {stdev(durations):.3f}s")
    print(f"  Min duration: {min(durations):.3f}s")
    print(f"  Max duration: {max(durations):.3f}s")

if __name__ == "__main__":
    asyncio.run(load_test(num_queries=1000, concurrency=50))
```

## Next Steps

- Review [Performance Tuning](../how-to/performance-tuning.md)
- Explore [Monitoring Best Practices](../how-to/monitoring.md)
- Check [Deployment Guide](../how-to/deployment.md)

## Additional Resources

- [Microservices Patterns](https://microservices.io/patterns/)
- [Service Mesh with Istio](https://istio.io/)
- [Kong API Gateway](https://konghq.com/)
- [Consul Service Discovery](https://www.consul.io/)
