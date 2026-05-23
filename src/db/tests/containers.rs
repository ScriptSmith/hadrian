//! Shared repository tests for [`ContainersRepo`], focused on the
//! `hard_delete_expired` cleanup path. Run against both SQLite (fast,
//! in-memory) and PostgreSQL (testcontainers, `--ignored`).

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{
    db::repos::{
        ContainerFileSourceKind, ContainerPatch, ContainerStatus, ContainersRepo, NewContainer,
        NewContainerFile, ResponseOwner, truncate_to_millis,
    },
    models::StorageBackend,
};

/// Insert a fresh `active` container for `org_id` and return its id.
async fn seed_org(repo: &dyn ContainersRepo, org_id: Uuid) -> String {
    let container_id = format!("cntr_{}", Uuid::new_v4().simple());
    repo.insert(NewContainer::from_owner(
        container_id.clone(),
        org_id,
        ResponseOwner::Organization(org_id),
        "test",
        None,
        1200,
        truncate_to_millis(Utc::now()),
    ))
    .await
    .expect("insert container");
    container_id
}

/// Flip a container into a terminal state with an explicit `expires_at`.
async fn make_terminal(
    repo: &dyn ContainersRepo,
    id: &str,
    org_id: Uuid,
    status: ContainerStatus,
    expires_at: chrono::DateTime<Utc>,
) {
    repo.update_within_org(
        id,
        org_id,
        ContainerPatch {
            status: Some(status),
            last_active_at: None,
            expires_at: Some(expires_at),
        },
    )
    .await
    .expect("transition container")
    .expect("container exists");
}

/// Old `expired` and `deleted` containers past the cutoff are hard-deleted,
/// and their `container_files` cascade.
pub async fn hard_delete_removes_old_terminal_containers(repo: &dyn ContainersRepo, org_id: Uuid) {
    let now = truncate_to_millis(Utc::now());
    let old = now - Duration::hours(2);

    let expired_id = seed_org(repo, org_id).await;
    make_terminal(repo, &expired_id, org_id, ContainerStatus::Expired, old).await;

    let deleted_id = seed_org(repo, org_id).await;
    make_terminal(repo, &deleted_id, org_id, ContainerStatus::Deleted, old).await;

    // Attach a file to the expired container so we can assert the cascade.
    repo.upsert_file(NewContainerFile {
        id: format!("cfile_{}", Uuid::new_v4().simple()),
        container_id: expired_id.clone(),
        org_id,
        path: "/mnt/data/out.bin".to_string(),
        filename: "out.bin".to_string(),
        size_bytes: 4,
        content_type: Some("application/octet-stream".to_string()),
        content_hash: "deadbeef".to_string(),
        source: ContainerFileSourceKind::Assistant,
        storage_backend: StorageBackend::Database,
        file_data: Some(b"data".to_vec()),
        storage_path: None,
        source_response_id: None,
        source_call_id: None,
        created_at: now,
    })
    .await
    .expect("upsert file");

    // Cutoff: anything terminal more than 1h ago.
    let cutoff = now - Duration::hours(1);
    let mut deleted = repo.hard_delete_expired(cutoff, 100).await.expect("delete");
    deleted.sort();
    let mut want = vec![expired_id.clone(), deleted_id.clone()];
    want.sort();
    assert_eq!(deleted, want, "both old terminal containers deleted");

    assert!(
        repo.get_by_id_and_org(&expired_id, org_id)
            .await
            .expect("get")
            .is_none(),
        "expired container row gone"
    );
    assert!(
        repo.get_by_id_and_org(&deleted_id, org_id)
            .await
            .expect("get")
            .is_none(),
        "deleted container row gone"
    );
    // Files cascade with the container row.
    let files = repo
        .list_files_for_replay(&expired_id)
        .await
        .expect("list files");
    assert!(files.is_empty(), "container_files cascaded on hard delete");
}

/// Active containers and terminal containers newer than the cutoff are NOT
/// hard-deleted.
pub async fn hard_delete_spares_active_and_recent(repo: &dyn ContainersRepo, org_id: Uuid) {
    let now = truncate_to_millis(Utc::now());

    // Active container — never a candidate, even with no expires_at.
    let active_id = seed_org(repo, org_id).await;

    // Terminal but transitioned only a moment ago (after the cutoff).
    let recent_id = seed_org(repo, org_id).await;
    make_terminal(repo, &recent_id, org_id, ContainerStatus::Expired, now).await;

    // Terminal and old enough to be a candidate.
    let old_id = seed_org(repo, org_id).await;
    make_terminal(
        repo,
        &old_id,
        org_id,
        ContainerStatus::Deleted,
        now - Duration::hours(2),
    )
    .await;

    let cutoff = now - Duration::hours(1);
    let deleted = repo.hard_delete_expired(cutoff, 100).await.expect("delete");
    assert_eq!(deleted, vec![old_id.clone()], "only the old row is deleted");

    assert!(
        repo.get_by_id_and_org(&active_id, org_id)
            .await
            .expect("get")
            .is_some(),
        "active container preserved"
    );
    assert!(
        repo.get_by_id_and_org(&recent_id, org_id)
            .await
            .expect("get")
            .is_some(),
        "recently-terminal container preserved"
    );
    assert!(
        repo.get_by_id_and_org(&old_id, org_id)
            .await
            .expect("get")
            .is_none(),
        "old terminal container deleted"
    );
}

/// `limit` caps how many rows a single pass removes.
pub async fn hard_delete_respects_limit(repo: &dyn ContainersRepo, org_id: Uuid) {
    let now = truncate_to_millis(Utc::now());
    let old = now - Duration::hours(2);
    for _ in 0..3 {
        let id = seed_org(repo, org_id).await;
        make_terminal(repo, &id, org_id, ContainerStatus::Expired, old).await;
    }

    let cutoff = now - Duration::hours(1);
    let first = repo.hard_delete_expired(cutoff, 2).await.expect("delete");
    assert_eq!(first.len(), 2, "first pass capped at limit");
    let second = repo.hard_delete_expired(cutoff, 2).await.expect("delete");
    assert_eq!(second.len(), 1, "second pass drains remainder");
    let third = repo.hard_delete_expired(cutoff, 2).await.expect("delete");
    assert!(third.is_empty(), "nothing left to delete");
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use uuid::Uuid;

    use crate::{
        db::{
            repos::OrganizationRepo,
            sqlite::{SqliteContainersRepo, SqliteOrganizationRepo},
            tests::harness::{create_sqlite_pool, run_sqlite_migrations},
        },
        models::CreateOrganization,
    };

    async fn create_repo() -> (SqliteContainersRepo, Uuid) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        // Enable cascades so `hard_delete_expired` exercises the FK path the
        // production pool relies on (the harness pool doesn't set it).
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("enable foreign keys");
        let org = SqliteOrganizationRepo::new(pool.clone())
            .create(CreateOrganization {
                slug: "acme".to_string(),
                name: "Acme".to_string(),
            })
            .await
            .expect("create org");
        (SqliteContainersRepo::new(pool), org.id)
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (repo, org_id) = create_repo().await;
                super::$name(&repo, org_id).await;
            }
        };
    }

    sqlite_test!(hard_delete_removes_old_terminal_containers);
    sqlite_test!(hard_delete_spares_active_and_recent);
    sqlite_test!(hard_delete_respects_limit);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use uuid::Uuid;

    use crate::{
        db::{
            postgres::{PostgresContainersRepo, PostgresOrganizationRepo},
            repos::OrganizationRepo,
            tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
        },
        models::CreateOrganization,
    };

    async fn create_repo() -> (PostgresContainersRepo, Uuid) {
        let pool = create_isolated_postgres_pool().await;
        run_postgres_migrations(&pool).await;
        let org = PostgresOrganizationRepo::new(pool.clone(), None)
            .create(CreateOrganization {
                slug: "acme".to_string(),
                name: "Acme".to_string(),
            })
            .await
            .expect("create org");
        (PostgresContainersRepo::new(pool, None), org.id)
    }

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let (repo, org_id) = create_repo().await;
                super::$name(&repo, org_id).await;
            }
        };
    }

    postgres_test!(hard_delete_removes_old_terminal_containers);
    postgres_test!(hard_delete_spares_active_and_recent);
    postgres_test!(hard_delete_respects_limit);
}
