//! Shared tests for ConversationRepo implementations
//!
//! Tests are written as async functions that take a ConversationRepo trait object.
//! This allows the same test logic to run against both SQLite and PostgreSQL.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ConversationRepo, ListParams},
    },
    models::{
        AppendMessages, ConversationOwner, ConversationOwnerType, CreateConversation, Message,
        UpdateConversation,
    },
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_message(role: &str, content: &str) -> Message {
    Message {
        role: role.to_string(),
        content: content.to_string(),
    }
}

fn create_conversation_input(
    owner: ConversationOwner,
    title: &str,
    models: Vec<&str>,
    messages: Vec<Message>,
) -> CreateConversation {
    CreateConversation {
        owner,
        title: title.to_string(),
        models: models.into_iter().map(|m| m.to_string()).collect(),
        messages,
    }
}

// ============================================================================
// Create Tests
// ============================================================================

pub async fn test_create_with_project_owner(repo: &dyn ConversationRepo) {
    let project_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::Project { project_id },
        "Test Conversation",
        vec!["gpt-4"],
        vec![],
    );

    let conv = repo.create(input).await.expect("Failed to create");

    assert!(!conv.id.is_nil());
    assert_eq!(conv.owner_type, ConversationOwnerType::Project);
    assert_eq!(conv.owner_id, project_id);
    assert_eq!(conv.title, "Test Conversation");
    assert_eq!(conv.models, vec!["gpt-4"]);
    assert!(conv.messages.is_empty());
}

pub async fn test_create_with_user_owner(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "User Chat",
        vec!["claude-3-opus"],
        vec![],
    );

    let conv = repo.create(input).await.expect("Failed to create");

    assert_eq!(conv.owner_type, ConversationOwnerType::User);
    assert_eq!(conv.owner_id, user_id);
    assert_eq!(conv.title, "User Chat");
}

pub async fn test_create_with_messages(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let messages = vec![
        create_message("user", "Hello!"),
        create_message("assistant", "Hi there!"),
    ];
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Chat with history",
        vec!["gpt-4"],
        messages,
    );

    let conv = repo.create(input).await.expect("Failed to create");

    assert_eq!(conv.messages.len(), 2);
    assert_eq!(conv.messages[0].role, "user");
    assert_eq!(conv.messages[0].content, "Hello!");
    assert_eq!(conv.messages[1].role, "assistant");
    assert_eq!(conv.messages[1].content, "Hi there!");
}

pub async fn test_create_with_multiple_models(repo: &dyn ConversationRepo) {
    let project_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::Project { project_id },
        "Multi-model chat",
        vec!["gpt-4", "claude-3-opus", "gemini-pro"],
        vec![],
    );

    let conv = repo.create(input).await.expect("Failed to create");

    assert_eq!(conv.models.len(), 3);
    assert_eq!(conv.models[0], "gpt-4");
    assert_eq!(conv.models[1], "claude-3-opus");
    assert_eq!(conv.models[2], "gemini-pro");
}

// ============================================================================
// Get by ID Tests
// ============================================================================

pub async fn test_get_by_id(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Fetch Test",
        vec!["gpt-4"],
        vec![create_message("user", "Test message")],
    );

    let created = repo.create(input).await.expect("Failed to create");
    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.title, "Fetch Test");
    assert_eq!(fetched.messages.len(), 1);
    assert_eq!(fetched.messages[0].content, "Test message");
}

pub async fn test_get_by_id_not_found(repo: &dyn ConversationRepo) {
    let result = repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_id_deleted_returns_none(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "To be deleted",
        vec![],
        vec![],
    );

    let created = repo.create(input).await.expect("Failed to create");
    repo.delete(created.id).await.expect("Failed to delete");

    let result = repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

// ============================================================================
// List by Owner Tests
// ============================================================================

pub async fn test_list_by_owner_empty(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let result = repo
        .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_owner_with_records(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    for i in 0..3 {
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            &format!("Conversation {}", i),
            vec![],
            vec![],
        );
        repo.create(input).await.expect("Failed to create");
    }

    let result = repo
        .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_list_by_owner_filters_by_owner_type(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();

    // Create user conversations
    for i in 0..2 {
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            &format!("User Conv {}", i),
            vec![],
            vec![],
        );
        repo.create(input).await.expect("Failed to create");
    }

    // Create project conversations
    for i in 0..3 {
        let input = create_conversation_input(
            ConversationOwner::Project { project_id },
            &format!("Project Conv {}", i),
            vec![],
            vec![],
        );
        repo.create(input).await.expect("Failed to create");
    }

    let user_result = repo
        .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
        .await
        .expect("Failed to list user conversations");

    let project_result = repo
        .list_by_owner(
            ConversationOwnerType::Project,
            project_id,
            ListParams::default(),
        )
        .await
        .expect("Failed to list project conversations");

    assert_eq!(user_result.items.len(), 2);
    assert_eq!(project_result.items.len(), 3);
}

pub async fn test_list_by_owner_filters_by_owner_id(repo: &dyn ConversationRepo) {
    let user1 = Uuid::new_v4();
    let user2 = Uuid::new_v4();

    let input1 = create_conversation_input(
        ConversationOwner::User { user_id: user1 },
        "User 1 Chat",
        vec![],
        vec![],
    );
    repo.create(input1).await.expect("Failed to create");

    let input2 = create_conversation_input(
        ConversationOwner::User { user_id: user2 },
        "User 2 Chat",
        vec![],
        vec![],
    );
    repo.create(input2).await.expect("Failed to create");

    let user1_result = repo
        .list_by_owner(ConversationOwnerType::User, user1, ListParams::default())
        .await
        .expect("Failed to list");

    let user2_result = repo
        .list_by_owner(ConversationOwnerType::User, user2, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(user1_result.items.len(), 1);
    assert_eq!(user1_result.items[0].title, "User 1 Chat");
    assert_eq!(user2_result.items.len(), 1);
    assert_eq!(user2_result.items[0].title, "User 2 Chat");
}

pub async fn test_list_by_owner_pagination(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    for i in 0..5 {
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            &format!("Conv {}", i),
            vec![],
            vec![],
        );
        repo.create(input).await.expect("Failed to create");
    }

    let page1 = repo
        .list_by_owner(
            ConversationOwnerType::User,
            user_id,
            ListParams {
                limit: Some(2),
                include_deleted: false,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 1");

    let page2 = repo
        .list_by_owner(
            ConversationOwnerType::User,
            user_id,
            ListParams {
                limit: Some(2),
                include_deleted: false,
                cursor: page1.cursors.next.clone(),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 2");

    assert_eq!(page1.items.len(), 2);
    assert_eq!(page2.items.len(), 2);
    assert!(page1.has_more);
    assert!(page2.has_more);
    assert_ne!(page1.items[0].id, page2.items[0].id);
}

pub async fn test_list_by_owner_excludes_deleted(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();

    let input1 = create_conversation_input(
        ConversationOwner::User { user_id },
        "Active Conv",
        vec![],
        vec![],
    );
    repo.create(input1).await.expect("Failed to create");

    let input2 = create_conversation_input(
        ConversationOwner::User { user_id },
        "Deleted Conv",
        vec![],
        vec![],
    );
    let to_delete = repo.create(input2).await.expect("Failed to create");
    repo.delete(to_delete.id).await.expect("Failed to delete");

    let result = repo
        .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Active Conv");
}

pub async fn test_list_by_owner_include_deleted(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();

    let input1 = create_conversation_input(
        ConversationOwner::User { user_id },
        "Active Conv",
        vec![],
        vec![],
    );
    repo.create(input1).await.expect("Failed to create");

    let input2 = create_conversation_input(
        ConversationOwner::User { user_id },
        "Deleted Conv",
        vec![],
        vec![],
    );
    let to_delete = repo.create(input2).await.expect("Failed to create");
    repo.delete(to_delete.id).await.expect("Failed to delete");

    let result = repo
        .list_by_owner(
            ConversationOwnerType::User,
            user_id,
            ListParams {
                limit: None,
                include_deleted: true,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 2);
}

// ============================================================================
// Count by Owner Tests
// ============================================================================

pub async fn test_count_by_owner_empty(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let count = repo
        .count_by_owner(ConversationOwnerType::User, user_id, false)
        .await
        .expect("Failed to count");

    assert_eq!(count, 0);
}

pub async fn test_count_by_owner_with_records(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    for i in 0..4 {
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            &format!("Conv {}", i),
            vec![],
            vec![],
        );
        repo.create(input).await.expect("Failed to create");
    }

    let count = repo
        .count_by_owner(ConversationOwnerType::User, user_id, false)
        .await
        .expect("Failed to count");

    assert_eq!(count, 4);
}

pub async fn test_count_by_owner_excludes_deleted(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();

    let input1 = create_conversation_input(
        ConversationOwner::User { user_id },
        "Active",
        vec![],
        vec![],
    );
    repo.create(input1).await.expect("Failed to create");

    let input2 = create_conversation_input(
        ConversationOwner::User { user_id },
        "To Delete",
        vec![],
        vec![],
    );
    let to_delete = repo.create(input2).await.expect("Failed to create");
    repo.delete(to_delete.id).await.expect("Failed to delete");

    let count = repo
        .count_by_owner(ConversationOwnerType::User, user_id, false)
        .await
        .expect("Failed to count");

    assert_eq!(count, 1);
}

pub async fn test_count_by_owner_include_deleted(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();

    let input1 = create_conversation_input(
        ConversationOwner::User { user_id },
        "Active",
        vec![],
        vec![],
    );
    repo.create(input1).await.expect("Failed to create");

    let input2 = create_conversation_input(
        ConversationOwner::User { user_id },
        "To Delete",
        vec![],
        vec![],
    );
    let to_delete = repo.create(input2).await.expect("Failed to create");
    repo.delete(to_delete.id).await.expect("Failed to delete");

    let count = repo
        .count_by_owner(ConversationOwnerType::User, user_id, true)
        .await
        .expect("Failed to count");

    assert_eq!(count, 2);
}

// ============================================================================
// Update Tests
// ============================================================================

pub async fn test_update_title(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Original Title",
        vec!["gpt-4"],
        vec![],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let updated = repo
        .update(
            created.id,
            UpdateConversation {
                title: Some("New Title".to_string()),
                models: None,
                messages: None,
                owner: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.title, "New Title");
    assert_eq!(updated.models, vec!["gpt-4"]); // Unchanged
}

pub async fn test_update_models(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Test",
        vec!["gpt-4"],
        vec![],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let updated = repo
        .update(
            created.id,
            UpdateConversation {
                title: None,
                models: Some(vec!["claude-3-opus".to_string(), "gemini-pro".to_string()]),
                messages: None,
                owner: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.title, "Test"); // Unchanged
    assert_eq!(updated.models, vec!["claude-3-opus", "gemini-pro"]);
}

pub async fn test_update_messages(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Test",
        vec![],
        vec![create_message("user", "Original")],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let new_messages = vec![
        create_message("user", "Replaced"),
        create_message("assistant", "Response"),
    ];
    let updated = repo
        .update(
            created.id,
            UpdateConversation {
                title: None,
                models: None,
                messages: Some(new_messages),
                owner: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.messages.len(), 2);
    assert_eq!(updated.messages[0].content, "Replaced");
    assert_eq!(updated.messages[1].content, "Response");
}

pub async fn test_update_all_fields(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Original",
        vec!["gpt-4"],
        vec![],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let updated = repo
        .update(
            created.id,
            UpdateConversation {
                title: Some("Updated".to_string()),
                models: Some(vec!["claude-3".to_string()]),
                messages: Some(vec![create_message("system", "Hello")]),
                owner: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.title, "Updated");
    assert_eq!(updated.models, vec!["claude-3"]);
    assert_eq!(updated.messages.len(), 1);
    assert_eq!(updated.messages[0].role, "system");
}

pub async fn test_update_not_found(repo: &dyn ConversationRepo) {
    let result = repo
        .update(
            Uuid::new_v4(),
            UpdateConversation {
                title: Some("Test".to_string()),
                models: None,
                messages: None,
                owner: None,
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_update_deleted_fails(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");
    repo.delete(created.id).await.expect("Failed to delete");

    let result = repo
        .update(
            created.id,
            UpdateConversation {
                title: Some("Updated".to_string()),
                models: None,
                messages: None,
                owner: None,
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_update_preserves_timestamps(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");

    // Small delay to ensure updated_at differs
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let updated = repo
        .update(
            created.id,
            UpdateConversation {
                title: Some("Updated".to_string()),
                models: None,
                messages: None,
                owner: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.created_at, created.created_at);
    assert!(updated.updated_at > created.updated_at);
}

// ============================================================================
// Append Messages Tests
// ============================================================================

pub async fn test_append_messages_to_empty(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");

    let messages = repo
        .append_messages(
            created.id,
            AppendMessages {
                messages: vec![
                    create_message("user", "Hello"),
                    create_message("assistant", "Hi!"),
                ],
            },
        )
        .await
        .expect("Failed to append");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "Hello");
    assert_eq!(messages[1].content, "Hi!");
}

pub async fn test_append_messages_to_existing(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Test",
        vec![],
        vec![create_message("user", "First")],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let messages = repo
        .append_messages(
            created.id,
            AppendMessages {
                messages: vec![
                    create_message("assistant", "Second"),
                    create_message("user", "Third"),
                ],
            },
        )
        .await
        .expect("Failed to append");

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].content, "First");
    assert_eq!(messages[1].content, "Second");
    assert_eq!(messages[2].content, "Third");
}

pub async fn test_append_messages_not_found(repo: &dyn ConversationRepo) {
    let result = repo
        .append_messages(
            Uuid::new_v4(),
            AppendMessages {
                messages: vec![create_message("user", "Test")],
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_append_messages_to_deleted_fails(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");
    repo.delete(created.id).await.expect("Failed to delete");

    let result = repo
        .append_messages(
            created.id,
            AppendMessages {
                messages: vec![create_message("user", "Test")],
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_append_messages_updates_timestamp(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    repo.append_messages(
        created.id,
        AppendMessages {
            messages: vec![create_message("user", "Test")],
        },
    )
    .await
    .expect("Failed to append");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert!(fetched.updated_at > created.updated_at);
}

// ============================================================================
// Delete Tests
// ============================================================================

pub async fn test_delete(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");

    repo.delete(created.id).await.expect("Failed to delete");

    let result = repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_delete_not_found(repo: &dyn ConversationRepo) {
    let result = repo.delete(Uuid::new_v4()).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete_already_deleted(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input =
        create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
    let created = repo.create(input).await.expect("Failed to create");
    repo.delete(created.id)
        .await
        .expect("First delete should succeed");

    let result = repo.delete(created.id).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

pub async fn test_messages_with_special_characters(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let special_content = r#"Hello "world"! What's up? <script>alert('xss')</script>"#;
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Test",
        vec![],
        vec![create_message("user", special_content)],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.messages[0].content, special_content);
}

pub async fn test_unicode_content(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let unicode_content = "Hello 你好 مرحبا 🌍 🚀 ñ é ü";
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Test 日本語",
        vec!["model-日本語"],
        vec![create_message("user", unicode_content)],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.title, "Test 日本語");
    assert_eq!(fetched.models[0], "model-日本語");
    assert_eq!(fetched.messages[0].content, unicode_content);
}

pub async fn test_empty_models_vec(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "No Models",
        vec![],
        vec![],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert!(fetched.models.is_empty());
}

pub async fn test_update_to_empty_models(repo: &dyn ConversationRepo) {
    let user_id = Uuid::new_v4();
    let input = create_conversation_input(
        ConversationOwner::User { user_id },
        "Test",
        vec!["gpt-4"],
        vec![],
    );
    let created = repo.create(input).await.expect("Failed to create");

    let updated = repo
        .update(
            created.id,
            UpdateConversation {
                title: None,
                models: Some(vec![]),
                messages: None,
                owner: None,
            },
        )
        .await
        .expect("Failed to update");

    assert!(updated.models.is_empty());
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use crate::db::{
        sqlite::SqliteConversationRepo,
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repo() -> SqliteConversationRepo {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        SqliteConversationRepo::new(pool)
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let repo = create_repo().await;
                super::$name(&repo).await;
            }
        };
    }

    // Create tests
    sqlite_test!(test_create_with_project_owner);
    sqlite_test!(test_create_with_user_owner);
    sqlite_test!(test_create_with_messages);
    sqlite_test!(test_create_with_multiple_models);

    // Get by ID tests
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);
    sqlite_test!(test_get_by_id_deleted_returns_none);

    // List by owner tests
    sqlite_test!(test_list_by_owner_empty);
    sqlite_test!(test_list_by_owner_with_records);
    sqlite_test!(test_list_by_owner_filters_by_owner_type);
    sqlite_test!(test_list_by_owner_filters_by_owner_id);
    sqlite_test!(test_list_by_owner_pagination);
    sqlite_test!(test_list_by_owner_excludes_deleted);
    sqlite_test!(test_list_by_owner_include_deleted);

    // Count by owner tests
    sqlite_test!(test_count_by_owner_empty);
    sqlite_test!(test_count_by_owner_with_records);
    sqlite_test!(test_count_by_owner_excludes_deleted);
    sqlite_test!(test_count_by_owner_include_deleted);

    // Update tests
    sqlite_test!(test_update_title);
    sqlite_test!(test_update_models);
    sqlite_test!(test_update_messages);
    sqlite_test!(test_update_all_fields);
    sqlite_test!(test_update_not_found);
    sqlite_test!(test_update_deleted_fails);
    sqlite_test!(test_update_preserves_timestamps);

    // Append messages tests
    sqlite_test!(test_append_messages_to_empty);
    sqlite_test!(test_append_messages_to_existing);
    sqlite_test!(test_append_messages_not_found);
    sqlite_test!(test_append_messages_to_deleted_fails);
    sqlite_test!(test_append_messages_updates_timestamp);

    // Delete tests
    sqlite_test!(test_delete);
    sqlite_test!(test_delete_not_found);
    sqlite_test!(test_delete_already_deleted);

    // Edge case tests
    sqlite_test!(test_messages_with_special_characters);
    sqlite_test!(test_unicode_content);
    sqlite_test!(test_empty_models_vec);
    sqlite_test!(test_update_to_empty_models);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use crate::db::{
        postgres::PostgresConversationRepo,
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let repo = PostgresConversationRepo::new(pool, None);
                super::$name(&repo).await;
            }
        };
    }

    // Create tests
    postgres_test!(test_create_with_project_owner);
    postgres_test!(test_create_with_user_owner);
    postgres_test!(test_create_with_messages);
    postgres_test!(test_create_with_multiple_models);

    // Get by ID tests
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_get_by_id_deleted_returns_none);

    // List by owner tests
    postgres_test!(test_list_by_owner_empty);
    postgres_test!(test_list_by_owner_with_records);
    postgres_test!(test_list_by_owner_filters_by_owner_type);
    postgres_test!(test_list_by_owner_filters_by_owner_id);
    postgres_test!(test_list_by_owner_pagination);
    postgres_test!(test_list_by_owner_excludes_deleted);
    postgres_test!(test_list_by_owner_include_deleted);

    // Count by owner tests
    postgres_test!(test_count_by_owner_empty);
    postgres_test!(test_count_by_owner_with_records);
    postgres_test!(test_count_by_owner_excludes_deleted);
    postgres_test!(test_count_by_owner_include_deleted);

    // Update tests
    postgres_test!(test_update_title);
    postgres_test!(test_update_models);
    postgres_test!(test_update_messages);
    postgres_test!(test_update_all_fields);
    postgres_test!(test_update_not_found);
    postgres_test!(test_update_deleted_fails);
    postgres_test!(test_update_preserves_timestamps);

    // Append messages tests
    postgres_test!(test_append_messages_to_empty);
    postgres_test!(test_append_messages_to_existing);
    postgres_test!(test_append_messages_not_found);
    postgres_test!(test_append_messages_to_deleted_fails);
    postgres_test!(test_append_messages_updates_timestamp);

    // Delete tests
    postgres_test!(test_delete);
    postgres_test!(test_delete_not_found);
    postgres_test!(test_delete_already_deleted);

    // Edge case tests
    postgres_test!(test_messages_with_special_characters);
    postgres_test!(test_unicode_content);
    postgres_test!(test_empty_models_vec);
    postgres_test!(test_update_to_empty_models);
}
