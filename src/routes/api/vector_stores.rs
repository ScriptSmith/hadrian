use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use axum_valid::Valid;
use chrono::Utc;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    ApiError, SortOrder, check_resource_access_optional, extract_identity_memberships,
    get_services, validate_embedding_model_compatibility,
};
use crate::{
    AppState,
    auth::AuthenticatedRequest,
    db::ListParams,
    middleware::AuthzContext,
    models::{
        AddFileToVectorStore, AttributeFilter, ChunkingStrategy, CreateVectorStore, FileId,
        FileSearchRankingOptions, UpdateVectorStore, VectorStore, VectorStoreFile,
        VectorStoreFileId, VectorStoreFileStatus, VectorStoreId, VectorStoreOwner,
        VectorStoreOwnerType, chunk_id_serde, file_id_serde, vector_store_id_serde,
    },
    openapi::PaginationMeta,
};

/// Query parameters for listing vector stores.
///
/// ## OpenAI Compatibility
///
/// This endpoint supports OpenAI-compatible cursor-based pagination:
/// - `limit`: Maximum number of results (1-100, default 20)
/// - `order`: Sort order by `created_at` timestamp (asc/desc, default desc)
/// - `after`: Cursor for forward pagination (object ID, e.g., `vs_abc123`)
/// - `before`: Cursor for backward pagination (object ID, e.g., `vs_abc123`)
///
/// ## Hadrian Extensions
///
/// - `owner_type`, `owner_id`: Optional for multi-tenancy scoping. When omitted, returns all
///   vector stores accessible to the authenticated user based on their memberships.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListVectorStoresQuery {
    /// **Hadrian Extension:** Owner type for multi-tenancy (organization, team, project, or user).
    /// When omitted along with `owner_id`, returns all accessible vector stores.
    pub owner_type: Option<String>,
    /// **Hadrian Extension:** Owner ID for multi-tenancy.
    /// When omitted along with `owner_type`, returns all accessible vector stores.
    pub owner_id: Option<Uuid>,
    /// Maximum number of vector stores to return (default: 20, max: 100)
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 100))]
    pub limit: Option<i64>,
    /// Sort order by `created_at` timestamp (default: desc)
    #[serde(default)]
    pub order: Option<SortOrder>,
    /// Cursor for forward pagination. Returns results after this object ID.
    /// Use the `last_id` from a previous response to get the next page.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vs_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub after: Option<String>,
    /// Cursor for backward pagination. Returns results before this object ID.
    /// Use the `first_id` from a previous response to get the previous page.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vs_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub before: Option<String>,
}

/// Paginated list of vector stores response (OpenAI-compatible).
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of vector stores
    pub data: Vec<VectorStore>,
    /// ID of the first object in the list (for backward pagination with `before`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    /// ID of the last object in the list (for forward pagination with `after`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    /// Whether there are more results available beyond this page
    pub has_more: bool,
}

/// Delete vector store response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteVectorStoreResponse {
    /// Vector store ID that was deleted
    pub id: String,
    /// Object type (always "vector_store.deleted")
    pub object: String,
    /// Whether the vector store was deleted
    pub deleted: bool,
}

/// Request to add a file to a vector store
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateVectorStoreFileRequest {
    /// The ID of the file to add (from the Files API)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// Chunking strategy for processing the file
    #[serde(default)]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Query parameters for listing vector store files (OpenAI-compatible).
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListVectorStoreFilesQuery {
    /// Maximum number of files to return (default: 20, max: 100)
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 100))]
    pub limit: Option<i64>,
    /// Sort order by `created_at` timestamp (default: desc)
    #[serde(default)]
    pub order: Option<SortOrder>,
    /// Cursor for forward pagination. Returns results after this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vsf_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub after: Option<String>,
    /// Cursor for backward pagination. Returns results before this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vsf_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub before: Option<String>,
    /// Filter by status (in_progress, completed, failed, cancelled)
    pub filter: Option<String>,
}

/// Paginated list of vector store files response (OpenAI-compatible).
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreFileListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of vector store files
    pub data: Vec<VectorStoreFile>,
    /// ID of the first file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    /// ID of the last file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    /// Whether there are more results available
    pub has_more: bool,
}

/// Delete vector store file response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteVectorStoreFileResponse {
    /// Vector store file ID that was deleted
    pub id: String,
    /// Object type (always "vector_store.file.deleted")
    pub object: String,
    /// Whether the file was deleted from the vector store
    pub deleted: bool,
}

/// Create a vector store
///
/// Creates a new vector store for storing file embeddings.
/// Optionally attaches files to the vector store at creation time via `file_ids`.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores",
    tag = "vector-stores",
    operation_id = "vector_store_create",
    request_body = CreateVectorStore,
    responses(
        (status = 201, description = "Vector store created", body = VectorStore),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_create(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Valid(Json(input)): Valid<Json<CreateVectorStore>>,
) -> Result<(StatusCode, Json<VectorStore>), ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "create",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    // Check caller has permission to create for this owner
    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        input.owner.owner_type(),
        input.owner.owner_id(),
    )?;

    // Verify the owner exists
    match &input.owner {
        VectorStoreOwner::Organization { organization_id } => {
            services
                .organizations
                .get_by_id(*organization_id)
                .await?
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_FOUND,
                        "not_found",
                        format!("Organization '{}' not found", organization_id),
                    )
                })?;
        }
        VectorStoreOwner::Team { team_id } => {
            services.teams.get_by_id(*team_id).await?.ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    format!("Team '{}' not found", team_id),
                )
            })?;
        }
        VectorStoreOwner::Project { project_id } => {
            services
                .projects
                .get_by_id(*project_id)
                .await?
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_FOUND,
                        "not_found",
                        format!("Project '{}' not found", project_id),
                    )
                })?;
        }
        VectorStoreOwner::User { user_id } => {
            services.users.get_by_id(*user_id).await?.ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    format!("User '{}' not found", user_id),
                )
            })?;
        }
    }

    // Extract file_ids and chunking_strategy before creating vector store
    let file_ids = input.file_ids.clone();
    let chunking_strategy = input.chunking_strategy.clone();

    // Create the vector store
    let vector_store = services.vector_stores.create(input).await?;

    // Attach files if file_ids were provided (OpenAI-compatible create-time file attachment)
    if !file_ids.is_empty() {
        for file_id in file_ids {
            // Verify the file exists
            if services.files.get(file_id).await?.is_none() {
                tracing::warn!(
                    file_id = %file_id,
                    vector_store_id = %vector_store.id,
                    "File not found when attaching to vector store at creation time"
                );
                continue;
            }

            let add_input = AddFileToVectorStore {
                vector_store_id: vector_store.id,
                file_id,
                chunking_strategy: chunking_strategy.clone(),
                attributes: None,
            };

            match services.vector_stores.add_file(add_input).await {
                Ok(_vector_store_file) => {
                    // Trigger file processing
                    #[cfg(any(
                        feature = "document-extraction-basic",
                        feature = "document-extraction-full"
                    ))]
                    if let Some(processor) = &state.document_processor {
                        let processor = processor.clone();
                        if let Err(e) = processor
                            .schedule_processing(_vector_store_file.internal_id)
                            .await
                        {
                            tracing::error!(
                                error = %e,
                                file_id = %_vector_store_file.internal_id,
                                "Failed to schedule file processing"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        file_id = %file_id,
                        vector_store_id = %vector_store.id,
                        "Failed to attach file to vector store at creation time"
                    );
                }
            }
        }

        // Refresh vector store to get updated file_counts
        if let Some(updated) = services.vector_stores.get_by_id(vector_store.id).await? {
            return Ok((StatusCode::CREATED, Json(updated)));
        }
    }

    Ok((StatusCode::CREATED, Json(vector_store)))
}

/// List vector stores
///
/// Returns a list of vector stores owned by the specified owner.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores",
    tag = "vector-stores",
    operation_id = "vector_store_list",
    params(ListVectorStoresQuery),
    responses(
        (status = 200, description = "List of vector stores", body = VectorStoreListResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_list(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Query(query): Query<ListVectorStoresQuery>,
) -> Result<Json<VectorStoreListResponse>, ApiError> {
    use crate::db::repos::{Cursor, CursorDirection};

    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "list",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    // OpenAI defaults: limit=20, order=desc
    let limit = query.limit.unwrap_or(20).min(100);

    // Parse cursor from `after` or `before` parameter
    // OpenAI uses object IDs as cursors (e.g., "vs_abc123")
    let (cursor, direction) = if let Some(ref after_id) = query.after {
        // `after` means get items after this ID (forward pagination)
        let vector_store_id: VectorStoreId = after_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'after' cursor: {}", after_id),
            )
        })?;

        // Look up the record to get its timestamp for keyset pagination
        let cursor_record = services
            .vector_stores
            .get_by_id(vector_store_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store '{}' not found for cursor", after_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.updated_at, cursor_record.id)),
            CursorDirection::Forward,
        )
    } else if let Some(ref before_id) = query.before {
        // `before` means get items before this ID (backward pagination)
        let vector_store_id: VectorStoreId = before_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'before' cursor: {}", before_id),
            )
        })?;

        // Look up the record to get its timestamp for keyset pagination
        let cursor_record = services
            .vector_stores
            .get_by_id(vector_store_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store '{}' not found for cursor", before_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.updated_at, cursor_record.id)),
            CursorDirection::Backward,
        )
    } else {
        (None, CursorDirection::Forward)
    };

    let params = ListParams {
        limit: Some(limit),
        cursor,
        direction,
        sort_order: query.order.unwrap_or_default().into(),
        ..Default::default()
    };

    // Determine whether to list by specific owner or by accessible collections
    let result = match (query.owner_type.as_ref(), query.owner_id) {
        // Both owner_type and owner_id provided - use single-owner listing
        (Some(owner_type_str), Some(owner_id)) => {
            let owner_type: VectorStoreOwnerType = owner_type_str.parse().map_err(|_| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_owner_type",
                    "Invalid owner_type. Must be one of: organization, team, project, user",
                )
            })?;

            // Check caller has permission to list for this owner
            check_resource_access_optional(auth.as_ref().map(|e| &e.0), owner_type, owner_id)?;

            services
                .vector_stores
                .list(owner_type, owner_id, params)
                .await?
        }

        // Neither provided - list all accessible collections based on identity
        (None, None) => {
            match auth.as_ref() {
                None => {
                    // No auth - list all vector stores (open access mode)
                    services.vector_stores.list_all(params).await?
                }
                Some(auth_ext) => {
                    let (user_id, org_ids, team_ids, project_ids) =
                        extract_identity_memberships(Some(&auth_ext.0))?;

                    services
                        .vector_stores
                        .list_accessible(user_id, &org_ids, &team_ids, &project_ids, params)
                        .await?
                }
            }
        }

        // Only one provided - invalid
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_parameters",
                "Both owner_type and owner_id must be provided together, or both omitted to list all accessible vector stores",
            ));
        }
    };

    // Build OpenAI-compatible response with first_id and last_id
    let first_id = result
        .items
        .first()
        .map(|c| VectorStoreId::new(c.id).to_string());
    let last_id = result
        .items
        .last()
        .map(|c| VectorStoreId::new(c.id).to_string());

    Ok(Json(VectorStoreListResponse {
        object: "list".to_string(),
        data: result.items,
        first_id,
        last_id,
        has_more: result.has_more,
    }))
}

/// Get a vector store
///
/// Retrieves a vector store by ID.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}",
    tag = "vector-stores",
    operation_id = "vector_store_get",
    params(("vector_store_id" = String, Path, description = "Vector store ID (e.g., vs_550e8400-e29b-41d4-a716-446655440000)")),
    responses(
        (status = 200, description = "Vector store details", body = VectorStore),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_get(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(vector_store_id): Path<VectorStoreId>,
) -> Result<Json<VectorStore>, ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "read",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id.into_inner())
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    // Check access permission
    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    Ok(Json(vector_store))
}

/// Modify a vector store
///
/// Modifies a vector store's metadata.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}",
    tag = "vector-stores",
    operation_id = "vector_store_modify",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = UpdateVectorStore,
    responses(
        (status = 200, description = "Vector store updated", body = VectorStore),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_modify(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Valid(Json(input)): Valid<Json<UpdateVectorStore>>,
) -> Result<Json<VectorStore>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let existing = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        existing.owner_type,
        existing.owner_id,
    )?;

    let vector_store = services
        .vector_stores
        .update(vector_store_id, input)
        .await?;
    Ok(Json(vector_store))
}

/// Delete a vector store
///
/// Deletes a vector store and all its files (soft delete).
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/vector_stores/{vector_store_id}",
    tag = "vector-stores",
    operation_id = "vector_store_delete",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    responses(
        (status = 200, description = "Vector store deleted", body = DeleteVectorStoreResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_delete(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(vector_store_id): Path<VectorStoreId>,
) -> Result<Json<DeleteVectorStoreResponse>, ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "delete",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let vector_store_id_prefixed = vector_store_id.to_string();
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    services.vector_stores.delete(vector_store_id).await?;

    Ok(Json(DeleteVectorStoreResponse {
        id: vector_store_id_prefixed,
        object: "vector_store.deleted".to_string(),
        deleted: true,
    }))
}

// ============================================================================
// Vector Store File Route Handlers
// ============================================================================

/// Create a vector store file
///
/// Adds a file to a vector store. The file must already exist in the Files API.
/// Processing will start automatically after the file is added.
///
/// ## Content Deduplication
///
/// Files are deduplicated by content hash (SHA-256). If a file with identical content
/// already exists in the vector store, the existing file is returned with status 200
/// instead of creating a duplicate. This is idempotent behavior—uploading the same
/// content multiple times has no additional effect.
///
/// ## Embedding Model Validation
///
/// The gateway validates that its configured embedding model matches the vector store's
/// embedding model before adding files. This prevents incompatible embeddings from being
/// stored together. If there's a mismatch, a 409 Conflict error is returned with details
/// about the expected vs. configured models.
///
/// - **201 Created**: New file added, processing started
/// - **200 OK**: Duplicate content detected, existing file returned (no re-processing)
/// - **409 Conflict**: Embedding model mismatch between gateway configuration and vector store
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}/files",
    tag = "vector-stores",
    operation_id = "vector_store_file_create",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = CreateVectorStoreFileRequest,
    responses(
        (status = 200, description = "Duplicate content detected, existing file returned", body = VectorStoreFile),
        (status = 201, description = "File added to vector store", body = VectorStoreFile),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Embedding model mismatch", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search service not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_create_file(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Json(input): Json<CreateVectorStoreFileRequest>,
) -> Result<(StatusCode, Json<VectorStoreFile>), ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Verify the file exists and get its content hash for deduplication
    let file = services.files.get(input.file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", input.file_id),
        )
    })?;

    // Verify the user has access to the file being added
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    // Check if this file is already in the vector store (idempotency)
    if let Some(existing_file) = services
        .vector_stores
        .find_by_file_id(vector_store_id, input.file_id)
        .await?
    {
        // If the file previously failed, allow re-processing by resetting status
        if existing_file.status == VectorStoreFileStatus::Failed {
            tracing::info!(
                vector_store_id = %vector_store_id,
                file_id = %input.file_id,
                vector_store_file_internal_id = %existing_file.internal_id,
                previous_error = ?existing_file.last_error,
                "Re-processing previously failed file"
            );

            // Reset status to InProgress and clear error
            services
                .vector_stores
                .update_vector_store_file_status(
                    existing_file.internal_id,
                    VectorStoreFileStatus::InProgress,
                    None,
                )
                .await?;

            // Re-trigger processing (shadow-copy pattern ensures old partial chunks
            // are cleaned up after successful re-processing)
            #[cfg(any(
                feature = "document-extraction-basic",
                feature = "document-extraction-full"
            ))]
            if let Some(processor) = &state.document_processor {
                let processor = processor.clone();
                let internal_id = existing_file.internal_id;
                if let Err(e) = processor.schedule_processing(internal_id).await {
                    tracing::error!(
                        error = %e,
                        internal_id = %internal_id,
                        "Failed to schedule file re-processing"
                    );
                }
            }

            // Return updated file with 200 OK
            let updated_file = services
                .vector_stores
                .get_vector_store_file(existing_file.internal_id)
                .await?
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "file_not_found",
                        "File disappeared after status update",
                    )
                })?;
            return Ok((StatusCode::OK, Json(updated_file)));
        }

        // Check for stale InProgress files (stuck due to worker crash, etc.)
        if existing_file.status == VectorStoreFileStatus::InProgress {
            let stale_timeout_secs = state
                .config
                .features
                .file_processing
                .stale_processing_timeout_secs;

            // Only check for staleness if timeout is configured (> 0)
            if stale_timeout_secs > 0 {
                let age_secs = (Utc::now() - existing_file.updated_at).num_seconds();
                if age_secs > stale_timeout_secs as i64 {
                    tracing::info!(
                        vector_store_id = %vector_store_id,
                        file_id = %input.file_id,
                        vector_store_file_internal_id = %existing_file.internal_id,
                        age_secs = age_secs,
                        stale_timeout_secs = stale_timeout_secs,
                        "Re-processing stale in-progress file"
                    );

                    // Reset status to InProgress (to update timestamp) and clear any error
                    services
                        .vector_stores
                        .update_vector_store_file_status(
                            existing_file.internal_id,
                            VectorStoreFileStatus::InProgress,
                            None,
                        )
                        .await?;

                    // Re-trigger processing
                    #[cfg(any(
                        feature = "document-extraction-basic",
                        feature = "document-extraction-full"
                    ))]
                    if let Some(processor) = &state.document_processor {
                        let processor = processor.clone();
                        let internal_id = existing_file.internal_id;
                        if let Err(e) = processor.schedule_processing(internal_id).await {
                            tracing::error!(
                                error = %e,
                                internal_id = %internal_id,
                                "Failed to schedule stale file re-processing"
                            );
                        }
                    }

                    // Return updated file with 200 OK
                    let updated_file = services
                        .vector_stores
                        .get_vector_store_file(existing_file.internal_id)
                        .await?
                        .ok_or_else(|| {
                            ApiError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "file_not_found",
                                "File disappeared after status update",
                            )
                        })?;
                    return Ok((StatusCode::OK, Json(updated_file)));
                }
            }
        }

        tracing::info!(
            vector_store_id = %vector_store_id,
            file_id = %input.file_id,
            vector_store_file_internal_id = %existing_file.internal_id,
            status = ?existing_file.status,
            "File already in vector_store, returning existing entry"
        );
        // Return existing entry with 200 OK (idempotent behavior)
        return Ok((StatusCode::OK, Json(existing_file)));
    }

    // Check for same-owner content deduplication (prevents accidental duplicates)
    if let Some(content_hash) = &file.content_hash
        && let Some(existing_file) = services
            .vector_stores
            .find_by_content_hash_and_owner(
                vector_store_id,
                content_hash,
                file.owner_type,
                file.owner_id,
            )
            .await?
    {
        tracing::info!(
            vector_store_id = %vector_store_id,
            file_id = %input.file_id,
            existing_file_id = %existing_file.file_id,
            vector_store_file_internal_id = %existing_file.internal_id,
            content_hash = %content_hash,
            "Same-owner duplicate content detected, returning existing file"
        );
        // Return existing file with 200 OK (deduplication)
        return Ok((StatusCode::OK, Json(existing_file)));
    }

    // Validate embedding model compatibility before adding new file.
    // This ensures the gateway's configured embedding model matches the vector store's model,
    // preventing incompatible vectors from being stored.
    validate_embedding_model_compatibility(&state, &vector_store)?;

    let add_input = AddFileToVectorStore {
        vector_store_id,
        file_id: input.file_id,
        chunking_strategy: input.chunking_strategy,
        attributes: None,
    };

    let vector_store_file = services.vector_stores.add_file(add_input).await?;

    // Trigger file processing (chunking + embedding)
    #[cfg(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    ))]
    if let Some(processor) = &state.document_processor {
        let processor = processor.clone();
        let internal_id = vector_store_file.internal_id;
        if let Err(e) = processor.schedule_processing(internal_id).await {
            tracing::error!(
                error = %e,
                internal_id = %internal_id,
                "Failed to schedule file processing"
            );
        }
    } else {
        tracing::warn!(
            internal_id = %vector_store_file.internal_id,
            "Document processor not configured, file will remain in 'in_progress' status"
        );
    }
    #[cfg(not(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    )))]
    tracing::warn!(
        internal_id = %vector_store_file.internal_id,
        "Document processor not configured (feature disabled), file will remain in 'in_progress' status"
    );

    Ok((StatusCode::CREATED, Json(vector_store_file)))
}

/// List vector store files
///
/// Returns a list of files in a vector store.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/files",
    tag = "vector-stores",
    operation_id = "vector_store_file_list",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ListVectorStoreFilesQuery,
    ),
    responses(
        (status = 200, description = "List of files", body = VectorStoreFileListResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_list_files(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Query(query): Query<ListVectorStoreFilesQuery>,
) -> Result<Json<VectorStoreFileListResponse>, ApiError> {
    use crate::db::repos::{Cursor, CursorDirection};

    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // OpenAI defaults: limit=20
    let limit = query.limit.unwrap_or(20).min(100);

    // Parse cursor from `after` or `before` parameter
    let (cursor, direction) = if let Some(ref after_id) = query.after {
        let file_id: VectorStoreFileId = after_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'after' cursor: {}", after_id),
            )
        })?;

        // Look up the record to get its timestamp for keyset pagination
        let cursor_record = services
            .vector_stores
            .get_vector_store_file(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store file '{}' not found for cursor", after_id),
                )
            })?;

        (
            Some(Cursor::new(
                cursor_record.updated_at,
                cursor_record.internal_id,
            )),
            CursorDirection::Forward,
        )
    } else if let Some(ref before_id) = query.before {
        let file_id: VectorStoreFileId = before_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'before' cursor: {}", before_id),
            )
        })?;

        let cursor_record = services
            .vector_stores
            .get_vector_store_file(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store file '{}' not found for cursor", before_id),
                )
            })?;

        (
            Some(Cursor::new(
                cursor_record.updated_at,
                cursor_record.internal_id,
            )),
            CursorDirection::Backward,
        )
    } else {
        (None, CursorDirection::Forward)
    };

    let params = ListParams {
        limit: Some(limit),
        cursor,
        direction,
        sort_order: query.order.unwrap_or_default().into(),
        ..Default::default()
    };

    let result = services
        .vector_stores
        .list_vector_store_files(vector_store_id, params)
        .await?;

    // Filter by status if requested
    let items = if let Some(filter) = query.filter {
        let status: VectorStoreFileStatus = filter.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_filter",
                format!("Invalid filter status: {}", filter),
            )
        })?;
        result
            .items
            .into_iter()
            .filter(|f| f.status == status)
            .collect()
    } else {
        result.items
    };

    // Build OpenAI-compatible response
    // Use file_id as the external ID (matches OpenAI behavior)
    let first_id = items.first().map(|f| FileId::new(f.file_id).to_string());
    let last_id = items.last().map(|f| FileId::new(f.file_id).to_string());

    Ok(Json(VectorStoreFileListResponse {
        object: "list".to_string(),
        data: items,
        first_id,
        last_id,
        has_more: result.has_more,
    }))
}

/// Get a vector store file
///
/// Retrieves a file from a vector store.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/files/{file_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_get",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("file_id" = Uuid, Path, description = "Vector store file ID"),
    ),
    responses(
        (status = 200, description = "Vector store file details", body = VectorStoreFile),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_get_file(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path((vector_store_id, file_id)): Path<(VectorStoreId, FileId)>,
) -> Result<Json<VectorStoreFile>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Look up by file_id (Files API ID) + vector_store_id, not by vector_store_file.id
    let vector_store_file = services
        .vector_stores
        .find_by_file_id(vector_store_id, file_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!(
                    "File '{}' not found in vector store '{}'",
                    file_id, vector_store_id
                ),
            )
        })?;

    Ok(Json(vector_store_file))
}

/// Delete a vector store file
///
/// Removes a file from a vector store. This does not delete the underlying file
/// from the Files API.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/vector_stores/{vector_store_id}/files/{file_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_delete",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("file_id" = Uuid, Path, description = "Vector store file ID"),
    ),
    responses(
        (status = 200, description = "File removed from vector store", body = DeleteVectorStoreFileResponse),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_delete_file(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path((vector_store_id, file_id)): Path<(VectorStoreId, FileId)>,
) -> Result<Json<DeleteVectorStoreFileResponse>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    // Keep prefixed form for response
    let file_id_prefixed = file_id.to_string();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Look up by file_id (Files API ID) + vector_store_id, not by vector_store_file.id
    let vector_store_file = services
        .vector_stores
        .find_by_file_id(vector_store_id, file_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!(
                    "File '{}' not found in vector store '{}'",
                    file_id, vector_store_id
                ),
            )
        })?;

    // Remove the file from the vector store using vector_store_file.internal_id
    services
        .vector_stores
        .remove_file(vector_store_file.internal_id)
        .await?;

    Ok(Json(DeleteVectorStoreFileResponse {
        id: file_id_prefixed,
        object: "vector_store.file.deleted".to_string(),
        deleted: true,
    }))
}

// ============================================================================
// Vector Store File Batch Route Handlers (Stub implementations)
// ============================================================================

/// File batch response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileBatch {
    /// Batch ID
    pub id: String,
    /// Object type (always "vector_store.file_batch")
    pub object: String,
    /// Vector store ID
    pub vector_store_id: String,
    /// Batch status
    pub status: String,
    /// File counts by status
    pub file_counts: FileBatchCounts,
    /// Unix timestamp when batch was created
    pub created_at: i64,
}

/// File batch counts
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileBatchCounts {
    pub in_progress: i32,
    pub completed: i32,
    pub failed: i32,
    pub cancelled: i32,
    pub total: i32,
}

/// Create file batch request
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateFileBatchRequest {
    /// File IDs to add to the batch
    pub file_ids: Vec<Uuid>,
    /// Chunking strategy for all files in the batch
    #[serde(default)]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Create a file batch
///
/// Creates a batch of files to be added to a vector store.
/// Note: File batches are not yet fully implemented. This endpoint creates
/// files individually and returns a batch representation.
///
/// ## Content Deduplication
///
/// Files are deduplicated by content hash (SHA-256). If a file with identical content
/// already exists in the vector store, it is counted as "completed" in the batch
/// response but no re-processing occurs. This prevents duplicate chunks and wasted
/// compute while still reporting success for the file.
///
/// The `file_counts.completed` field in the response includes both newly processed
/// files and deduplicated files.
///
/// ## Embedding Model Validation
///
/// The gateway validates that its configured embedding model matches the vector store's
/// embedding model before processing any files in the batch. This prevents incompatible
/// embeddings from being stored together. If there's a mismatch, a 409 Conflict error
/// is returned with details about the expected vs. configured models.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_create",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = CreateFileBatchRequest,
    responses(
        (status = 201, description = "File batch created", body = FileBatch),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Embedding model mismatch", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search service not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_create_file_batch(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Json(input): Json<CreateFileBatchRequest>,
) -> Result<(StatusCode, Json<FileBatch>), ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Validate embedding model compatibility before processing any files.
    // This ensures the gateway's configured embedding model matches the vector store's model,
    // preventing incompatible vectors from being stored.
    validate_embedding_model_compatibility(&state, &vector_store)?;

    // Add each file to the vector store
    let mut completed = 0;
    let mut failed = 0;
    let mut duplicates = 0;

    for file_id in &input.file_ids {
        // Verify the file exists and get its content hash
        let file = match services.files.get(*file_id).await? {
            Some(f) => f,
            None => {
                failed += 1;
                continue;
            }
        };

        // Verify the user has access to the file being added
        if check_resource_access_optional(
            auth.as_ref().map(|e| &e.0),
            file.owner_type,
            file.owner_id,
        )
        .is_err()
        {
            tracing::warn!(
                file_id = %file_id,
                "Access denied to file in batch, skipping"
            );
            failed += 1;
            continue;
        }

        // Check if this file is already in the vector store (idempotency)
        if let Some(existing_file) = services
            .vector_stores
            .find_by_file_id(vector_store_id, *file_id)
            .await?
        {
            tracing::info!(
                vector_store_id = %vector_store_id,
                file_id = %file_id,
                vector_store_file_internal_id = %existing_file.internal_id,
                "File already in vector store in batch, skipping"
            );
            // Count as completed since the file is already in the vector store
            completed += 1;
            duplicates += 1;
            continue;
        }

        // Check for same-owner content deduplication (prevents accidental duplicates)
        if let Some(content_hash) = &file.content_hash
            && let Some(existing_file) = services
                .vector_stores
                .find_by_content_hash_and_owner(
                    vector_store_id,
                    content_hash,
                    file.owner_type,
                    file.owner_id,
                )
                .await?
        {
            tracing::info!(
                vector_store_id = %vector_store_id,
                file_id = %file_id,
                existing_file_id = %existing_file.file_id,
                vector_store_file_internal_id = %existing_file.internal_id,
                content_hash = %content_hash,
                "Same-owner duplicate content in batch, skipping"
            );
            // Count as completed since equivalent content is already in the vector store
            completed += 1;
            duplicates += 1;
            continue;
        }

        let add_input = AddFileToVectorStore {
            vector_store_id,
            file_id: *file_id,
            chunking_strategy: input.chunking_strategy.clone(),
            attributes: None,
        };

        match services.vector_stores.add_file(add_input).await {
            Ok(_vector_store_file) => {
                completed += 1;
                // Trigger file processing
                #[cfg(any(
                    feature = "document-extraction-basic",
                    feature = "document-extraction-full"
                ))]
                if let Some(processor) = &state.document_processor {
                    let processor = processor.clone();
                    if let Err(e) = processor
                        .schedule_processing(_vector_store_file.internal_id)
                        .await
                    {
                        tracing::error!(
                            error = %e,
                            internal_id = %_vector_store_file.internal_id,
                            "Failed to schedule file processing in batch"
                        );
                    }
                }
            }
            Err(_) => failed += 1,
        }
    }

    if duplicates > 0 {
        tracing::info!(
            vector_store_id = %vector_store_id,
            duplicates = duplicates,
            "Batch contained duplicate files that were skipped"
        );
    }

    let total = input.file_ids.len() as i32;
    let batch_id = Uuid::new_v4();

    Ok((
        StatusCode::CREATED,
        Json(FileBatch {
            id: format!("vsfb_{}", batch_id),
            object: "vector_store.file_batch".to_string(),
            vector_store_id: vector_store_id.to_string(),
            status: if failed == 0 { "completed" } else { "failed" }.to_string(),
            file_counts: FileBatchCounts {
                in_progress: 0,
                completed,
                failed,
                cancelled: 0,
                total,
            },
            created_at: vector_store.created_at.timestamp(),
        }),
    ))
}

/// Get a file batch
///
/// Retrieves a file batch. Note: File batches are executed synchronously,
/// so this endpoint returns a "completed" or "failed" status.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_get",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("batch_id" = String, Path, description = "File batch ID"),
    ),
    responses(
        (status = 404, description = "File batches are not persisted", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(_state))]
pub async fn api_v1_vector_stores_get_file_batch(
    State(_state): State<AppState>,
    Path((_vector_store_id, _batch_id)): Path<(VectorStoreId, String)>,
) -> Result<Json<FileBatch>, ApiError> {
    // File batches are executed synchronously and not persisted
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "File batches are not persisted. Use the create endpoint which returns the final status.",
    ))
}

/// Cancel a file batch
///
/// Cancels a file batch. Note: File batches are executed synchronously,
/// so cancellation is not supported.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_cancel",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("batch_id" = String, Path, description = "File batch ID"),
    ),
    responses(
        (status = 400, description = "File batches cannot be cancelled", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(_state))]
pub async fn api_v1_vector_stores_cancel_file_batch(
    State(_state): State<AppState>,
    Path((_vector_store_id, _batch_id)): Path<(VectorStoreId, String)>,
) -> Result<Json<FileBatch>, ApiError> {
    Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "not_supported",
        "File batches are executed synchronously and cannot be cancelled.",
    ))
}

/// List files in a batch
///
/// Lists files in a file batch. Note: File batches are not persisted.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}/files",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_list_files",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("batch_id" = String, Path, description = "File batch ID"),
    ),
    responses(
        (status = 404, description = "File batches are not persisted", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(_state))]
pub async fn api_v1_vector_stores_list_batch_files(
    State(_state): State<AppState>,
    Path((_vector_store_id, _batch_id)): Path<(VectorStoreId, String)>,
) -> Result<Json<VectorStoreFileListResponse>, ApiError> {
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "File batches are not persisted. List the vector store files directly using GET /v1/vector_stores/{id}/files",
    ))
}

// ============================================================================
// Hadrian Extensions - Chunk and Search Endpoints
// ============================================================================

/// A stored chunk as returned by the chunks endpoint.
///
/// ## OpenAI Compatibility Notes
///
/// - `id` is serialized with `chunk_` prefix (e.g., `chunk_550e8400-e29b-41d4-a716-446655440000`)
/// - `vector_store_id` is serialized with `vs_` prefix
/// - `file_id` is serialized with `file-` prefix
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChunkResponse {
    /// Unique identifier for this chunk (serialized with `chunk_` prefix)
    #[serde(with = "chunk_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "chunk_550e8400-e29b-41d4-a716-446655440000"))]
    pub id: Uuid,
    /// Object type (always "vector_store.file.chunk")
    pub object: String,
    /// The vector store this chunk belongs to (serialized with `vs_` prefix)
    #[serde(with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub vector_store_id: Uuid,
    /// The file this chunk was extracted from (serialized with `file-` prefix)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// Sequential index within the file (0-based)
    pub chunk_index: i32,
    /// The actual text content of the chunk
    pub content: String,
    /// Number of tokens in this chunk
    pub token_count: i32,
    /// Character offset where this chunk starts in the original file
    pub char_start: i32,
    /// Character offset where this chunk ends in the original file
    pub char_end: i32,
    /// Optional additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Unix timestamp when the chunk was created
    pub created_at: i64,
}

/// Paginated list of chunks response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChunkListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of chunks
    pub data: Vec<ChunkResponse>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

/// Search request for a vector store.
///
/// ## Ranking Options
///
/// Use `ranking_options` to control result scoring and filtering:
/// - `ranker`: Algorithm for ranking results
///   - `auto` (default): Automatically selects best ranker; supports hybrid search
///   - `vector`: Vector-only cosine similarity search
///   - `hybrid`: Combines vector and keyword search with RRF fusion
///   - `llm`: LLM-based re-ranking for highest quality results
///   - `none`: No re-ranking, raw similarity order
/// - `score_threshold`: Minimum similarity score (0.0-1.0, default: 0.0)
/// - `hybrid_search`: Enable hybrid search combining vector and keyword search
///   - `embedding_weight`: Weight for semantic (vector) search (default: 1.0)
///   - `text_weight`: Weight for keyword (full-text) search (default: 1.0)
///
/// ## Hybrid Search Example
///
/// ```json
/// {
///   "query": "API authentication",
///   "ranking_options": {
///     "ranker": "hybrid",
///     "score_threshold": 0.5,
///     "hybrid_search": {
///       "embedding_weight": 0.7,
///       "text_weight": 0.3
///     }
///   }
/// }
/// ```
///
/// ## LLM Re-ranking Example
///
/// ```json
/// {
///   "query": "How to authenticate API requests",
///   "ranking_options": {
///     "ranker": "llm",
///     "score_threshold": 0.5
///   }
/// }
/// ```
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreSearchRequest {
    /// The search query text.
    pub query: String,

    /// Maximum number of results to return (default: 10, max: 50).
    #[serde(default)]
    pub max_num_results: Option<usize>,

    /// Ranking options for controlling result scoring and filtering.
    ///
    /// If not specified, uses default ranking with score_threshold of 0.0 (return all results).
    #[serde(default)]
    pub ranking_options: Option<FileSearchRankingOptions>,

    /// A filter to apply based on file attributes. Supports comparison operators
    /// (eq, ne, gt, gte, lt, lte) and logical operators (and, or) for combining filters.
    ///
    /// Example: `{"type": "eq", "key": "category", "value": "documentation"}`
    #[serde(default)]
    pub filters: Option<AttributeFilter>,
}

/// A single search result.
///
/// ## Hadrian Extensions
///
/// The following fields are **Hadrian extensions** not present in the standard OpenAI API:
/// - `chunk_id`: Unique identifier for the matched chunk
/// - `object`: Object type identifier
/// - `vector_store_id`: Vector store ID the chunk belongs to
/// - `chunk_index`: Position of chunk within the source file
/// - `metadata`: Arbitrary metadata (OpenAI uses `attributes`)
///
/// ## OpenAI Compatibility Notes
///
/// - `chunk_id` is serialized with `chunk_` prefix
/// - `vector_store_id` is serialized with `vs_` prefix
/// - `file_id` is serialized with `file-` prefix
/// - `content` is a string; OpenAI uses `content: [{type: "text", text: "..."}]` array format
/// - `filename` is optional; OpenAI requires it
/// - `metadata` maps to OpenAI's `attributes` field
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SearchResultItem {
    /// **Hadrian Extension:** The chunk ID in the vector store (serialized with `chunk_` prefix)
    #[serde(with = "chunk_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "chunk_550e8400-e29b-41d4-a716-446655440000"))]
    pub chunk_id: Uuid,
    /// **Hadrian Extension:** Object type (always "vector_store.search_result")
    pub object: String,
    /// **Hadrian Extension:** The vector store this chunk belongs to (serialized with `vs_` prefix)
    #[serde(with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub vector_store_id: Uuid,
    /// The file this chunk was extracted from (serialized with `file-` prefix)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// **Hadrian Extension:** Index of this chunk within the file
    pub chunk_index: i32,
    /// The actual text content of the chunk. Note: OpenAI uses array format `[{type, text}]`.
    pub content: String,
    /// Similarity score (0.0 to 1.0, higher is more similar)
    pub score: f64,
    /// Filename of the source file. Note: Required in OpenAI, optional in Hadrian.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// **Hadrian Extension:** Optional additional metadata. Note: OpenAI uses `attributes`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Search response from a vector store.
///
/// ## OpenAI Compatibility Notes
///
/// - `object` is "vector_store.search_results"; OpenAI uses "vector_store.search_results.page"
/// - `query` is a string; OpenAI uses `search_query` as an array of strings
/// - `has_more` and `next_page` pagination fields are not yet supported
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreSearchResponse {
    /// Object type. Note: OpenAI uses "vector_store.search_results.page".
    pub object: String,
    /// **Hadrian Extension:** The search query that was used. Note: OpenAI uses `search_query` as an array.
    pub query: String,
    /// Search results ordered by relevance (highest first)
    pub data: Vec<SearchResultItem>,
}

/// List chunks for a file
///
/// **Hadrian Extension** - This endpoint is not part of the OpenAI API.
///
/// Returns all chunks that have been extracted and embedded from a file.
/// This is useful for debugging chunking behavior and verifying embeddings.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/files/{file_id}/chunks",
    tag = "vector-stores",
    operation_id = "vector_store_file_chunks_list",
    summary = "List chunks for a file [Hadrian Extension]",
    description = "**Hadrian Extension** - This endpoint is not part of the standard OpenAI API.\n\nReturns all chunks that have been extracted and embedded from a file. Useful for debugging chunking behavior and verifying embeddings.",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("file_id" = Uuid, Path, description = "Vector store file ID"),
    ),
    responses(
        (status = 200, description = "List of chunks for the file", body = ChunkListResponse),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_list_file_chunks(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path((vector_store_id, file_id)): Path<(VectorStoreId, FileId)>,
) -> Result<Json<ChunkListResponse>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Look up by file_id (Files API ID) + vector_store_id, not by vector_store_file.id
    let vector_store_file = services
        .vector_stores
        .find_by_file_id(vector_store_id, file_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!(
                    "File '{}' not found in vector store '{}'",
                    file_id, vector_store_id
                ),
            )
        })?;

    // Get the file search service (which has access to the vector store)
    let file_search_service = state.file_search_service.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "not_configured",
            "File search is not configured. Enable [features.file_search] in configuration.",
        )
    })?;

    // Get chunks from the vector store
    // Note: chunks are stored by the underlying file_id, not the vector_store_file ID
    let chunks = file_search_service
        .get_chunks_by_file(vector_store_file.file_id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                format!("Failed to retrieve chunks: {}", e),
            )
        })?;

    let data: Vec<ChunkResponse> = chunks
        .into_iter()
        .map(|c| ChunkResponse {
            id: c.id,
            object: "vector_store.file.chunk".to_string(),
            vector_store_id: c.vector_store_id,
            file_id: c.file_id,
            chunk_index: c.chunk_index,
            content: c.content,
            token_count: c.token_count,
            char_start: c.char_start,
            char_end: c.char_end,
            metadata: c.metadata,
            created_at: c.created_at,
        })
        .collect();

    let total = data.len() as i64;
    let pagination = PaginationMeta::with_cursors(total, false, None, None);

    Ok(Json(ChunkListResponse {
        object: "list".to_string(),
        data,
        pagination,
    }))
}

/// Search a vector store
///
/// Performs a semantic search against a vector store (OpenAI-compatible endpoint).
/// Note: Request/response schema has Hadrian-specific extensions.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}/search",
    tag = "vector-stores",
    operation_id = "vector_store_search",
    summary = "Search vector store",
    description = "Performs a semantic search against a vector store.\n\n**Hadrian Extensions:** The response schema includes additional fields not in the standard OpenAI API:\n- `chunk_id`, `vector_store_id`, `chunk_index` (debugging info)",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = VectorStoreSearchRequest,
    responses(
        (status = 200, description = "Search results", body = VectorStoreSearchResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_search(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Json(input): Json<VectorStoreSearchRequest>,
) -> Result<Json<VectorStoreSearchResponse>, ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "search",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Get the file search service
    let file_search_service = state.file_search_service.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "not_configured",
            "File search is not configured. Enable [features.file_search] in configuration.",
        )
    })?;

    // Extract and validate score_threshold
    let score_threshold = input.ranking_options.as_ref().map(|r| r.score_threshold);
    if let Some(threshold) = score_threshold
        && !(0.0..=1.0).contains(&threshold)
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_parameter",
            format!(
                "score_threshold must be between 0.0 and 1.0, got {}",
                threshold
            ),
        ));
    }

    let search_request = crate::services::FileSearchRequest {
        query: input.query.clone(),
        vector_store_ids: vec![vector_store_id],
        max_results: input.max_num_results,
        threshold: score_threshold,
        file_ids: None,
        filters: input.filters,
        ranking_options: input.ranking_options,
    };

    // Execute search
    let search_response = file_search_service
        .search(search_request, None)
        .await
        .map_err(|e| match e {
            crate::services::FileSearchError::VectorStoreNotFound(id) => ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("VectorStore '{}' not found", id),
            ),
            crate::services::FileSearchError::EmbeddingError(msg) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "embedding_error",
                format!("Embedding error: {}", msg),
            ),
            crate::services::FileSearchError::SearchError(msg) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "search_error",
                format!("Search error: {}", msg),
            ),
            crate::services::FileSearchError::NotConfigured => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "not_configured",
                "File search is not configured",
            ),
            _ => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                e.to_string(),
            ),
        })?;

    let data: Vec<SearchResultItem> = search_response
        .results
        .into_iter()
        .map(|r| SearchResultItem {
            chunk_id: r.chunk_id,
            object: "vector_store.search_result".to_string(),
            vector_store_id: r.vector_store_id,
            file_id: r.file_id,
            chunk_index: r.chunk_index,
            content: r.content,
            score: r.score,
            filename: r.filename,
            metadata: r.metadata,
        })
        .collect();

    Ok(Json(VectorStoreSearchResponse {
        object: "vector_store.search_results".to_string(),
        query: input.query,
        data,
    }))
}
