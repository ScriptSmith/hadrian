use uuid::Uuid;

use crate::db::error::{DbError, DbResult};

/// Parse a UUID string from the database, returning a DbError on failure
pub fn parse_uuid(s: &str) -> DbResult<Uuid> {
    Uuid::parse_str(s).map_err(|e| DbError::Internal(format!("Invalid UUID in database: {}", e)))
}
