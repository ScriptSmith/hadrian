# Adding a New Admin Endpoint

Follow these steps to add a new admin endpoint. Use `users` as a reference implementation.

## 1. Define Models (`src/models/`)

Create or update model structs with serde and utoipa derives:

```rust
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Widget {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWidget {
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateWidget {
    pub name: Option<String>,
}
```

## 2. Define Repository Trait (`src/db/repos/{resource}.rs`)

```rust
use async_trait::async_trait;
use uuid::Uuid;

use super::ListParams;
use crate::{db::error::DbResult, models::{CreateWidget, UpdateWidget, Widget}};

#[async_trait]
pub trait WidgetRepo: Send + Sync {
    async fn create(&self, input: CreateWidget) -> DbResult<Widget>;
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Widget>>;
    async fn list(&self, params: ListParams) -> DbResult<Vec<Widget>>;
    async fn update(&self, id: Uuid, input: UpdateWidget) -> DbResult<Widget>;
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
```

Add to `src/db/repos/mod.rs`:
```rust
mod widgets;
pub use widgets::WidgetRepo;
```

## 3. Implement for SQLite (`src/db/sqlite/{resource}.rs`)

```rust
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

pub struct SqliteWidgetRepo {
    pool: SqlitePool,
}

impl SqliteWidgetRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WidgetRepo for SqliteWidgetRepo {
    async fn create(&self, input: CreateWidget) -> DbResult<Widget> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(r#"
            INSERT INTO widgets (id, name, created_at, updated_at)
            VALUES (?, ?, ?, ?)
        "#)
        .bind(id.to_string())
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!("Widget '{}' already exists", input.name))
            }
            _ => DbError::from(e),
        })?;

        Ok(Widget { id, name: input.name, created_at: now, updated_at: now })
    }
    // ... implement other methods
}
```

## 4. Implement for PostgreSQL (`src/db/postgres/{resource}.rs`)

Same pattern as SQLite but use `$1, $2` placeholders and native UUID type.

## 5. Wire up DbPool (`src/db/mod.rs`)

Add a method to `DbPool` to access the repo:
```rust
pub fn widgets(&self) -> &dyn WidgetRepo {
    match self {
        DbPool::Sqlite(pool) => &pool.widgets,
        DbPool::Postgres(pool) => &pool.widgets,
    }
}
```

## 6. Add Service Layer (`src/services/{resource}.rs`)

```rust
use std::sync::Arc;
use crate::db::{DbPool, DbResult, ListParams};

#[derive(Clone)]
pub struct WidgetService {
    db: Arc<DbPool>,
}

impl WidgetService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateWidget) -> DbResult<Widget> {
        self.db.widgets().create(input).await
    }
    // ... wrap other repo methods, add caching if needed
}
```

Add to `Services` struct in `src/services/mod.rs`.

## 7. Add Route Handlers (`src/routes/admin/{resource}.rs`)

```rust
use axum::{Json, extract::{Path, Query, State}};

use super::{error::AdminError, organizations::ListQuery};
use crate::{AppState, models::{CreateWidget, Widget}, services::Services};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create a widget
#[utoipa::path(
    post,
    path = "/admin/v1/widgets",
    tag = "widgets",
    request_body = CreateWidget,
    responses(
        (status = 200, description = "Widget created", body = Widget),
        (status = 409, description = "Conflict", body = crate::openapi::ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateWidget>,
) -> Result<Json<Widget>, AdminError> {
    let services = get_services(&state)?;
    let widget = services.widgets.create(input).await?;
    Ok(Json(widget))
}

// Add get, list, update, delete handlers following same pattern
```

## 8. Implement Cursor-Based Pagination

All list endpoints use cursor-based (keyset) pagination. Do not use offset-based pagination.

### Route Handler Pattern

Use `ListQuery` for query params and convert to `ListParams`:

```rust
pub async fn list(Query(query): Query<ListQuery>) -> Result<Json<Response>, AdminError> {
    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;
    let result = repo.list(params).await?;
    let pagination = PaginationMeta::with_cursors(
        limit, result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );
    Ok(Json(Response { data: result.items, pagination }))
}
```

### Repository SQL Pattern

Fetch `limit + 1` rows to detect `has_more`, use `ROW(created_at, id)` comparison:

```rust
// Forward pagination (default): get items BEFORE cursor
WHERE ROW(created_at, id) < ROW($cursor_ts, $cursor_id)
ORDER BY created_at DESC, id DESC

// Backward pagination: get items AFTER cursor, then reverse
WHERE ROW(created_at, id) > ROW($cursor_ts, $cursor_id)
ORDER BY created_at ASC, id ASC
```

### Cursor Encoding

Uses `{timestamp_millis}:{uuid}` encoded as URL-safe base64.

### Key Types (in `src/db/repos/cursor.rs`)

- `Cursor` — Encodes `created_at` + `id` for stable ordering
- `ListParams` — Internal pagination params (limit, cursor, direction)
- `ListResult<T>` — Query result with items, `has_more`, and `PageCursors`
- `PaginationMeta` — Response pagination metadata (in `src/openapi.rs`)

**Important:** Truncate timestamps to milliseconds when creating entities, since cursors use millisecond precision. This prevents comparison issues in SQLite (which stores DateTime as TEXT).

## 9. Register Routes (`src/routes/admin/mod.rs`)

```rust
pub mod widgets;

fn admin_v1_routes() -> Router<AppState> {
    Router::new()
        // ... existing routes
        .route("/widgets", post(widgets::create).get(widgets::list))
        .route("/widgets/{id}", get(widgets::get).patch(widgets::update).delete(widgets::delete))
}
```

## 10. Update OpenAPI Schema (`src/openapi.rs`)

Add the new endpoints and schemas to the `#[openapi(...)]` macro.

## 11. Add Tests

Add tests in the route handler file following patterns in `src/routes/admin/mod.rs` tests section.

## 12. Regenerate UI Client

After starting the server:
```bash
cd ui && pnpm openapi-ts
```
