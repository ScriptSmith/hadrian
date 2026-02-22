//! Shared tests for AuditLogRepo implementations
//!
//! Tests are written as async functions that take a test context containing
//! the audit log repo and utilities for creating test organizations and projects.

use chrono::Duration;
use serde_json::json;
use uuid::Uuid;

use crate::{
    db::repos::{AuditLogRepo, OrganizationRepo, ProjectRepo},
    models::{AuditActorType, AuditLogQuery, CreateAuditLog, CreateOrganization, CreateProject},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_audit_log_input(
    actor_type: AuditActorType,
    actor_id: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: Uuid,
) -> CreateAuditLog {
    CreateAuditLog {
        actor_type,
        actor_id,
        action: action.to_string(),
        resource_type: resource_type.to_string(),
        resource_id,
        org_id: None,
        project_id: None,
        details: json!({}),
        ip_address: None,
        user_agent: None,
    }
}

fn create_org_input(slug: &str, name: &str) -> CreateOrganization {
    CreateOrganization {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

fn create_project_input(slug: &str, name: &str) -> CreateProject {
    CreateProject {
        slug: slug.to_string(),
        name: name.to_string(),
        team_id: None,
    }
}

/// Test context containing repos needed for audit log tests
pub struct AuditLogTestContext<'a> {
    pub audit_log_repo: &'a dyn AuditLogRepo,
    pub org_repo: &'a dyn OrganizationRepo,
    pub project_repo: &'a dyn ProjectRepo,
}

impl<'a> AuditLogTestContext<'a> {
    /// Create a test organization and return its ID
    pub async fn create_test_org(&self, slug: &str) -> Uuid {
        self.org_repo
            .create(create_org_input(slug, &format!("Org {}", slug)))
            .await
            .expect("Failed to create test org")
            .id
    }

    /// Create a test project and return its ID
    pub async fn create_test_project(&self, org_id: Uuid, slug: &str) -> Uuid {
        self.project_repo
            .create(
                org_id,
                create_project_input(slug, &format!("Project {}", slug)),
            )
            .await
            .expect("Failed to create test project")
            .id
    }
}

// ============================================================================
// Create Tests
// ============================================================================

pub async fn test_create_basic(ctx: &AuditLogTestContext<'_>) {
    let user_id = Uuid::new_v4();
    let resource_id = Uuid::new_v4();
    let input = create_audit_log_input(
        AuditActorType::User,
        Some(user_id),
        "api_key.create",
        "api_key",
        resource_id,
    );

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    assert!(!log.id.is_nil());
    assert_eq!(log.actor_type, AuditActorType::User);
    assert_eq!(log.actor_id, Some(user_id));
    assert_eq!(log.action, "api_key.create");
    assert_eq!(log.resource_type, "api_key");
    assert_eq!(log.resource_id, resource_id);
}

pub async fn test_create_with_all_fields(ctx: &AuditLogTestContext<'_>) {
    let actor_id = Uuid::new_v4();
    let resource_id = Uuid::new_v4();
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    let input = CreateAuditLog {
        actor_type: AuditActorType::User,
        actor_id: Some(actor_id),
        action: "user.update".to_string(),
        resource_type: "user".to_string(),
        resource_id,
        org_id: Some(org_id),
        project_id: Some(project_id),
        details: json!({"field": "name", "old": "Alice", "new": "Bob"}),
        ip_address: Some("192.168.1.1".to_string()),
        user_agent: Some("Mozilla/5.0".to_string()),
    };

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    assert_eq!(log.org_id, Some(org_id));
    assert_eq!(log.project_id, Some(project_id));
    assert_eq!(log.ip_address, Some("192.168.1.1".to_string()));
    assert_eq!(log.user_agent, Some("Mozilla/5.0".to_string()));
    assert_eq!(log.details["field"], "name");
    assert_eq!(log.details["old"], "Alice");
    assert_eq!(log.details["new"], "Bob");
}

pub async fn test_create_with_api_key_actor(ctx: &AuditLogTestContext<'_>) {
    let api_key_id = Uuid::new_v4();
    let resource_id = Uuid::new_v4();
    let input = create_audit_log_input(
        AuditActorType::ApiKey,
        Some(api_key_id),
        "chat.completion",
        "completion",
        resource_id,
    );

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    assert_eq!(log.actor_type, AuditActorType::ApiKey);
    assert_eq!(log.actor_id, Some(api_key_id));
}

pub async fn test_create_with_system_actor(ctx: &AuditLogTestContext<'_>) {
    let resource_id = Uuid::new_v4();
    let input = create_audit_log_input(
        AuditActorType::System,
        None,
        "cache.clear",
        "cache",
        resource_id,
    );

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    assert_eq!(log.actor_type, AuditActorType::System);
    assert!(log.actor_id.is_none());
}

pub async fn test_create_with_complex_details(ctx: &AuditLogTestContext<'_>) {
    let resource_id = Uuid::new_v4();
    let details = json!({
        "changes": [
            {"field": "name", "before": "old", "after": "new"},
            {"field": "email", "before": "old@example.com", "after": "new@example.com"}
        ],
        "metadata": {
            "reason": "user request",
            "approved_by": "admin"
        }
    });

    let input = CreateAuditLog {
        actor_type: AuditActorType::User,
        actor_id: Some(Uuid::new_v4()),
        action: "user.update".to_string(),
        resource_type: "user".to_string(),
        resource_id,
        org_id: None,
        project_id: None,
        details: details.clone(),
        ip_address: None,
        user_agent: None,
    };

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    assert_eq!(log.details, details);
}

// ============================================================================
// Get by ID Tests
// ============================================================================

pub async fn test_get_by_id(ctx: &AuditLogTestContext<'_>) {
    let resource_id = Uuid::new_v4();
    let input = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "api_key.create",
        "api_key",
        resource_id,
    );

    let created = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");
    let fetched = ctx
        .audit_log_repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.action, "api_key.create");
    assert_eq!(fetched.resource_id, resource_id);
}

pub async fn test_get_by_id_not_found(ctx: &AuditLogTestContext<'_>) {
    let result = ctx
        .audit_log_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

// ============================================================================
// List Tests
// ============================================================================

pub async fn test_list_empty(ctx: &AuditLogTestContext<'_>) {
    let result = ctx
        .audit_log_repo
        .list(AuditLogQuery::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
}

pub async fn test_list_with_records(ctx: &AuditLogTestContext<'_>) {
    for i in 0..3 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            &format!("action.{}", i),
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let result = ctx
        .audit_log_repo
        .list(AuditLogQuery::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 3);
}

pub async fn test_list_ordered_by_timestamp_desc(ctx: &AuditLogTestContext<'_>) {
    for i in 0..3 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            &format!("action.{}", i),
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let result = ctx
        .audit_log_repo
        .list(AuditLogQuery::default())
        .await
        .expect("Failed to list");

    // Most recent should be first
    assert!(result.items[0].timestamp >= result.items[1].timestamp);
    assert!(result.items[1].timestamp >= result.items[2].timestamp);
}

pub async fn test_list_pagination(ctx: &AuditLogTestContext<'_>) {
    // Add small delays to ensure distinct timestamps for stable cursor pagination
    for i in 0..5 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            &format!("action.{}", i),
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
        tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
    }

    let page1 = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            limit: Some(2),
            ..Default::default()
        })
        .await
        .expect("Failed to list page 1");

    let page2 = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            limit: Some(2),
            cursor: page1.cursors.next.as_ref().map(|c| c.encode()),
            ..Default::default()
        })
        .await
        .expect("Failed to list page 2");

    assert_eq!(page1.items.len(), 2);
    assert_eq!(page2.items.len(), 2);
    assert_ne!(page1.items[0].id, page2.items[0].id);
}

pub async fn test_list_filter_by_actor_type(ctx: &AuditLogTestContext<'_>) {
    let user_input = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "user.action",
        "resource",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(user_input)
        .await
        .expect("Failed to create");

    let api_key_input = create_audit_log_input(
        AuditActorType::ApiKey,
        Some(Uuid::new_v4()),
        "api_key.action",
        "resource",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(api_key_input)
        .await
        .expect("Failed to create");

    let system_input = create_audit_log_input(
        AuditActorType::System,
        None,
        "system.action",
        "resource",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(system_input)
        .await
        .expect("Failed to create");

    let user_logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            actor_type: Some(AuditActorType::User),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(user_logs.items.len(), 1);
    assert_eq!(user_logs.items[0].actor_type, AuditActorType::User);
}

pub async fn test_list_filter_by_actor_id(ctx: &AuditLogTestContext<'_>) {
    let actor1 = Uuid::new_v4();
    let actor2 = Uuid::new_v4();

    for _ in 0..2 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(actor1),
            "action",
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let input = create_audit_log_input(
        AuditActorType::User,
        Some(actor2),
        "action",
        "resource",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            actor_id: Some(actor1),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 2);
    assert!(logs.items.iter().all(|l| l.actor_id == Some(actor1)));
}

pub async fn test_list_filter_by_action(ctx: &AuditLogTestContext<'_>) {
    let input1 = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "api_key.create",
        "api_key",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(input1)
        .await
        .expect("Failed to create");

    let input2 = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "api_key.delete",
        "api_key",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(input2)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            action: Some("api_key.create".to_string()),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].action, "api_key.create");
}

pub async fn test_list_filter_by_resource_type(ctx: &AuditLogTestContext<'_>) {
    let input1 = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "create",
        "api_key",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(input1)
        .await
        .expect("Failed to create");

    let input2 = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "create",
        "user",
        Uuid::new_v4(),
    );
    ctx.audit_log_repo
        .create(input2)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            resource_type: Some("api_key".to_string()),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].resource_type, "api_key");
}

pub async fn test_list_filter_by_resource_id(ctx: &AuditLogTestContext<'_>) {
    let resource1 = Uuid::new_v4();
    let resource2 = Uuid::new_v4();

    let input1 = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "action",
        "resource",
        resource1,
    );
    ctx.audit_log_repo
        .create(input1)
        .await
        .expect("Failed to create");

    let input2 = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "action",
        "resource",
        resource2,
    );
    ctx.audit_log_repo
        .create(input2)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            resource_id: Some(resource1),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].resource_id, resource1);
}

pub async fn test_list_filter_by_org_id(ctx: &AuditLogTestContext<'_>) {
    let org1 = ctx.create_test_org("org-1").await;
    let org2 = ctx.create_test_org("org-2").await;

    let input1 = CreateAuditLog {
        org_id: Some(org1),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input1)
        .await
        .expect("Failed to create");

    let input2 = CreateAuditLog {
        org_id: Some(org2),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input2)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            org_id: Some(org1),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].org_id, Some(org1));
}

pub async fn test_list_filter_by_project_id(ctx: &AuditLogTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project1 = ctx.create_test_project(org_id, "project-1").await;
    let project2 = ctx.create_test_project(org_id, "project-2").await;

    let input1 = CreateAuditLog {
        project_id: Some(project1),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input1)
        .await
        .expect("Failed to create");

    let input2 = CreateAuditLog {
        project_id: Some(project2),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input2)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            project_id: Some(project1),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].project_id, Some(project1));
}

pub async fn test_list_filter_by_date_range_from(ctx: &AuditLogTestContext<'_>) {
    for _ in 0..3 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let future = chrono::Utc::now() + Duration::hours(1);

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            from: Some(future),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert!(logs.items.is_empty());
}

pub async fn test_list_filter_by_date_range_to(ctx: &AuditLogTestContext<'_>) {
    for _ in 0..3 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let past = chrono::Utc::now() - Duration::hours(1);

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            to: Some(past),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert!(logs.items.is_empty());
}

pub async fn test_list_filter_by_date_range_combined(ctx: &AuditLogTestContext<'_>) {
    for _ in 0..3 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let now = chrono::Utc::now();
    let past = now - Duration::hours(1);
    let future = now + Duration::hours(1);

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            from: Some(past),
            to: Some(future),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 3);
}

pub async fn test_list_combined_filters(ctx: &AuditLogTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let actor_id = Uuid::new_v4();

    // Create matching log
    let matching_input = CreateAuditLog {
        org_id: Some(org_id),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(actor_id),
            "api_key.create",
            "api_key",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(matching_input)
        .await
        .expect("Failed to create");

    // Create non-matching logs
    let other_org = ctx.create_test_org("other-org").await;
    let input2 = CreateAuditLog {
        org_id: Some(other_org), // Different org
        ..create_audit_log_input(
            AuditActorType::User,
            Some(actor_id),
            "api_key.create",
            "api_key",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input2)
        .await
        .expect("Failed to create");

    let input3 = CreateAuditLog {
        org_id: Some(org_id),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()), // Different actor
            "api_key.create",
            "api_key",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input3)
        .await
        .expect("Failed to create");

    let logs = ctx
        .audit_log_repo
        .list(AuditLogQuery {
            org_id: Some(org_id),
            actor_id: Some(actor_id),
            action: Some("api_key.create".to_string()),
            ..Default::default()
        })
        .await
        .expect("Failed to list");

    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].org_id, Some(org_id));
    assert_eq!(logs.items[0].actor_id, Some(actor_id));
}

// ============================================================================
// Count Tests
// ============================================================================

pub async fn test_count_empty(ctx: &AuditLogTestContext<'_>) {
    let count = ctx
        .audit_log_repo
        .count(AuditLogQuery::default())
        .await
        .expect("Failed to count");

    assert_eq!(count, 0);
}

pub async fn test_count_with_records(ctx: &AuditLogTestContext<'_>) {
    for _ in 0..5 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let count = ctx
        .audit_log_repo
        .count(AuditLogQuery::default())
        .await
        .expect("Failed to count");

    assert_eq!(count, 5);
}

pub async fn test_count_with_filters(ctx: &AuditLogTestContext<'_>) {
    for _ in 0..3 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "api_key.create",
            "api_key",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    for _ in 0..2 {
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "api_key.delete",
            "api_key",
            Uuid::new_v4(),
        );
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    let create_count = ctx
        .audit_log_repo
        .count(AuditLogQuery {
            action: Some("api_key.create".to_string()),
            ..Default::default()
        })
        .await
        .expect("Failed to count");

    let delete_count = ctx
        .audit_log_repo
        .count(AuditLogQuery {
            action: Some("api_key.delete".to_string()),
            ..Default::default()
        })
        .await
        .expect("Failed to count");

    assert_eq!(create_count, 3);
    assert_eq!(delete_count, 2);
}

pub async fn test_count_with_combined_filters(ctx: &AuditLogTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    // Create matching logs
    for _ in 0..2 {
        let input = CreateAuditLog {
            org_id: Some(org_id),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };
        ctx.audit_log_repo
            .create(input)
            .await
            .expect("Failed to create");
    }

    // Create non-matching log (different org)
    let other_org = ctx.create_test_org("other-org").await;
    let input = CreateAuditLog {
        org_id: Some(other_org),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        )
    };
    ctx.audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    let count = ctx
        .audit_log_repo
        .count(AuditLogQuery {
            org_id: Some(org_id),
            ..Default::default()
        })
        .await
        .expect("Failed to count");

    assert_eq!(count, 2);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

pub async fn test_special_characters_in_action(ctx: &AuditLogTestContext<'_>) {
    let input = create_audit_log_input(
        AuditActorType::User,
        Some(Uuid::new_v4()),
        "action:with/special.chars_and-more",
        "resource",
        Uuid::new_v4(),
    );

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    let fetched = ctx
        .audit_log_repo
        .get_by_id(log.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.action, "action:with/special.chars_and-more");
}

pub async fn test_unicode_in_details(ctx: &AuditLogTestContext<'_>) {
    let details = json!({
        "message": "Hello 你好 مرحبا 🌍",
        "user_name": "Jürgen Müller"
    });

    let input = CreateAuditLog {
        details: details.clone(),
        ..create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            Uuid::new_v4(),
        )
    };

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    let fetched = ctx
        .audit_log_repo
        .get_by_id(log.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.details, details);
}

pub async fn test_null_optional_fields(ctx: &AuditLogTestContext<'_>) {
    let input = CreateAuditLog {
        actor_type: AuditActorType::System,
        actor_id: None,
        action: "system.startup".to_string(),
        resource_type: "system".to_string(),
        resource_id: Uuid::new_v4(),
        org_id: None,
        project_id: None,
        details: json!({}),
        ip_address: None,
        user_agent: None,
    };

    let log = ctx
        .audit_log_repo
        .create(input)
        .await
        .expect("Failed to create");

    let fetched = ctx
        .audit_log_repo
        .get_by_id(log.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert!(fetched.actor_id.is_none());
    assert!(fetched.org_id.is_none());
    assert!(fetched.project_id.is_none());
    assert!(fetched.ip_address.is_none());
    assert!(fetched.user_agent.is_none());
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::{SqliteAuditLogRepo, SqliteOrganizationRepo, SqliteProjectRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (
        SqliteAuditLogRepo,
        SqliteOrganizationRepo,
        SqliteProjectRepo,
    ) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteAuditLogRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool.clone()),
            SqliteProjectRepo::new(pool),
        )
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (audit_log_repo, org_repo, project_repo) = create_repos().await;
                let ctx = AuditLogTestContext {
                    audit_log_repo: &audit_log_repo,
                    org_repo: &org_repo,
                    project_repo: &project_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Create tests
    sqlite_test!(test_create_basic);
    sqlite_test!(test_create_with_all_fields);
    sqlite_test!(test_create_with_api_key_actor);
    sqlite_test!(test_create_with_system_actor);
    sqlite_test!(test_create_with_complex_details);

    // Get by ID tests
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);

    // List tests
    sqlite_test!(test_list_empty);
    sqlite_test!(test_list_with_records);
    sqlite_test!(test_list_ordered_by_timestamp_desc);
    sqlite_test!(test_list_pagination);
    sqlite_test!(test_list_filter_by_actor_type);
    sqlite_test!(test_list_filter_by_actor_id);
    sqlite_test!(test_list_filter_by_action);
    sqlite_test!(test_list_filter_by_resource_type);
    sqlite_test!(test_list_filter_by_resource_id);
    sqlite_test!(test_list_filter_by_org_id);
    sqlite_test!(test_list_filter_by_project_id);
    sqlite_test!(test_list_filter_by_date_range_from);
    sqlite_test!(test_list_filter_by_date_range_to);
    sqlite_test!(test_list_filter_by_date_range_combined);
    sqlite_test!(test_list_combined_filters);

    // Count tests
    sqlite_test!(test_count_empty);
    sqlite_test!(test_count_with_records);
    sqlite_test!(test_count_with_filters);
    sqlite_test!(test_count_with_combined_filters);

    // Edge case tests
    sqlite_test!(test_special_characters_in_action);
    sqlite_test!(test_unicode_in_details);
    sqlite_test!(test_null_optional_fields);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresAuditLogRepo, PostgresOrganizationRepo, PostgresProjectRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let audit_log_repo = PostgresAuditLogRepo::new(pool.clone(), None);
                let org_repo = PostgresOrganizationRepo::new(pool.clone(), None);
                let project_repo = PostgresProjectRepo::new(pool, None);
                let ctx = AuditLogTestContext {
                    audit_log_repo: &audit_log_repo,
                    org_repo: &org_repo,
                    project_repo: &project_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Create tests
    postgres_test!(test_create_basic);
    postgres_test!(test_create_with_all_fields);
    postgres_test!(test_create_with_api_key_actor);
    postgres_test!(test_create_with_system_actor);
    postgres_test!(test_create_with_complex_details);

    // Get by ID tests
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);

    // List tests
    postgres_test!(test_list_empty);
    postgres_test!(test_list_with_records);
    postgres_test!(test_list_ordered_by_timestamp_desc);
    postgres_test!(test_list_pagination);
    postgres_test!(test_list_filter_by_actor_type);
    postgres_test!(test_list_filter_by_actor_id);
    postgres_test!(test_list_filter_by_action);
    postgres_test!(test_list_filter_by_resource_type);
    postgres_test!(test_list_filter_by_resource_id);
    postgres_test!(test_list_filter_by_org_id);
    postgres_test!(test_list_filter_by_project_id);
    postgres_test!(test_list_filter_by_date_range_from);
    postgres_test!(test_list_filter_by_date_range_to);
    postgres_test!(test_list_filter_by_date_range_combined);
    postgres_test!(test_list_combined_filters);

    // Count tests
    postgres_test!(test_count_empty);
    postgres_test!(test_count_with_records);
    postgres_test!(test_count_with_filters);
    postgres_test!(test_count_with_combined_filters);

    // Edge case tests
    postgres_test!(test_special_characters_in_action);
    postgres_test!(test_unicode_in_details);
    postgres_test!(test_null_optional_fields);
}
