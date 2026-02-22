# Database Changes

## Migration Files

- **SQLite**: `migrations_sqlx/sqlite/20250101000000_initial.sql`
- **PostgreSQL**: `migrations_sqlx/postgres/20250101000000_initial.sql`

Since there hasn't been a release yet, modify the existing migration files directly rather than creating new ones. Keep both files in sync.

## Key Differences Between SQLite and PostgreSQL

| Feature | SQLite | PostgreSQL |
|---------|--------|------------|
| UUID type | `TEXT` | `UUID` |
| Timestamp | `TEXT` with `datetime('now')` | `TIMESTAMPTZ` with `NOW()` |
| Boolean | `INTEGER` (0/1) | `BOOLEAN` |
| String types | `TEXT` | `VARCHAR(n)` |
| Auto-increment | `AUTOINCREMENT` | `SERIAL` / `BIGSERIAL` |
| Enum types | `CHECK` constraint | `CREATE TYPE ... AS ENUM` |
| Decimal | `REAL` | `DECIMAL(p, s)` |

## Adding a New Table

### SQLite Example

```sql
CREATE TABLE IF NOT EXISTS widgets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    is_active INTEGER NOT NULL DEFAULT 1,
    config TEXT NOT NULL DEFAULT '{}',  -- JSON stored as text
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_widgets_org_id ON widgets(org_id);
CREATE INDEX IF NOT EXISTS idx_widgets_name ON widgets(name);
```

### PostgreSQL Equivalent

```sql
CREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY NOT NULL,
    name VARCHAR(255) NOT NULL UNIQUE,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    config JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_widgets_org_id ON widgets(org_id);
CREATE INDEX IF NOT EXISTS idx_widgets_name ON widgets(name);
```

## Enum Types

### SQLite (CHECK constraint)

```sql
CREATE TABLE IF NOT EXISTS items (
    id TEXT PRIMARY KEY NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'completed'))
);
```

### PostgreSQL (CREATE TYPE)

```sql
DO $$ BEGIN
    CREATE TYPE item_status AS ENUM ('pending', 'active', 'completed');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS items (
    id UUID PRIMARY KEY NOT NULL,
    status item_status NOT NULL
);
```

## UUID Handling in Rust

SQLite stores UUIDs as TEXT, so conversion is needed:

```rust
// SQLite: UUID stored as TEXT
use super::common::parse_uuid;

let id: Uuid = parse_uuid(&row.get::<String, _>("id"))?;

// When inserting
.bind(id.to_string())
```

```rust
// PostgreSQL: Native UUID support
let id: Uuid = row.get("id");

// When inserting
.bind(id)
```

## Boolean Handling

```rust
// SQLite: INTEGER (0/1)
let is_active: bool = row.get::<i32, _>("is_active") != 0;
.bind(if is_active { 1 } else { 0 })

// PostgreSQL: Native BOOLEAN
let is_active: bool = row.get("is_active");
.bind(is_active)
```

## JSON Handling

```rust
// SQLite: TEXT column, serialize/deserialize manually
let config: serde_json::Value = serde_json::from_str(&row.get::<String, _>("config"))?;
.bind(serde_json::to_string(&config)?)

// PostgreSQL: JSONB column, native support via sqlx
let config: serde_json::Value = row.get("config");
.bind(&config)
```

## Index Considerations

- Add indexes for foreign keys (e.g., `org_id`, `project_id`)
- Add indexes for frequently queried columns
- Add indexes for unique constraints
- Consider composite indexes for common query patterns

```sql
-- Single column index
CREATE INDEX IF NOT EXISTS idx_widgets_org_id ON widgets(org_id);

-- Composite index for common queries
CREATE INDEX IF NOT EXISTS idx_widgets_org_active ON widgets(org_id, is_active);

-- Partial index (PostgreSQL only)
CREATE INDEX IF NOT EXISTS idx_widgets_active ON widgets(org_id) WHERE is_active = TRUE;
```

## Soft Deletes

Use `deleted_at` column for soft deletes:

```sql
deleted_at TEXT  -- SQLite
deleted_at TIMESTAMPTZ  -- PostgreSQL
```

Filter in queries:
```sql
WHERE deleted_at IS NULL
```

## Running Migrations

Migrations run automatically on startup when `run_migrations = true` in config.

To test migrations:
```bash
# SQLite
cargo test

# PostgreSQL (requires Docker)
cd deploy/tests && pnpm test postgres
```

## Cursor-Based Pagination and Timestamps

When implementing cursor-based pagination (see `src/db/repos/cursor.rs`), timestamps must be truncated to millisecond precision to avoid comparison issues in SQLite.

**Problem**: Cursors encode timestamps as milliseconds. If entities store timestamps with nanosecond precision (from `Utc::now()`), the cursor's decoded timestamp won't exactly match the stored value. SQLite stores DateTime as TEXT, and string comparison of `2024-01-01T00:00:00.123456789Z` vs `2024-01-01T00:00:00.123Z` fails unexpectedly due to lexicographic ordering.

**Solution**: Use `truncate_to_millis()` when creating entities that support cursor pagination:

```rust
use crate::db::repos::truncate_to_millis;
use chrono::Utc;

async fn create(&self, input: CreateEntity) -> DbResult<Entity> {
    let id = Uuid::new_v4();
    let now = truncate_to_millis(Utc::now());  // Truncate to ms precision

    // ... insert with `now` as created_at
}
```

## Common Pitfalls

1. **Forgetting to sync both files** - Always update both SQLite and PostgreSQL migrations
2. **UUID binding** - Remember `.to_string()` for SQLite, direct bind for PostgreSQL
3. **Boolean values** - Use `1`/`0` for SQLite, `TRUE`/`FALSE` for PostgreSQL
4. **Datetime functions** - `datetime('now')` for SQLite, `NOW()` for PostgreSQL
5. **Enum handling** - CHECK constraint for SQLite, CREATE TYPE for PostgreSQL
6. **Timestamp precision** - Truncate to milliseconds for cursor pagination (see above)
