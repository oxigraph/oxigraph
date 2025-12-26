# How to Run the Oxigraph SPARQL Server

This guide covers deploying and using the Oxigraph server for SPARQL queries and updates.

## Installation

### Using Cargo

```bash
cargo install oxigraph-cli
```

### Using Docker

```bash
docker pull ghcr.io/oxigraph/oxigraph:latest
```

### Using Conda

```bash
conda install -c conda-forge oxigraph-server
```

### Using UV (Python)

```bash
uvx oxigraph
```

### From Source

```bash
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph/cli
cargo build --release
```

The binary will be at `target/release/oxigraph`.

## Starting the Server

### Basic Server

```bash
# In-memory server (data lost on shutdown)
oxigraph serve

# Persistent server with data directory
oxigraph serve --location /path/to/data

# Custom host and port
oxigraph serve --location /path/to/data --bind 0.0.0.0:8080
```

### Server Options

```bash
oxigraph serve --help
```

Key options:
- `--location <PATH>`: Directory for persistent storage (optional)
- `--bind <ADDRESS>`: Bind address (default: `localhost:7878`)
- `--cors`: Enable CORS headers for cross-origin requests
- `--union-default-graph`: Treat default graph as union of all graphs
- `--timeout-s <SECONDS>`: Query timeout in seconds

### Read-Only Server

For read-only access to a database (allows multiple read-only instances):

```bash
oxigraph serve-read-only --location /path/to/data --bind localhost:7878
```

## Using Docker

### Basic Docker Deployment

```bash
# Create data directory
mkdir -p ./data

# Run server
docker run --rm \
  -v $PWD/data:/data \
  -p 7878:7878 \
  ghcr.io/oxigraph/oxigraph \
  serve --location /data --bind 0.0.0.0:7878
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: "3"
services:
  oxigraph:
    image: ghcr.io/oxigraph/oxigraph:latest
    command: serve --location /data --bind 0.0.0.0:7878
    volumes:
      - ./data:/data
    ports:
      - "7878:7878"
    restart: unless-stopped
```

Run with:

```bash
docker-compose up -d
```

### Docker with Authentication

Create `nginx.conf`:

```nginx
daemon off;
events {
    worker_connections 1024;
}
http {
    server {
        server_name localhost;
        listen 7878;
        proxy_ignore_client_abort on;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header Host $http_host;

        # Public read access
        location ~ ^(/|/query)$ {
            proxy_pass http://oxigraph:7878;
            proxy_pass_request_headers on;
        }

        # Authenticated write access
        location ~ ^(/update|/store)$ {
            auth_basic "Oxigraph Admin";
            auth_basic_user_file /etc/nginx/.htpasswd;
            proxy_pass http://oxigraph:7878;
            proxy_pass_request_headers on;
        }
    }
}
```

Create `docker-compose.yml` with nginx:

```yaml
version: "3"
services:
  oxigraph:
    image: ghcr.io/oxigraph/oxigraph:latest
    command: serve --location /data --bind 0.0.0.0:7878
    volumes:
      - ./data:/data

  nginx-auth:
    image: nginx:1.21
    environment:
      - OXIGRAPH_USER=admin
      - OXIGRAPH_PASSWORD=secret
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
    ports:
      - "7878:7878"
    entrypoint: >
      bash -c 'echo -n $$OXIGRAPH_USER: >> /etc/nginx/.htpasswd &&
      echo $$OXIGRAPH_PASSWORD | openssl passwd -stdin -apr1 >> /etc/nginx/.htpasswd &&
      /docker-entrypoint.sh nginx'
```

## Server Endpoints

The server exposes the following endpoints:

### Web Interface

```
GET http://localhost:7878/
```

Opens the YASGUI-based query interface in your browser.

### SPARQL Query Endpoint

```
POST /query
Content-Type: application/sparql-query

SELECT * WHERE { ?s ?p ?o } LIMIT 10
```

Example with curl:

```bash
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: application/sparql-results+json' \
  --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10'
```

Supported Accept types:
- `application/sparql-results+json` (SPARQL JSON)
- `application/sparql-results+xml` (SPARQL XML)
- `text/csv` (CSV)
- `text/tab-separated-values` (TSV)
- `text/turtle` (Turtle, for CONSTRUCT)
- `application/n-triples` (N-Triples, for CONSTRUCT)
- `application/rdf+xml` (RDF/XML, for CONSTRUCT)

### SPARQL Update Endpoint

```
POST /update
Content-Type: application/sparql-update

INSERT DATA {
  <http://example.com/s> <http://example.com/p> "value" .
}
```

Example with curl:

```bash
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data 'INSERT DATA { <http://example.com/s> <http://example.com/p> "value" }'
```

### Graph Store HTTP Protocol

#### Upload data to default graph

```bash
curl -X POST http://localhost:7878/store?default \
  -H 'Content-Type: text/turtle' \
  -T data.ttl
```

#### Upload data to named graph

```bash
curl -X POST "http://localhost:7878/store?graph=http://example.com/g" \
  -H 'Content-Type: text/turtle' \
  -T data.ttl
```

#### Upload dataset (N-Quads)

```bash
curl -X POST http://localhost:7878/store \
  -H 'Content-Type: application/n-quads' \
  -T dataset.nq
```

#### Download graph

```bash
# Get default graph
curl -H 'Accept: text/turtle' \
  http://localhost:7878/store?default > default.ttl

# Get named graph
curl -H 'Accept: text/turtle' \
  "http://localhost:7878/store?graph=http://example.com/g" > graph.ttl
```

#### Delete graph

```bash
curl -X DELETE "http://localhost:7878/store?graph=http://example.com/g"
```

## Configuration Options

### Enable CORS

For cross-origin requests from web applications:

```bash
oxigraph serve --location data --cors
```

This adds appropriate CORS headers to all responses.

### Union Default Graph

Make the default graph behave as the union of all named graphs:

```bash
oxigraph serve --location data --union-default-graph
```

### Query Timeout

Set a timeout for long-running queries:

```bash
oxigraph serve --location data --timeout-s 30
```

Queries exceeding 30 seconds will be cancelled.

### Bind Address

#### Listen on all interfaces

```bash
oxigraph serve --location data --bind 0.0.0.0:7878
```

#### Custom port

```bash
oxigraph serve --location data --bind localhost:8080
```

## Running as a System Service

### Systemd (Linux)

Create `/etc/systemd/system/oxigraph.service`:

```ini
[Unit]
Description=Oxigraph SPARQL Server
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
User=oxigraph
Group=oxigraph
ExecStart=/usr/local/bin/oxigraph serve --location /var/lib/oxigraph
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

Setup and start:

```bash
# Create user
sudo useradd -r -s /bin/false oxigraph

# Create data directory
sudo mkdir -p /var/lib/oxigraph
sudo chown oxigraph:oxigraph /var/lib/oxigraph

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable oxigraph
sudo systemctl start oxigraph

# Check status
sudo systemctl status oxigraph
```

View logs:

```bash
sudo journalctl -u oxigraph -f
```

### User-level Systemd

Create `~/.config/systemd/user/oxigraph.service`:

```ini
[Unit]
Description=Oxigraph SPARQL Server
After=network-online.target

[Service]
Type=notify
ExecStart=%h/.cargo/bin/oxigraph serve --location %h/oxigraph-data

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable oxigraph
systemctl --user start oxigraph
```

## Using the Server

### Query Examples

#### SELECT Query

```bash
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: application/sparql-results+json' \
  --data '
PREFIX ex: <http://example.com/>
SELECT ?name WHERE {
  ?person a ex:Person ;
          ex:name ?name .
}'
```

#### CONSTRUCT Query

```bash
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: text/turtle' \
  --data '
PREFIX ex: <http://example.com/>
CONSTRUCT { ?s ex:label ?name }
WHERE { ?s ex:name ?name }'
```

#### ASK Query

```bash
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  --data 'ASK { ?s ?p ?o }'
```

### Update Examples

#### INSERT DATA

```bash
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data '
PREFIX ex: <http://example.com/>
INSERT DATA {
  ex:Alice a ex:Person ;
           ex:name "Alice" ;
           ex:age 30 .
}'
```

#### DELETE/INSERT

```bash
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data '
PREFIX ex: <http://example.com/>
DELETE { ?person ex:age ?old }
INSERT { ?person ex:age 31 }
WHERE {
  ?person ex:name "Alice" ;
          ex:age ?old .
}'
```

#### LOAD from URL

```bash
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data 'LOAD <http://example.com/data.ttl>'
```

## Monitoring and Maintenance

### Health Check

```bash
# Simple query to check if server is running
curl http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  --data 'ASK { ?s ?p ?o }'
```

### Backup

```bash
# Stop server first (or use read-only backup)
systemctl stop oxigraph

# Create backup
oxigraph backup --location /var/lib/oxigraph \
  --destination /backup/oxigraph-$(date +%Y%m%d)

# Restart server
systemctl start oxigraph
```

For live backup, use a read-only connection:

```bash
oxigraph backup --location /var/lib/oxigraph \
  --destination /backup/oxigraph-$(date +%Y%m%d)
```

### Database Optimization

After bulk loading, optimize the database:

```bash
# This is automatically suggested after loading
oxigraph optimize --location /var/lib/oxigraph
```

## Reverse Proxy Setup

### Nginx

```nginx
server {
    listen 80;
    server_name sparql.example.com;

    location / {
        proxy_pass http://localhost:7878;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Increase timeout for long queries
        proxy_read_timeout 300s;
    }
}
```

### Apache

```apache
<VirtualHost *:80>
    ServerName sparql.example.com

    ProxyPreserveHost On
    ProxyPass / http://localhost:7878/
    ProxyPassReverse / http://localhost:7878/

    # Increase timeout for long queries
    ProxyTimeout 300
</VirtualHost>
```

## Performance Tuning

### Bulk Loading Before Starting

For best performance, load data before starting the server:

```bash
# Load data
oxigraph load --location data --file large-dataset.nq

# Then start server
oxigraph serve --location data
```

### Read-Only Replicas

Run multiple read-only servers for load balancing:

```bash
# Primary read-write server
oxigraph serve --location data --bind localhost:7878

# Read-only replica 1
oxigraph serve-read-only --location data --bind localhost:7879

# Read-only replica 2
oxigraph serve-read-only --location data --bind localhost:7880
```

### Resource Limits

Set resource limits in systemd:

```ini
[Service]
MemoryMax=4G
CPUQuota=200%
```

## Troubleshooting

### Server won't start

Check if port is already in use:

```bash
lsof -i :7878
```

Use a different port:

```bash
oxigraph serve --location data --bind localhost:8080
```

### Permission denied

Ensure the data directory is writable:

```bash
chmod 755 /path/to/data
```

### Out of memory

Reduce query timeout or limit concurrent queries using a reverse proxy.

## Next Steps

- Learn how to [import data](import-rdf-data.md)
- Learn how to [export data](export-rdf-data.md)
- Optimize your setup with [performance tips](optimize-performance.md)
