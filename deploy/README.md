# Hadrian Gateway Deployment

This directory contains Docker Compose configurations for deploying Hadrian Gateway in various environments, from simple development setups to production-grade architectures.

## Quick Start

```bash
# Copy and configure environment variables
cp ../.env.example .env
# Edit .env with your API keys and passwords

# Development (SQLite)
docker compose -f docker-compose.sqlite.yml up -d

# Development with caching (SQLite + Redis)
docker compose -f docker-compose.sqlite-redis.yml up -d

# Production (PostgreSQL + Redis)
docker compose -f docker-compose.postgres.yml up -d
```

## Available Configurations

| Configuration | Use Case | Components |
|---------------|----------|------------|
| `sqlite.yml` | Development, single-user | SQLite |
| `sqlite-redis.yml` | Development with caching | SQLite, Redis |
| `postgres.yml` | Production, single-node | PostgreSQL, Redis |
| `vault.yml` | Secret management | PostgreSQL, Redis, HashiCorp Vault |
| `observability.yml` | Full monitoring stack | SQLite, Redis, OTEL, Prometheus, Grafana, Jaeger, Loki |
| `keycloak.yml` | Enterprise authentication | SQLite, Redis, Keycloak, OAuth2 Proxy |
| `traefik.yml` | Load-balanced gateway | PostgreSQL, Redis, Traefik, 3× Gateway |
| `postgres-ha.yml` | Database high availability | PostgreSQL Primary + 2 Replicas, PgBouncer |
| `redis-cluster.yml` | Cache high availability | SQLite, 6-node Redis Cluster |
| `dlq.yml` | Async processing & retries | SQLite, Redis, RabbitMQ (or NATS) |
| `production.yml` | Full production reference | All components combined |

## Configuration Details

### Development: SQLite (`docker-compose.sqlite.yml`)

Simplest setup for local development and testing.

```bash
docker compose -f docker-compose.sqlite.yml up -d

# Gateway available at http://localhost:8080
```

### Production: PostgreSQL (`docker-compose.postgres.yml`)

Standard production deployment with PostgreSQL and Redis.

```bash
docker compose -f docker-compose.postgres.yml up -d

# Gateway: http://localhost:8080
# PostgreSQL: localhost:5432
# Redis: localhost:6379
```

### Secret Management: Vault (`docker-compose.vault.yml`)

Uses HashiCorp Vault for secure secret storage with Vault Agent for automatic secret injection.

```bash
# 1. Start Vault
docker compose -f docker-compose.vault.yml up vault -d

# 2. Initialize Vault (save the unseal keys and root token!)
docker exec hadrian-vault vault operator init

# 3. Unseal Vault (repeat with 3 different keys)
docker exec hadrian-vault vault operator unseal <key-1>
docker exec hadrian-vault vault operator unseal <key-2>
docker exec hadrian-vault vault operator unseal <key-3>

# 4. Login and configure
export VAULT_ADDR=http://localhost:8200
vault login <root-token>

# 5. Enable KV secrets engine and store secrets
vault secrets enable -path=secret kv-v2
vault kv put secret/gateway \
  openrouter_api_key="sk-or-..." \
  anthropic_api_key="sk-ant-..." \
  openai_api_key="sk-..."

# 6. Create AppRole for gateway (see Vault docs)
# 7. Start all services
docker compose -f docker-compose.vault.yml up -d
```

### Observability (`docker-compose.observability.yml`)

Full monitoring, tracing, and logging stack.

```bash
docker compose -f docker-compose.observability.yml up -d

# Endpoints:
# - Gateway: http://localhost:8080
# - Grafana: http://localhost:3001 (admin/admin)
# - Prometheus: http://localhost:9090
# - Jaeger UI: http://localhost:16686
# - Alertmanager: http://localhost:9093
```

**Features:**
- OpenTelemetry Collector for unified telemetry pipeline
- Prometheus for metrics with pre-configured alerting rules
- Grafana with auto-provisioned datasources
- Jaeger for distributed tracing
- Loki + Promtail for log aggregation
- Alertmanager for alert routing (Slack, PagerDuty, email)

### Authentication: Keycloak (`docker-compose.keycloak.yml`)

Enterprise identity management with OIDC/OAuth2.

```bash
docker compose -f docker-compose.keycloak.yml up -d

# Endpoints:
# - Gateway: http://localhost:8080 (Note: conflicts with Keycloak, use different port in production)
# - Keycloak Admin: http://localhost:8080 (admin/admin)
# - OIDC Discovery: http://localhost:8080/realms/hadrian/.well-known/openid-configuration
```

**Setup:**
1. Access Keycloak admin console
2. Create realm "hadrian"
3. Create client "hadrian-gateway" with confidential access type
4. Configure client credentials in `.env`

**Optional OAuth2 Proxy:**
```bash
docker compose -f docker-compose.keycloak.yml --profile oauth2-proxy up -d
# Access gateway through OAuth2 Proxy at http://localhost:4180
```

### Load Balancing: Traefik (`docker-compose.traefik.yml`)

Reverse proxy with automatic HTTPS, load balancing, and rate limiting.

```bash
docker compose -f docker-compose.traefik.yml up -d

# Endpoints:
# - Gateway (HTTPS): https://gateway.localhost
# - Traefik Dashboard: http://localhost:8080
```

**Features:**
- 3 gateway instances with round-robin load balancing
- Automatic HTTPS with Let's Encrypt (configure for production)
- Rate limiting (100 req/s with 50 burst)
- Security headers (HSTS, XSS protection, etc.)
- Sticky sessions for consistent routing
- Health checks with automatic failover

**Production HTTPS:**
1. Update `DOMAIN` and `ACME_EMAIL` in `.env`
2. Uncomment Let's Encrypt configuration in compose file
3. Ensure ports 80/443 are publicly accessible

### Database HA: PostgreSQL Replicas (`docker-compose.postgres-ha.yml`)

PostgreSQL with streaming replication for read scaling.

```bash
docker compose -f docker-compose.postgres-ha.yml up -d

# Connections:
# - Primary (writes): pgbouncer-primary:6432 or localhost:6432
# - Replicas (reads): pgbouncer-replica:6432 or localhost:6433
```

**Architecture:**
- Primary PostgreSQL for all writes
- 2 read replicas with streaming replication
- PgBouncer connection pooling for both primary and replicas
- Optional HAProxy for replica load balancing

**Configure gateway for read/write splitting:**
```toml
[database]
url = "postgres://gateway:pass@pgbouncer-primary:6432/gateway"
read_url = "postgres://gateway:pass@pgbouncer-replica:6432/gateway"
```

### Cache HA: Redis Cluster (`docker-compose.redis-cluster.yml`)

6-node Redis Cluster for high availability caching.

```bash
# Start Redis nodes
docker compose -f docker-compose.redis-cluster.yml up -d

# Initialize cluster (one-time)
docker compose -f docker-compose.redis-cluster.yml --profile init up redis-cluster-init

# Optional: Redis Insight UI
docker compose -f docker-compose.redis-cluster.yml --profile ui up -d redis-insight
# Access at http://localhost:8001
```

**Cluster topology:**
- 3 master nodes (redis-1, redis-2, redis-3)
- 3 replica nodes (redis-4, redis-5, redis-6)
- Automatic failover if a master goes down

### Async Processing: Dead Letter Queue (`docker-compose.dlq.yml`)

RabbitMQ for reliable async operations with dead letter handling.

```bash
docker compose -f docker-compose.dlq.yml up -d

# Endpoints:
# - Gateway: http://localhost:8080
# - RabbitMQ Management: http://localhost:15672 (guest/guest)
```

**Pre-configured queues:**
- `usage.tracking` - Async usage tracking with DLQ
- `webhooks` - Webhook delivery with priority and DLQ
- `audit.log` - Audit log stream

**Alternative: NATS JetStream:**
```bash
docker compose -f docker-compose.dlq.yml --profile nats up -d
# NATS monitoring: http://localhost:8222
```

### Full Production: Reference Architecture (`docker-compose.production.yml`)

Comprehensive production setup combining all features.

```bash
# Configure all environment variables
cp ../.env.example .env
# Edit .env with production values

# Start everything
docker compose -f docker-compose.production.yml up -d
```

**Includes:**
- Traefik with automatic HTTPS
- 3 load-balanced gateway instances
- PostgreSQL with read replica and PgBouncer
- 3-node Redis cluster
- RabbitMQ for async processing
- Keycloak for authentication
- Full observability stack (OTEL, Prometheus, Grafana, Jaeger, Loki)

**Network isolation:**
- `frontend` network: Public-facing services (Traefik, Grafana, Keycloak)
- `backend` network: Internal services (databases, Redis, RabbitMQ)

## Configuration Files

The `config/` directory contains configuration for all services:

```
config/
├── alertmanager.yml          # Alert routing (Slack, PagerDuty, email)
├── grafana/
│   └── provisioning/
│       ├── dashboards/       # Dashboard auto-provisioning
│       └── datasources/      # Datasource configuration
├── loki.yaml                 # Log aggregation settings
├── nats.conf                 # NATS JetStream configuration
├── otel-collector.yaml       # OpenTelemetry pipeline
├── prometheus.yml            # Prometheus scrape config
├── prometheus-alerts.yml     # Alerting rules
├── promtail.yaml             # Log collection from Docker
├── rabbitmq/
│   ├── definitions.json      # Queue/exchange topology
│   └── rabbitmq.conf         # RabbitMQ settings
├── redis-cluster.conf        # Redis cluster settings
├── traefik/
│   └── dynamic/              # Dynamic Traefik configuration
├── vault/
│   └── config.hcl            # Vault server configuration
└── vault-agent/
    ├── agent.hcl             # Vault Agent configuration
    └── templates/            # Secret templates
```

## Environment Variables

See `../.env.example` for all available environment variables. Key variables:

| Variable | Description | Required For |
|----------|-------------|--------------|
| `OPENROUTER_API_KEY` | OpenRouter API key | All |
| `POSTGRES_PASSWORD` | PostgreSQL password | postgres, postgres-ha, production |
| `RABBITMQ_PASSWORD` | RabbitMQ password | dlq, production |
| `KEYCLOAK_ADMIN_PASSWORD` | Keycloak admin password | keycloak, production |
| `OIDC_CLIENT_SECRET` | OIDC client secret | keycloak, production |
| `GRAFANA_PASSWORD` | Grafana admin password | observability, production |
| `DOMAIN` | Your domain name | traefik, production |
| `ACME_EMAIL` | Let's Encrypt email | traefik, production |

## Scaling

### Horizontal Scaling (Gateway)

```bash
# Scale to 5 gateway instances
docker compose -f docker-compose.traefik.yml up -d --scale gateway-1=5
```

### Vertical Scaling (Resources)

Add resource limits to any service:

```yaml
services:
  gateway:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

## Health Checks

All services include health checks. Monitor status:

```bash
# Check all service health
docker compose -f docker-compose.postgres.yml ps

# View health check logs
docker inspect --format='{{json .State.Health}}' hadrian-gateway | jq
```

## Troubleshooting

### View Logs

```bash
# All services
docker compose -f docker-compose.postgres.yml logs -f

# Specific service
docker compose -f docker-compose.postgres.yml logs -f gateway

# Last 100 lines
docker compose -f docker-compose.postgres.yml logs --tail=100 gateway
```

### Common Issues

**Gateway can't connect to database:**
```bash
# Check database is healthy
docker compose -f docker-compose.postgres.yml ps postgres
# Check network connectivity
docker exec hadrian-gateway ping postgres
```

**Redis cluster not forming:**
```bash
# Check cluster status
docker exec hadrian-redis-1 redis-cli cluster info
# Re-run cluster init
docker compose -f docker-compose.redis-cluster.yml --profile init up redis-cluster-init
```

**Vault sealed after restart:**
```bash
# Unseal with your keys
docker exec hadrian-vault vault operator unseal <key>
```

## Production Checklist

- [ ] Configure real domain name and SSL certificates
- [ ] Set strong passwords for all services
- [ ] Enable Vault with proper unsealing strategy (auto-unseal recommended)
- [ ] Configure alerting destinations (Slack, PagerDuty, email)
- [ ] Set up database backups
- [ ] Configure log retention policies
- [ ] Review and adjust resource limits
- [ ] Set up external monitoring (uptime checks)
- [ ] Configure firewall rules (only expose necessary ports)
- [ ] Enable audit logging
