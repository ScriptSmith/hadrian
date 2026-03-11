#[cfg(feature = "server")]
use axum::extract::Multipart;
use axum::{
    Extension, Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiError, SortOrder, check_resource_access_optional, get_services};
use crate::{
    AppState,
    auth::AuthenticatedRequest,
    db::ListParams,
    middleware::AuthzContext,
    models::{File, FileId, FilePurpose, VectorStoreOwnerType},
    services::FilesService,
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListFilesQuery {
    /// Maximum number of files to return (default: 20, max: 100)
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 100))]
    pub limit: Option<i64>,
    /// Sort order by `created_at` timestamp (default: desc)
    #[serde(default)]
    pub order: Option<SortOrder>,
    /// Cursor for forward pagination. Returns results after this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "file-550e8400-e29b-41d4-a716-446655440000")
    )]
    pub after: Option<String>,
    /// **Hadrian Extension:** Cursor for backward pagination. Returns results before this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "file-550e8400-e29b-41d4-a716-446655440000")
    )]
    pub before: Option<String>,
    /// Filter by purpose
    #[cfg_attr(feature = "utoipa", param(example = "assistants"))]
    pub purpose: Option<String>,
    /// **Hadrian Extension:** Owner type for multi-tenancy (organization, project, or user)
    pub owner_type: String,
    /// **Hadrian Extension:** Owner ID for multi-tenancy
    pub owner_id: Uuid,
}

/// Paginated list of files response (OpenAI-compatible).
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of files
    pub data: Vec<File>,
    /// ID of the first file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    /// ID of the last file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    /// Whether there are more results available
    pub has_more: bool,
}

/// Delete file response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteFileResponse {
    /// File ID that was deleted
    pub id: String,
    /// Object type (always "file")
    pub object: String,
    /// Whether the file was deleted
    pub deleted: bool,
}

#[cfg(feature = "server")]
/// Upload a file
///
/// Uploads a file that can be used with vector stores for RAG.
/// Files are uploaded as multipart/form-data with the following fields:
/// - `file`: The file to upload (required)
/// - `purpose`: The intended purpose of the file (default: "assistants")
/// - `owner_type`: Owner type - "organization", "project", or "user" (required)
/// - `owner_id`: Owner ID (required)
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/files",
    tag = "files",
    operation_id = "file_upload",
    request_body(content_type = "multipart/form-data", description = "File upload with metadata"),
    responses(
        (status = 200, description = "File uploaded successfully", body = File),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 413, description = "File too large", body = crate::openapi::ErrorResponse),
        (status = 422, description = "Virus detected in uploaded file", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz, multipart), fields(purpose))]
pub async fn api_v1_files_upload(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    mut multipart: Multipart,
) -> Result<Json<File>, ApiError> {
    // Check file upload permission via CEL policies
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
                "file",
                "upload",
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

    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut purpose = FilePurpose::Assistants;
    let mut owner_type: Option<VectorStoreOwnerType> = None;
    let mut owner_id: Option<Uuid> = None;

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart_error",
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                content_type = field.content_type().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            ApiError::new(
                                StatusCode::BAD_REQUEST,
                                "file_read_error",
                                format!("Failed to read file: {}", e),
                            )
                        })?
                        .to_vec(),
                );
            }
            "purpose" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "purpose_read_error",
                        format!("Failed to read purpose: {}", e),
                    )
                })?;
                purpose = value.parse().map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_purpose",
                        format!("Invalid purpose: {}", value),
                    )
                })?;
            }
            "owner_type" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "owner_type_read_error",
                        format!("Failed to read owner_type: {}", e),
                    )
                })?;
                owner_type = Some(value.parse().map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_owner_type",
                        format!("Invalid owner_type: {}", value),
                    )
                })?);
            }
            "owner_id" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "owner_id_read_error",
                        format!("Failed to read owner_id: {}", e),
                    )
                })?;
                owner_id = Some(Uuid::parse_str(&value).map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_owner_id",
                        format!("Invalid owner_id: {}", value),
                    )
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_file",
            "Missing required field: file",
        )
    })?;
    let filename = filename.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_filename",
            "Missing filename in file field",
        )
    })?;
    let owner_type = owner_type.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_owner_type",
            "Missing required field: owner_type",
        )
    })?;
    let owner_id = owner_id.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_owner_id",
            "Missing required field: owner_id",
        )
    })?;

    // Validate file size against configured limit
    let max_file_size = state.config.features.file_processing.max_file_size_bytes();
    let file_size = file_data.len() as i64;
    if file_size > max_file_size {
        let max_mb = state.config.features.file_processing.max_file_size_mb;
        let file_mb = file_size as f64 / (1024.0 * 1024.0);
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
            format!(
                "File size ({:.2} MB) exceeds maximum allowed size ({} MB)",
                file_mb, max_mb
            ),
        ));
    }

    // Validate file type based on purpose (extension check)
    if let Err(msg) = purpose.validate_file_extension(&filename) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_file_type",
            msg,
        ));
    }

    // Validate file content magic bytes match declared type
    if let Err(msg) = purpose.validate_file_content(&file_data) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_file_content",
            msg,
        ));
    }

    // Virus scan if enabled
    #[cfg(feature = "virus-scan")]
    {
        let virus_scan_config = &state.config.features.file_processing.virus_scan;
        if virus_scan_config.enabled {
            use crate::services::{ClamAvScanner, VirusScanner};

            let clamav_config = virus_scan_config.clamav.clone().unwrap_or_default();
            let scanner = ClamAvScanner::new(clamav_config).map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "virus_scan_config_error",
                    format!("Failed to initialize virus scanner: {}", e),
                )
            })?;

            let scan_result = scanner.scan(&file_data).await.map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "virus_scan_error",
                    format!("Virus scan failed: {}", e),
                )
            })?;

            if !scan_result.is_clean {
                let threat_name = scan_result
                    .threat_name
                    .unwrap_or_else(|| "Unknown".to_string());
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "virus_detected",
                    format!("File rejected: malware detected ({})", threat_name),
                ));
            }
        }
    }

    // Validate that the owner exists
    let db = state.db.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_not_configured",
            "Database not configured",
        )
    })?;

    let owner_type_name = match owner_type {
        VectorStoreOwnerType::User => "User",
        VectorStoreOwnerType::Organization => "Organization",
        VectorStoreOwnerType::Team => "Team",
        VectorStoreOwnerType::Project => "Project",
    };

    let owner_exists = match owner_type {
        VectorStoreOwnerType::User => {
            let result: Option<crate::models::User> =
                db.users().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
        VectorStoreOwnerType::Organization => {
            let result: Option<crate::models::Organization> =
                db.organizations().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
        VectorStoreOwnerType::Team => {
            let result: Option<crate::models::Team> =
                db.teams().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
        VectorStoreOwnerType::Project => {
            let result: Option<crate::models::Project> =
                db.projects().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
    };

    if !owner_exists {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "owner_not_found",
            format!("{} with ID {} not found", owner_type_name, owner_id),
        ));
    }

    // Check file limit per owner
    let max = state.config.limits.resource_limits.max_files_per_owner;
    if max > 0 {
        let count = services
            .files
            .count_by_owner(owner_type, owner_id)
            .await
            .map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "count_error",
                    format!("Failed to count files: {}", e),
                )
            })?;
        if count >= max as i64 {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "limit_exceeded",
                format!(
                    "{} has reached the maximum number of files ({max})",
                    owner_type_name
                ),
            ));
        }
    }

    // Create file with configured storage backend
    let storage_backend = services.files.configured_backend();
    let input = FilesService::create_file_input(
        owner_type,
        owner_id,
        filename,
        purpose,
        content_type,
        file_data,
        storage_backend,
    );

    let file = services.files.upload(input).await?;
    Ok(Json(file))
}

/// List files
///
/// Returns a list of files owned by the specified owner.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/files",
    tag = "files",
    operation_id = "file_list",
    params(ListFilesQuery),
    responses(
        (status = 200, description = "List of files", body = FileListResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_list(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<FileListResponse>, ApiError> {
    use crate::db::repos::{Cursor, CursorDirection};

    // Check file list permission via CEL policies
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
                "file",
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

    let owner_type: VectorStoreOwnerType = query.owner_type.parse().map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_owner_type",
            "Invalid owner_type",
        )
    })?;

    let purpose = query
        .purpose
        .map(|p| {
            p.parse::<FilePurpose>().map_err(|_| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_purpose",
                    format!("Invalid purpose: {}", p),
                )
            })
        })
        .transpose()?;

    // OpenAI defaults: limit=20
    let limit = query.limit.unwrap_or(20).min(100);

    // Parse cursor from `after` or `before` parameter
    let (cursor, direction) = if let Some(ref after_id) = query.after {
        let file_id: FileId = after_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'after' cursor: {}", after_id),
            )
        })?;

        let cursor_record = services
            .files
            .get(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("File '{}' not found for cursor", after_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.created_at, cursor_record.id)),
            CursorDirection::Forward,
        )
    } else if let Some(ref before_id) = query.before {
        let file_id: FileId = before_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'before' cursor: {}", before_id),
            )
        })?;

        let cursor_record = services
            .files
            .get(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("File '{}' not found for cursor", before_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.created_at, cursor_record.id)),
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
        .files
        .list(owner_type, query.owner_id, purpose, params)
        .await?;

    // Build OpenAI-compatible response
    let first_id = result.items.first().map(|f| FileId::new(f.id).to_string());
    let last_id = result.items.last().map(|f| FileId::new(f.id).to_string());

    Ok(Json(FileListResponse {
        object: "list".to_string(),
        data: result.items,
        first_id,
        last_id,
        has_more: result.has_more,
    }))
}

/// Get file metadata
///
/// Returns information about a specific file.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/files/{file_id}",
    tag = "files",
    operation_id = "file_get",
    params(("file_id" = Uuid, Path, description = "File ID")),
    responses(
        (status = 200, description = "File metadata", body = File),
        (status = 404, description = "File not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_get(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(file_id): Path<FileId>,
) -> Result<Json<File>, ApiError> {
    // Check file read permission via CEL policies
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
                "file",
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

    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    let file = services.files.get(file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", file_id),
        )
    })?;

    // Check access permission
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    Ok(Json(file))
}

/// Get file content
///
/// Returns the content of a file.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/files/{file_id}/content",
    tag = "files",
    operation_id = "file_get_content",
    params(("file_id" = Uuid, Path, description = "File ID")),
    responses(
        (status = 200, description = "File content", content_type = "application/octet-stream"),
        (status = 404, description = "File not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_get_content(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(file_id): Path<FileId>,
) -> Result<Response, ApiError> {
    // Check file read permission via CEL policies
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
                "file",
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

    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Get file metadata first (for content-type and filename)
    let file = services.files.get(file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", file_id),
        )
    })?;

    // Check access permission
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    // Get content from the appropriate storage backend
    let content = services.files.get_content(file_id).await?;

    let content_type = file
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file.filename),
            ),
        ],
        Bytes::from(content),
    )
        .into_response())
}

/// Delete a file
///
/// Deletes a file. The file cannot be deleted if it is still referenced by any vector stores.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/files/{file_id}",
    tag = "files",
    operation_id = "file_delete",
    params(("file_id" = Uuid, Path, description = "File ID")),
    responses(
        (status = 200, description = "File deleted", body = DeleteFileResponse),
        (status = 400, description = "File is still in use", body = crate::openapi::ErrorResponse),
        (status = 404, description = "File not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_delete(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(file_id): Path<FileId>,
) -> Result<Json<DeleteFileResponse>, ApiError> {
    // Check file delete permission via CEL policies
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
                "file",
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

    // Keep prefixed ID for response formatting
    let file_id_prefixed = file_id.to_string();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Check if file exists
    let file = services.files.get(file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", file_id),
        )
    })?;

    // Check access permission
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    // Check if file is still referenced (active references only, not soft-deleted)
    let ref_count = services.files.count_references(file_id).await?;
    if ref_count > 0 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "file_in_use",
            format!("File is still referenced by {} vector store(s)", ref_count),
        ));
    }

    // Clean up any soft-deleted references to avoid FK constraint violations
    services
        .vector_stores
        .cleanup_soft_deleted_references(file_id)
        .await?;

    // Delete the file
    services.files.delete(file_id).await?;

    Ok(Json(DeleteFileResponse {
        id: file_id_prefixed,
        object: "file".to_string(),
        deleted: true,
    }))
}
