use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{error::AdminError, organizations::ListQuery};
use crate::{
    AppState,
    middleware::AuthzContext,
    models::{
        AppendMessages, Conversation, ConversationWithProject, CreateConversation, Message,
        SetPinOrder, UpdateConversation,
    },
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of conversations
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConversationListResponse {
    /// List of conversations
    pub data: Vec<Conversation>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create a new conversation
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/conversations",
    tag = "conversations",
    operation_id = "conversation_create",
    request_body = CreateConversation,
    responses(
        (status = 201, description = "Conversation created", body = Conversation),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn create(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateConversation>>,
) -> Result<(StatusCode, Json<Conversation>), AdminError> {
    authz.require("conversation", "create", None, None, None, None)?;
    let services = get_services(&state)?;

    // Verify the owner exists
    match &input.owner {
        crate::models::ConversationOwner::Project { project_id } => {
            services
                .projects
                .get_by_id(*project_id)
                .await?
                .ok_or_else(|| {
                    AdminError::NotFound(format!("Project '{}' not found", project_id))
                })?;
        }
        crate::models::ConversationOwner::User { user_id } => {
            services
                .users
                .get_by_id(*user_id)
                .await?
                .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;
        }
    }

    let conversation = services.conversations.create(input).await?;
    Ok((StatusCode::CREATED, Json(conversation)))
}

/// Get a conversation by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/conversations/{id}",
    tag = "conversations",
    operation_id = "conversation_get",
    params(("id" = Uuid, Path, description = "Conversation ID")),
    responses(
        (status = 200, description = "Conversation found", body = Conversation),
        (status = 404, description = "Conversation not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Conversation>, AdminError> {
    authz.require("conversation", "read", None, None, None, None)?;
    let services = get_services(&state)?;

    let conversation = services
        .conversations
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Conversation '{}' not found", id)))?;

    Ok(Json(conversation))
}

/// List conversations by project
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/conversations",
    tag = "conversations",
    operation_id = "conversation_list_by_project",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of conversations", body = ConversationListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_project(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ConversationListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;
    authz.require(
        "conversation",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get project by slug
    let project = services
        .projects
        .get_by_slug(org.id, &project_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Project '{}' not found in organization '{}'",
                project_slug, org_slug
            ))
        })?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .conversations
        .list_by_project(project.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(ConversationListResponse {
        data: result.items,
        pagination,
    }))
}

/// List conversations by user
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/conversations",
    tag = "conversations",
    operation_id = "conversation_list_by_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of conversations", body = ConversationListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ConversationListResponse>, AdminError> {
    authz.require("conversation", "list", None, None, None, None)?;
    let services = get_services(&state)?;

    // Verify user exists
    services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.conversations.list_by_user(user_id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(ConversationListResponse {
        data: result.items,
        pagination,
    }))
}

/// List of conversations with project metadata
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConversationWithProjectListResponse {
    /// List of conversations with project info
    pub data: Vec<ConversationWithProject>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

/// Query parameters for listing accessible conversations
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
pub struct ListAccessibleQuery {
    /// Maximum number of conversations to return
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 1000))]
    pub limit: Option<i64>,
}

/// List all conversations accessible to a user
///
/// Returns both user's personal conversations and conversations from projects they belong to.
/// Results include project metadata when applicable.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/conversations/accessible",
    tag = "conversations",
    operation_id = "conversation_list_accessible_for_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListAccessibleQuery,
    ),
    responses(
        (status = 200, description = "List of accessible conversations", body = ConversationWithProjectListResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_accessible_for_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListAccessibleQuery>,
) -> Result<Json<ConversationWithProjectListResponse>, AdminError> {
    authz.require("conversation", "list", None, None, None, None)?;
    let services = get_services(&state)?;

    // Verify user exists
    services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    let limit = query.limit.unwrap_or(100).min(1000);

    // Request one extra item to determine has_more
    let mut conversations = services
        .conversations
        .list_accessible_for_user(user_id, limit + 1, false)
        .await?;

    let has_more = conversations.len() as i64 > limit;
    if has_more {
        conversations.truncate(limit as usize);
    }

    let pagination = PaginationMeta::with_cursors(limit, has_more, None, None);

    Ok(Json(ConversationWithProjectListResponse {
        data: conversations,
        pagination,
    }))
}

/// Update a conversation
///
/// Can also be used to move a conversation to a different project or user by providing the `owner` field.
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/conversations/{id}",
    tag = "conversations",
    operation_id = "conversation_update",
    params(("id" = Uuid, Path, description = "Conversation ID")),
    request_body = UpdateConversation,
    responses(
        (status = 200, description = "Conversation updated", body = Conversation),
        (status = 404, description = "Conversation or new owner not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn update(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdateConversation>>,
) -> Result<Json<Conversation>, AdminError> {
    authz.require("conversation", "update", None, None, None, None)?;
    let services = get_services(&state)?;

    // Verify the new owner exists if one is provided
    if let Some(ref owner) = input.owner {
        match owner {
            crate::models::ConversationOwner::Project { project_id } => {
                services
                    .projects
                    .get_by_id(*project_id)
                    .await?
                    .ok_or_else(|| {
                        AdminError::NotFound(format!("Project '{}' not found", project_id))
                    })?;
            }
            crate::models::ConversationOwner::User { user_id } => {
                services
                    .users
                    .get_by_id(*user_id)
                    .await?
                    .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;
            }
        }
    }

    let updated = services.conversations.update(id, input).await?;
    Ok(Json(updated))
}

/// Append messages to a conversation
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/conversations/{id}/messages",
    tag = "conversations",
    operation_id = "conversation_append_messages",
    params(("id" = Uuid, Path, description = "Conversation ID")),
    request_body = AppendMessages,
    responses(
        (status = 200, description = "Messages appended, returns all messages", body = Vec<Message>),
        (status = 404, description = "Conversation not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn append_messages(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<AppendMessages>>,
) -> Result<Json<Vec<Message>>, AdminError> {
    authz.require("conversation", "update", None, None, None, None)?;
    let services = get_services(&state)?;

    let messages = services.conversations.append_messages(id, input).await?;
    Ok(Json(messages))
}

/// Delete a conversation
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/conversations/{id}",
    tag = "conversations",
    operation_id = "conversation_delete",
    params(("id" = Uuid, Path, description = "Conversation ID")),
    responses(
        (status = 200, description = "Conversation deleted"),
        (status = 404, description = "Conversation not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    authz.require("conversation", "delete", None, None, None, None)?;
    let services = get_services(&state)?;

    services.conversations.delete(id).await?;
    Ok(Json(()))
}

/// Set pin order for a conversation
///
/// Set `pin_order` to a number to pin the conversation (0 = first, higher = lower in list).
/// Set `pin_order` to null to unpin the conversation.
#[cfg_attr(feature = "utoipa", utoipa::path(
    put,
    path = "/admin/v1/conversations/{id}/pin",
    tag = "conversations",
    operation_id = "conversation_set_pin",
    params(("id" = Uuid, Path, description = "Conversation ID")),
    request_body = SetPinOrder,
    responses(
        (status = 200, description = "Pin order updated", body = Conversation),
        (status = 404, description = "Conversation not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn set_pin(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<SetPinOrder>>,
) -> Result<Json<Conversation>, AdminError> {
    authz.require("conversation", "update", None, None, None, None)?;
    let services = get_services(&state)?;

    let updated = services
        .conversations
        .set_pin_order(id, input.pin_order)
        .await?;
    Ok(Json(updated))
}
