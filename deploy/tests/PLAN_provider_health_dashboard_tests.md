# Plan: Provider Health Dashboard E2E Tests

## Overview

Create comprehensive E2E tests for the Provider Health Dashboard feature. The tests will verify:
1. Provider health status endpoints
2. Circuit breaker status endpoints
3. Provider metrics/stats endpoints (extend existing analytics tests)
4. Real-time WebSocket event updates
5. Integration between health states and circuit breaker

## Current State

### Existing Tests
- `shared/health-checks.ts` - Tests general `/health`, `/health/liveness`, `/health/readiness` endpoints
- `shared/analytics.ts` - Tests `/admin/v1/providers/stats` and `/admin/v1/providers/{provider}/stats` endpoints

### Missing Coverage
The following endpoints/features have no E2E tests:
1. `GET /admin/v1/providers/health` - All providers health status
2. `GET /admin/v1/providers/{provider}/health` - Single provider health
3. `GET /admin/v1/providers/circuit-breakers` - All circuit breaker states
4. `GET /admin/v1/providers/{provider}/circuit-breaker` - Single circuit breaker
5. WebSocket `/ws/events` with topics: `ProviderHealthChanged`, `CircuitBreakerStateChanged`

## New Test Files

### 1. `shared/provider-health.ts` - Provider Health & Circuit Breaker Tests

**Context Interface:**
```typescript
export interface ProviderHealthContext {
  gatewayUrl: string;
  client: Client;
  adminToken: string;
  providerName: string; // e.g., "test"
}
```

**Test Groups:**

#### 1.1 Provider Health Status
- `GET /admin/v1/providers/health returns health states for all providers`
  - Verify response contains `providers` array
  - Verify each provider has: `provider`, `status`, `latency_ms`, `last_check`, etc.

- `GET /admin/v1/providers/{provider}/health returns single provider health`
  - Verify response for configured provider
  - Verify health status fields match expected schema

- `GET /admin/v1/providers/{provider}/health returns 404 for unknown provider`
  - Verify 404 response for non-existent provider

- `Provider health reflects actual availability`
  - Make successful requests → verify status is "healthy"
  - This is an integration test to verify health checks work

#### 1.2 Circuit Breaker Status
- `GET /admin/v1/providers/circuit-breakers returns all circuit breaker states`
  - Verify response contains `circuit_breakers` array
  - Verify each entry has: `provider`, `state`, `failure_count`

- `GET /admin/v1/providers/{provider}/circuit-breaker returns single provider CB status`
  - Verify response for configured provider
  - Default state should be "closed" with 0 failures

- `GET /admin/v1/providers/{provider}/circuit-breaker returns 404 for unknown provider`
  - Verify 404 for non-existent/non-configured provider

#### 1.3 Circuit Breaker State Transitions
- `Circuit breaker opens after consecutive failures`
  - Configure test with low failure threshold
  - Make requests to `test/error-500` model repeatedly
  - Verify circuit breaker transitions to "open"

- `Circuit breaker enters half-open after timeout`
  - After opening, wait for recovery timeout
  - Verify state becomes "half_open"

- `Circuit breaker closes after successful request in half-open`
  - In half-open state, make successful request
  - Verify circuit breaker returns to "closed"

### 2. `shared/websocket-events.ts` - Real-time WebSocket Tests

**Context Interface:**
```typescript
export interface WebSocketTestContext {
  gatewayUrl: string;
  adminToken: string;
  apiKey: string;
  providerName: string;
}
```

**Test Groups:**

#### 2.1 WebSocket Connection
- `WebSocket connects to /ws/events endpoint`
  - Establish WebSocket connection
  - Verify connection is successful

- `WebSocket accepts topic subscriptions`
  - Subscribe to "health" topic
  - Verify subscription acknowledgment

#### 2.2 Health Event Broadcasting
- `Receives ProviderHealthChanged events on health changes`
  - Subscribe to health topic
  - Trigger health state change (e.g., by causing failures then recovery)
  - Verify event received with correct payload structure

- `Receives CircuitBreakerStateChanged events on CB transitions`
  - Subscribe to health topic
  - Trigger circuit breaker state change
  - Verify event received with old_state/new_state

#### 2.3 Event Filtering
- `Only receives events for subscribed topics`
  - Subscribe to health topic only
  - Verify non-health events are not received

- `Can subscribe to multiple topics`
  - Subscribe to health and another topic
  - Verify events from both are received

### 3. Updates to Existing Tests

#### 3.1 `shared/analytics.ts` - Add Missing Coverage
The current analytics tests are good but could add:
- Test that stats include `status_code` breakdown for errors
- Test historical stats with custom time ranges

### 4. Integration Test File

Create `infrastructure/provider-health-dashboard.test.ts` that:
- Uses a Docker Compose file with:
  - Gateway with test provider (health checks enabled)
  - Circuit breaker configured with low thresholds for faster testing

- Runs all shared test suites:
  - `runProviderHealthTests()`
  - `runWebSocketEventTests()`
  - `runAnalyticsTests()` (existing)

## Docker Compose Configuration

Create `docker-compose.provider-health.yml`:
```yaml
services:
  gateway:
    build:
      context: ..
      dockerfile: Dockerfile
    ports:
      - "${GATEWAY_PORT:-8090}:8080"
    volumes:
      - ./configs/gateway.provider-health.toml:/config.toml:ro
    environment:
      RUST_LOG: info

  # Test provider is built-in, no additional services needed
```

Create `configs/gateway.provider-health.toml`:
```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
type = "sqlite"
sqlite_path = "/tmp/hadrian.db"

[auth]
api_keys = true

[features.websocket]
enabled = true

[providers.test]
type = "test"
health_check.enabled = true
health_check.interval_seconds = 5
health_check.timeout_seconds = 2
circuit_breaker.enabled = true
circuit_breaker.failure_threshold = 3
circuit_breaker.recovery_timeout_seconds = 5

[observability.metrics]
enabled = true
```

## Implementation Order

1. **Phase 1: Provider Health & Circuit Breaker Tests**
   - Create `shared/provider-health.ts`
   - Test health status endpoints
   - Test circuit breaker endpoints
   - Test state transitions

2. **Phase 2: WebSocket Event Tests**
   - Create `shared/websocket-events.ts`
   - Test connection and subscriptions
   - Test event broadcasting
   - May need to add a WebSocket client utility

3. **Phase 3: Integration Test File**
   - Create Docker Compose config
   - Create test file that runs all shared suites
   - Verify parallel execution compatibility

4. **Phase 4: Analytics Enhancements (Optional)**
   - Extend existing analytics tests with additional coverage

## Port Allocation

For the new test file:
- Gateway port: 8090 (unique from other tests)

## Dependencies

- May need to add a WebSocket client library (e.g., `ws` for Node.js) to `deploy/tests/package.json`
- Current dependencies should cover health/circuit breaker HTTP tests

## Estimated Test Count

| Test File | Test Count |
|-----------|------------|
| provider-health.ts | ~10 tests |
| websocket-events.ts | ~8 tests |
| Integration file overhead | ~2 tests |
| **Total New Tests** | **~20 tests** |

## Notes

1. **Timing Sensitivity**: Circuit breaker and health check tests are time-sensitive. Use retry patterns from existing tests.

2. **WebSocket Complexity**: WebSocket tests require async event handling. Consider using promises with timeouts for event assertions.

3. **Test Independence**: Each test should work in isolation. Reset circuit breaker state between tests if needed.

4. **Test Provider**: The built-in `test` provider has special models (`test/error-500`, `test/error-503`, `test/error-429`) that can trigger specific error codes, which is useful for testing error scenarios.
