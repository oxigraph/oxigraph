# Deployment Troubleshooting

This guide helps resolve deployment issues for Oxigraph server including Docker, Kubernetes, proxy configuration, SSL/TLS, and health checks.

## Table of Contents

- [Docker Issues](#docker-issues)
- [Kubernetes Troubleshooting](#kubernetes-troubleshooting)
- [Proxy Configuration](#proxy-configuration)
- [SSL/TLS Setup Problems](#ssltls-setup-problems)
- [Health Check Configuration](#health-check-configuration)
- [Resource Limits](#resource-limits)
- [Backup and Recovery](#backup-and-recovery)

---

## Docker Issues

### Container Starts Then Exits

**Symptom:**
```bash
$ docker ps -a
CONTAINER ID   STATUS
abc123def456   Exited (1) 5 seconds ago
```

**Cause:**
- Incorrect command or entrypoint
- Missing data directory
- Permission issues

**Solution:**

```bash
# Check container logs
docker logs abc123def456

# Common errors and fixes:

# Error: Permission denied on /data
# Fix: Ensure volume permissions
docker run -d \
  -v oxigraph-data:/data \
  --user $(id -u):$(id -g) \
  oxigraph/oxigraph serve --location /data

# Error: No such file or directory
# Fix: Create volume first
docker volume create oxigraph-data
docker run -d \
  -v oxigraph-data:/data \
  -p 7878:7878 \
  oxigraph/oxigraph serve --location /data

# Error: Address already in use
# Fix: Use different port
docker run -d \
  -v oxigraph-data:/data \
  -p 8080:7878 \
  oxigraph/oxigraph serve --location /data --bind 0.0.0.0:7878
```

**Prevention:**
- Always check logs with `docker logs`
- Use named volumes for persistence
- Test locally before deploying
- Use healthchecks

---

### Data Persistence Issues

**Symptom:**
Data disappears when container restarts.

**Cause:**
No volume mounted, or volume mounted to wrong path.

**Solution:**

```dockerfile
# Dockerfile - proper volume setup
FROM oxigraph/oxigraph:latest

# Create data directory
RUN mkdir -p /data && chown -R 1000:1000 /data

# Declare volume
VOLUME ["/data"]

# Expose port
EXPOSE 7878

# Run server
CMD ["serve", "--location", "/data", "--bind", "0.0.0.0:7878"]
```

```yaml
# docker-compose.yml
version: '3.8'

services:
  oxigraph:
    image: oxigraph/oxigraph:latest
    command: serve --location /data --bind 0.0.0.0:7878
    ports:
      - "7878:7878"
    volumes:
      # Named volume (recommended)
      - oxigraph-data:/data
      # Or bind mount
      # - ./data:/data
    environment:
      - RUST_LOG=info
    restart: unless-stopped

volumes:
  oxigraph-data:
    driver: local
```

```bash
# Verify volume is mounted
docker inspect abc123def456 | jq '.[0].Mounts'

# Expected output:
# [
#   {
#     "Type": "volume",
#     "Name": "oxigraph-data",
#     "Source": "/var/lib/docker/volumes/oxigraph-data/_data",
#     "Destination": "/data",
#     "Driver": "local",
#     "Mode": "z",
#     "RW": true,
#     "Propagation": ""
#   }
# ]
```

**Prevention:**
- Always use volumes for production
- Test container restart with `docker restart`
- Backup volumes regularly
- Document volume paths

---

### Memory Limit Issues in Docker

**Symptom:**
Container killed by OOM (Out of Memory).

**Cause:**
Docker memory limit too low for workload.

**Solution:**

```bash
# Set memory limit
docker run -d \
  -m 4g \
  --memory-swap 8g \
  -v oxigraph-data:/data \
  -p 7878:7878 \
  oxigraph/oxigraph serve --location /data

# Monitor memory usage
docker stats abc123def456

# Output:
# CONTAINER ID   NAME      CPU %   MEM USAGE / LIMIT
# abc123def456   oxigraph  45.3%   2.1GiB / 4GiB
```

```yaml
# docker-compose.yml - with memory limits
services:
  oxigraph:
    image: oxigraph/oxigraph:latest
    command: serve --location /data --bind 0.0.0.0:7878
    deploy:
      resources:
        limits:
          memory: 4G
        reservations:
          memory: 2G
    volumes:
      - oxigraph-data:/data
```

**Prevention:**
- Set memory limits based on dataset size (estimate 5-10x raw data)
- Monitor memory usage in production
- Use swap for temporary spikes
- Implement memory alerts

---

### Building Custom Docker Image

**Symptom:**
Need custom configuration or features.

**Solution:**

```dockerfile
# Dockerfile.custom
FROM rust:1.75 as builder

WORKDIR /build

# Copy source
COPY . .

# Build with custom features
RUN cargo build --release -p oxigraph-cli --features geosparql

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /build/target/release/oxigraph /usr/local/bin/

# Create user
RUN useradd -m -u 1000 oxigraph && \
    mkdir -p /data && \
    chown oxigraph:oxigraph /data

USER oxigraph
VOLUME ["/data"]
EXPOSE 7878

ENTRYPOINT ["/usr/local/bin/oxigraph"]
CMD ["serve", "--location", "/data", "--bind", "0.0.0.0:7878"]
```

```bash
# Build
docker build -f Dockerfile.custom -t my-oxigraph:latest .

# Run
docker run -d \
  -v oxigraph-data:/data \
  -p 7878:7878 \
  my-oxigraph:latest
```

**Prevention:**
- Document custom build process
- Version tag images properly
- Test before deploying

---

## Kubernetes Troubleshooting

### Pod CrashLoopBackOff

**Symptom:**
```bash
$ kubectl get pods
NAME                        READY   STATUS             RESTARTS
oxigraph-7d5f9c8b6d-abc12   0/1     CrashLoopBackOff   5
```

**Cause:**
- Application error
- Misconfigured liveness/readiness probes
- Volume mount issues

**Solution:**

```bash
# Check pod logs
kubectl logs oxigraph-7d5f9c8b6d-abc12

# Check previous pod logs (if restarted)
kubectl logs oxigraph-7d5f9c8b6d-abc12 --previous

# Describe pod for events
kubectl describe pod oxigraph-7d5f9c8b6d-abc12

# Common issues and fixes:

# Issue: Volume mount failed
# Check PVC
kubectl get pvc
kubectl describe pvc oxigraph-data

# Issue: Liveness probe failing too quickly
# Fix: Increase initialDelaySeconds
```

```yaml
# deployment.yaml - fixed probes
apiVersion: apps/v1
kind: Deployment
metadata:
  name: oxigraph
spec:
  replicas: 1
  selector:
    matchLabels:
      app: oxigraph
  template:
    metadata:
      labels:
        app: oxigraph
    spec:
      containers:
      - name: oxigraph
        image: oxigraph/oxigraph:latest
        args: ["serve", "--location", "/data", "--bind", "0.0.0.0:7878"]
        ports:
        - containerPort: 7878
        volumeMounts:
        - name: data
          mountPath: /data
        resources:
          requests:
            memory: "2Gi"
            cpu: "500m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /
            port: 7878
          initialDelaySeconds: 60  # Wait for startup
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /
            port: 7878
          initialDelaySeconds: 30
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: oxigraph-data
```

**Prevention:**
- Proper probe configuration
- Adequate resource limits
- Test deployments in staging

---

### Persistent Volume Issues

**Symptom:**
```
FailedMount: MountVolume.SetUp failed for volume "oxigraph-data"
```

**Cause:**
- PVC not bound to PV
- Storage class not available
- Node cannot access volume

**Solution:**

```bash
# Check PVC status
kubectl get pvc oxigraph-data

# Should show:
# NAME            STATUS   VOLUME                                     CAPACITY
# oxigraph-data   Bound    pvc-abc-123                               10Gi

# If Pending, describe for details
kubectl describe pvc oxigraph-data

# Create PVC
```

```yaml
# pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: oxigraph-data
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
  storageClassName: fast-ssd  # Adjust to your cluster
```

```bash
# Apply
kubectl apply -f pvc.yaml

# Verify binding
kubectl get pvc oxigraph-data
```

**Prevention:**
- Verify storage class exists: `kubectl get storageclass`
- Use appropriate access mode for workload
- Monitor PV/PVC status

---

### Service Not Accessible

**Symptom:**
Cannot reach Oxigraph service from outside cluster.

**Cause:**
- Service type not configured correctly
- Network policies blocking traffic
- Ingress not set up

**Solution:**

```yaml
# service.yaml - LoadBalancer (cloud)
apiVersion: v1
kind: Service
metadata:
  name: oxigraph
spec:
  type: LoadBalancer
  selector:
    app: oxigraph
  ports:
  - port: 80
    targetPort: 7878
    protocol: TCP
```

```yaml
# service.yaml - NodePort (on-prem)
apiVersion: v1
kind: Service
metadata:
  name: oxigraph
spec:
  type: NodePort
  selector:
    app: oxigraph
  ports:
  - port: 7878
    targetPort: 7878
    nodePort: 30878  # 30000-32767
```

```yaml
# ingress.yaml - with Ingress controller
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: oxigraph
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  rules:
  - host: oxigraph.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: oxigraph
            port:
              number: 7878
```

```bash
# Test service internally
kubectl run -it --rm debug --image=curlimages/curl --restart=Never -- \
  curl http://oxigraph:7878

# Check endpoints
kubectl get endpoints oxigraph
```

**Prevention:**
- Document service access method
- Use Ingress for production HTTP(S)
- Test connectivity from different namespaces

---

### ConfigMap and Secret Issues

**Symptom:**
Configuration not loaded or authentication failing.

**Solution:**

```yaml
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: oxigraph-config
data:
  RUST_LOG: "info"
  SERVER_BIND: "0.0.0.0:7878"
```

```yaml
# secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: oxigraph-secret
type: Opaque
stringData:
  admin-password: "change-me-in-production"
```

```yaml
# deployment.yaml - using ConfigMap and Secret
spec:
  containers:
  - name: oxigraph
    envFrom:
    - configMapRef:
        name: oxigraph-config
    env:
    - name: ADMIN_PASSWORD
      valueFrom:
        secretKeyRef:
          name: oxigraph-secret
          key: admin-password
```

```bash
# Verify ConfigMap/Secret
kubectl get configmap oxigraph-config -o yaml
kubectl get secret oxigraph-secret -o yaml

# Update without restart (requires proper configuration)
kubectl create configmap oxigraph-config --from-literal=RUST_LOG=debug --dry-run=client -o yaml | kubectl apply -f -
```

**Prevention:**
- Use ConfigMaps for non-sensitive config
- Use Secrets for sensitive data
- Version control config files (not secret values)

---

## Proxy Configuration

### Reverse Proxy Setup (Nginx)

**Symptom:**
Need to expose Oxigraph behind Nginx for SSL termination or path-based routing.

**Solution:**

```nginx
# /etc/nginx/sites-available/oxigraph
upstream oxigraph {
    server localhost:7878;
    # Or for multiple instances
    # server oxigraph-1:7878;
    # server oxigraph-2:7878;
}

server {
    listen 80;
    server_name oxigraph.example.com;

    # Redirect to HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name oxigraph.example.com;

    # SSL configuration
    ssl_certificate /etc/ssl/certs/oxigraph.crt;
    ssl_certificate_key /etc/ssl/private/oxigraph.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # Increase timeouts for long queries
    proxy_read_timeout 300s;
    proxy_connect_timeout 300s;
    proxy_send_timeout 300s;

    # Increase body size for large RDF uploads
    client_max_body_size 100M;

    location / {
        proxy_pass http://oxigraph;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # CORS headers (if needed)
        add_header Access-Control-Allow-Origin *;
        add_header Access-Control-Allow-Methods "GET, POST, PUT, DELETE, OPTIONS";
        add_header Access-Control-Allow-Headers "Content-Type, Authorization";

        # Handle OPTIONS requests
        if ($request_method = OPTIONS) {
            return 204;
        }
    }

    # Specific location for SPARQL endpoint
    location /sparql {
        proxy_pass http://oxigraph/query;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # Cache SPARQL results (optional, use with caution)
        # proxy_cache sparql_cache;
        # proxy_cache_valid 200 5m;
    }
}
```

```bash
# Enable site
sudo ln -s /etc/nginx/sites-available/oxigraph /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx

# Test
curl -I https://oxigraph.example.com
```

**Prevention:**
- Monitor proxy logs: `/var/log/nginx/access.log`
- Set appropriate timeouts
- Enable rate limiting for public endpoints

---

### Apache Reverse Proxy

**Symptom:**
Using Apache instead of Nginx.

**Solution:**

```apache
# /etc/apache2/sites-available/oxigraph.conf
<VirtualHost *:80>
    ServerName oxigraph.example.com
    Redirect permanent / https://oxigraph.example.com/
</VirtualHost>

<VirtualHost *:443>
    ServerName oxigraph.example.com

    # SSL configuration
    SSLEngine on
    SSLCertificateFile /etc/ssl/certs/oxigraph.crt
    SSLCertificateKeyFile /etc/ssl/private/oxigraph.key

    # Proxy settings
    ProxyPreserveHost On
    ProxyTimeout 300

    # Increase upload size
    LimitRequestBody 104857600  # 100MB

    ProxyPass / http://localhost:7878/
    ProxyPassReverse / http://localhost:7878/

    # Error handling
    ErrorLog ${APACHE_LOG_DIR}/oxigraph_error.log
    CustomLog ${APACHE_LOG_DIR}/oxigraph_access.log combined
</VirtualHost>
```

```bash
# Enable modules
sudo a2enmod proxy proxy_http ssl
sudo a2ensite oxigraph
sudo apache2ctl configtest
sudo systemctl reload apache2
```

**Prevention:**
- Monitor Apache logs
- Keep Apache updated
- Configure proper timeouts

---

### Proxy Header Issues

**Symptom:**
Application sees proxy IP instead of client IP, or incorrect protocol (HTTP vs HTTPS).

**Cause:**
Missing or incorrect proxy headers.

**Solution:**

```nginx
# Nginx - proper headers
location / {
    proxy_pass http://oxigraph;

    # Essential headers
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Host $host;
    proxy_set_header X-Forwarded-Port $server_port;
}
```

**Prevention:**
- Always set proxy headers
- Log headers for debugging: `add_header X-Debug-Host $host;`
- Document header requirements

---

## SSL/TLS Setup Problems

### Self-Signed Certificate for Testing

**Symptom:**
Need SSL for testing but don't have certificate.

**Solution:**

```bash
# Generate self-signed certificate
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/ssl/private/oxigraph-selfsigned.key \
  -out /etc/ssl/certs/oxigraph-selfsigned.crt \
  -subj "/C=US/ST=State/L=City/O=Organization/CN=oxigraph.local"

# Set permissions
chmod 600 /etc/ssl/private/oxigraph-selfsigned.key
chmod 644 /etc/ssl/certs/oxigraph-selfsigned.crt

# Use in Nginx
ssl_certificate /etc/ssl/certs/oxigraph-selfsigned.crt;
ssl_certificate_key /etc/ssl/private/oxigraph-selfsigned.key;
```

**Prevention:**
- Use Let's Encrypt for production
- Never commit private keys to version control
- Rotate certificates before expiration

---

### Let's Encrypt with Certbot

**Symptom:**
Need free, valid SSL certificate for production.

**Solution:**

```bash
# Install Certbot
sudo apt-get update
sudo apt-get install certbot python3-certbot-nginx

# Obtain certificate (Nginx)
sudo certbot --nginx -d oxigraph.example.com

# Or Apache
# sudo certbot --apache -d oxigraph.example.com

# Test renewal
sudo certbot renew --dry-run

# Auto-renewal is set up via systemd timer
systemctl status certbot.timer
```

**Prevention:**
- Monitor certificate expiration
- Test renewal process
- Set up alerts for renewal failures

---

### Certificate Chain Issues

**Symptom:**
```
SSL certificate problem: unable to get local issuer certificate
```

**Cause:**
Incomplete certificate chain or missing intermediate certificates.

**Solution:**

```bash
# Check certificate chain
openssl s_client -connect oxigraph.example.com:443 -showcerts

# Verify certificate
openssl verify -CAfile /etc/ssl/certs/ca-certificates.crt \
  /etc/ssl/certs/oxigraph.crt

# Fix: Include full chain
cat oxigraph.crt intermediate.crt root.crt > oxigraph-fullchain.crt

# Use full chain in Nginx
ssl_certificate /etc/ssl/certs/oxigraph-fullchain.crt;
```

**Prevention:**
- Always use full certificate chain
- Test with SSL Labs: https://www.ssllabs.com/ssltest/
- Keep CA certificates updated

---

## Health Check Configuration

### Basic HTTP Health Check

**Symptom:**
Need to verify Oxigraph is healthy for load balancers or orchestrators.

**Solution:**

```bash
# Simple health check
curl -f http://localhost:7878/ || exit 1

# More detailed check
curl -f -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -d 'ASK { ?s ?p ?o }' || exit 1
```

```yaml
# Docker Compose healthcheck
services:
  oxigraph:
    image: oxigraph/oxigraph:latest
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:7878/"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

```yaml
# Kubernetes probes
livenessProbe:
  httpGet:
    path: /
    port: 7878
  initialDelaySeconds: 60
  periodSeconds: 10
  timeoutSeconds: 5
  failureThreshold: 3

readinessProbe:
  httpGet:
    path: /
    port: 7878
  initialDelaySeconds: 30
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 3
```

**Prevention:**
- Use appropriate initial delay for startup time
- Monitor health check failures
- Adjust thresholds based on SLA

---

### Custom Health Check Script

**Symptom:**
Need more sophisticated health checks.

**Solution:**

```bash
#!/bin/bash
# /usr/local/bin/oxigraph-health-check.sh

set -e

# Configuration
OXIGRAPH_URL="${OXIGRAPH_URL:-http://localhost:7878}"
TIMEOUT=5

# Check 1: HTTP connectivity
if ! curl -sf --max-time $TIMEOUT "$OXIGRAPH_URL/" > /dev/null; then
    echo "CRITICAL: Cannot connect to Oxigraph"
    exit 2
fi

# Check 2: SPARQL endpoint responds
QUERY='ASK { ?s ?p ?o }'
if ! curl -sf --max-time $TIMEOUT \
     -X POST "$OXIGRAPH_URL/query" \
     -H 'Content-Type: application/sparql-query' \
     -d "$QUERY" > /dev/null; then
    echo "CRITICAL: SPARQL endpoint not responding"
    exit 2
fi

# Check 3: Store is not empty (optional)
COUNT_QUERY='SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o } LIMIT 1'
RESULT=$(curl -sf --max-time $TIMEOUT \
    -X POST "$OXIGRAPH_URL/query" \
    -H 'Accept: application/sparql-results+json' \
    -H 'Content-Type: application/sparql-query' \
    -d "$COUNT_QUERY")

if [ -z "$RESULT" ]; then
    echo "WARNING: Could not verify store contents"
    exit 1
fi

echo "OK: Oxigraph is healthy"
exit 0
```

```bash
# Make executable
chmod +x /usr/local/bin/oxigraph-health-check.sh

# Test
/usr/local/bin/oxigraph-health-check.sh
```

**Prevention:**
- Keep health checks lightweight
- Don't perform expensive operations
- Log health check failures

---

## Resource Limits

### Setting Systemd Resource Limits

**Symptom:**
Need to limit resources for Oxigraph service on Linux.

**Solution:**

```ini
# /etc/systemd/system/oxigraph.service
[Unit]
Description=Oxigraph SPARQL Database
After=network.target

[Service]
Type=simple
User=oxigraph
Group=oxigraph
WorkingDirectory=/var/lib/oxigraph
ExecStart=/usr/local/bin/oxigraph serve --location /var/lib/oxigraph/data --bind 0.0.0.0:7878

# Resource limits
MemoryMax=4G
MemoryHigh=3G
CPUQuota=200%  # 2 cores
TasksMax=4096
LimitNOFILE=65536

# Restart policy
Restart=on-failure
RestartSec=10s

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=oxigraph

[Install]
WantedBy=multi-user.target
```

```bash
# Reload systemd
sudo systemctl daemon-reload

# Start service
sudo systemctl start oxigraph

# Check status and limits
systemctl status oxigraph
systemctl show oxigraph | grep -E '(Memory|CPU|Tasks)'

# Monitor resource usage
journalctl -u oxigraph -f
```

**Prevention:**
- Set limits based on available resources
- Monitor actual usage
- Adjust limits as needed

---

## Backup and Recovery

### Automated Backup Strategy

**Symptom:**
Need reliable backups for disaster recovery.

**Solution:**

```bash
#!/bin/bash
# /usr/local/bin/oxigraph-backup.sh

set -e

# Configuration
OXIGRAPH_URL="http://localhost:7878"
BACKUP_DIR="/backups/oxigraph"
DATA_DIR="/var/lib/oxigraph/data"
RETENTION_DAYS=7

# Create backup directory
BACKUP_DATE=$(date +%Y%m%d-%H%M%S)
BACKUP_PATH="$BACKUP_DIR/$BACKUP_DATE"
mkdir -p "$BACKUP_PATH"

echo "Starting Oxigraph backup at $BACKUP_DATE"

# Method 1: Export to N-Quads (slower but safer, no downtime)
echo "Exporting data..."
curl -f -X GET "$OXIGRAPH_URL/dump" \
  -H 'Accept: application/n-quads' \
  -o "$BACKUP_PATH/dump.nq.gz" \
  --compressed

# Method 2: Filesystem backup (faster, requires brief read-only mode)
# echo "Creating filesystem backup..."
# cp -r "$DATA_DIR" "$BACKUP_PATH/data"

# Verify backup
echo "Verifying backup..."
if [ ! -s "$BACKUP_PATH/dump.nq.gz" ]; then
    echo "ERROR: Backup file is empty!"
    exit 1
fi

# Cleanup old backups
echo "Cleaning up old backups..."
find "$BACKUP_DIR" -type d -mtime +$RETENTION_DAYS -exec rm -rf {} +

# Calculate size
BACKUP_SIZE=$(du -sh "$BACKUP_PATH" | cut -f1)
echo "Backup completed: $BACKUP_SIZE at $BACKUP_PATH"

# Optional: Upload to S3
# aws s3 sync "$BACKUP_PATH" "s3://my-bucket/oxigraph-backups/$BACKUP_DATE/"

echo "Backup successful!"
```

```bash
# Make executable
chmod +x /usr/local/bin/oxigraph-backup.sh

# Test
/usr/local/bin/oxigraph-backup.sh

# Schedule with cron (daily at 2 AM)
echo "0 2 * * * /usr/local/bin/oxigraph-backup.sh >> /var/log/oxigraph-backup.log 2>&1" | crontab -
```

**Prevention:**
- Test backups regularly
- Store backups off-site
- Monitor backup job status
- Document recovery procedures

---

### Restore from Backup

**Symptom:**
Need to restore from backup after data loss.

**Solution:**

```bash
#!/bin/bash
# restore-oxigraph.sh

set -e

BACKUP_FILE="$1"
DATA_DIR="/var/lib/oxigraph/data"

if [ -z "$BACKUP_FILE" ]; then
    echo "Usage: $0 <backup-file.nq.gz>"
    exit 1
fi

echo "WARNING: This will replace all data in $DATA_DIR"
read -p "Continue? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "Aborted"
    exit 0
fi

# Stop Oxigraph
echo "Stopping Oxigraph..."
sudo systemctl stop oxigraph

# Backup current data (just in case)
echo "Backing up current data..."
sudo mv "$DATA_DIR" "$DATA_DIR.pre-restore-$(date +%Y%m%d-%H%M%S)"

# Create fresh store
echo "Creating new store..."
sudo mkdir -p "$DATA_DIR"
sudo chown oxigraph:oxigraph "$DATA_DIR"

# Restore data
echo "Restoring from $BACKUP_FILE..."
sudo -u oxigraph oxigraph load \
  --location "$DATA_DIR" \
  --file <(zcat "$BACKUP_FILE")

# Start Oxigraph
echo "Starting Oxigraph..."
sudo systemctl start oxigraph

# Verify
sleep 5
if systemctl is-active --quiet oxigraph; then
    echo "Restore successful!"
else
    echo "ERROR: Oxigraph failed to start"
    systemctl status oxigraph
    exit 1
fi
```

**Prevention:**
- Test restore procedure regularly (at least quarterly)
- Document step-by-step recovery process
- Keep multiple backup versions
- Calculate RTO (Recovery Time Objective)

---

## Monitoring and Alerting

### Prometheus Metrics (Future Feature)

**Note:** Oxigraph doesn't currently expose Prometheus metrics by default. This shows how you might implement custom metrics.

```bash
# Custom metrics exporter script
# /usr/local/bin/oxigraph-metrics.sh

#!/bin/bash
OXIGRAPH_URL="http://localhost:7878"
METRICS_PORT=9090

# Count triples
TRIPLE_COUNT=$(curl -sf -X POST "$OXIGRAPH_URL/query" \
  -H 'Accept: application/sparql-results+json' \
  -H 'Content-Type: application/sparql-query' \
  -d 'SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o }' | \
  jq -r '.results.bindings[0].count.value')

# Expose metrics
cat <<EOF | nc -l -p $METRICS_PORT -q 1
HTTP/1.1 200 OK
Content-Type: text/plain

# HELP oxigraph_triples_total Total number of triples
# TYPE oxigraph_triples_total gauge
oxigraph_triples_total $TRIPLE_COUNT

# HELP oxigraph_up Oxigraph is up
# TYPE oxigraph_up gauge
oxigraph_up 1
EOF
```

**Prevention:**
- Set up monitoring before production
- Define SLOs (Service Level Objectives)
- Configure alerts for anomalies
- Review metrics regularly

---

## Quick Deployment Checklist

Before deploying to production:

- [ ] Persistent storage configured (volume/PVC)
- [ ] Resource limits set (memory, CPU)
- [ ] Health checks configured
- [ ] SSL/TLS certificate set up
- [ ] Backup strategy implemented and tested
- [ ] Monitoring and alerting configured
- [ ] Logs aggregated and rotated
- [ ] Security hardening (firewall, authentication)
- [ ] Documentation for operations team
- [ ] Disaster recovery plan tested
- [ ] Load testing completed
- [ ] High availability strategy (if needed)

---

**Still having deployment issues?** See the [troubleshooting index](index.md) or provide:
1. Deployment platform (Docker/Kubernetes/bare metal)
2. Configuration files (redact secrets)
3. Error logs
4. Environment details (OS, versions)
5. What you've tried so far
