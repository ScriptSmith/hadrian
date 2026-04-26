//! Cross-replica leader election for periodic background jobs.
//!
//! Without coordination every gateway replica runs every cleanup tick — that
//! duplicates upstream calls (vector store deletes, provider health probes),
//! emits redundant events, and wastes egress. We use Postgres' session-level
//! `pg_try_advisory_lock(bigint)` so each tick can early-out when another
//! replica is already holding the lock, releasing automatically when the
//! holding session disconnects.
//!
//! SQLite is single-process by construction, so the helper is a no-op there;
//! every tick proceeds.

use crate::db::DbPool;

/// Stable lock keys (random 64-bit constants). Don't reuse across jobs.
///
/// Only cleanup-style workers — those whose work is shared global state
/// (DB rows, external storage) — get a key here. `model_catalog_sync` and
/// `provider_health_check` deliberately don't, because they fan out per-
/// replica state (in-memory registries, circuit breakers) that every
/// replica must compute independently.
pub mod keys {
    pub const VECTOR_STORE_CLEANUP: i64 = 0x6861_6472_5f76_7363_u64 as i64;
    pub const OAUTH_CODE_CLEANUP: i64 = 0x6861_6472_5f6f_6163_u64 as i64;
}

/// Outcome of a leader-election attempt.
#[allow(dead_code)] // `Leader` / `NotLeader` are unused on SQLite-only builds
pub enum LeadershipOutcome {
    /// We acquired the lock; caller should run the work and let the guard
    /// drop after to release the Postgres session.
    Leader(LeaderGuard),
    /// Another replica already holds the lock; skip this tick.
    NotLeader,
    /// SQLite (or no DB-side advisory lock available); proceed without
    /// coordination.
    NoCoordination,
}

/// Holds an open dedicated connection that owns a Postgres advisory lock.
/// Drop releases the connection (and therefore the lock).
pub struct LeaderGuard {
    #[cfg(feature = "database-postgres")]
    _conn: sqlx::pool::PoolConnection<sqlx::Postgres>,
}

/// Try to acquire the named advisory lock for the duration of the returned
/// guard. Returns `LeadershipOutcome::NoCoordination` for SQLite so existing
/// single-replica deployments keep behaving as before.
pub async fn try_acquire(db: &DbPool, key: i64) -> LeadershipOutcome {
    #[cfg(feature = "database-postgres")]
    {
        let Some(pool) = db.pg_write_pool() else {
            return LeadershipOutcome::NoCoordination;
        };
        let mut conn = match pool.acquire().await {
            Ok(c) => c,
            Err(err) => {
                tracing::warn!(error = %err, key, "advisory lock: could not acquire connection");
                return LeadershipOutcome::NotLeader;
            }
        };
        let acquired: bool = match sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&mut *conn)
            .await
        {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(error = %err, key, "advisory lock: pg_try_advisory_lock failed");
                return LeadershipOutcome::NotLeader;
            }
        };
        if acquired {
            LeadershipOutcome::Leader(LeaderGuard { _conn: conn })
        } else {
            LeadershipOutcome::NotLeader
        }
    }
    #[cfg(not(feature = "database-postgres"))]
    {
        let _ = (db, key);
        LeadershipOutcome::NoCoordination
    }
}
