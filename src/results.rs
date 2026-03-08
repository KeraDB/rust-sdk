//! MongoDB-compatible result types for KeraDB operations.

// @group ResultTypes : Operation result structs matching MongoDB driver conventions

/// Result of an `insert_one` operation.
#[derive(Debug, Clone)]
pub struct InsertOneResult {
    /// The string ID that was assigned to the inserted document.
    pub inserted_id: String,
}

impl InsertOneResult {
    /// Create a new result with the given document ID.
    pub fn new(inserted_id: impl Into<String>) -> Self {
        Self {
            inserted_id: inserted_id.into(),
        }
    }
}

impl std::fmt::Display for InsertOneResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InsertOneResult(inserted_id='{}')", self.inserted_id)
    }
}

/// Result of an `insert_many` operation.
#[derive(Debug, Clone)]
pub struct InsertManyResult {
    /// The list of string IDs assigned to all inserted documents, in insertion order.
    pub inserted_ids: Vec<String>,
}

impl InsertManyResult {
    /// Create a new result with the given list of document IDs.
    pub fn new(inserted_ids: Vec<String>) -> Self {
        Self { inserted_ids }
    }
}

impl std::fmt::Display for InsertManyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InsertManyResult(inserted_ids={:?})", self.inserted_ids)
    }
}

/// Result of an `update_one` or `update_many` operation.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    /// Number of documents that matched the filter.
    pub matched_count: usize,
    /// Number of documents that were actually modified.
    pub modified_count: usize,
}

impl UpdateResult {
    /// Create a new result.
    pub fn new(matched_count: usize, modified_count: usize) -> Self {
        Self {
            matched_count,
            modified_count,
        }
    }
}

impl std::fmt::Display for UpdateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UpdateResult(matched={}, modified={})",
            self.matched_count, self.modified_count
        )
    }
}

/// Result of a `delete_one` or `delete_many` operation.
#[derive(Debug, Clone)]
pub struct DeleteResult {
    /// Number of documents that were deleted.
    pub deleted_count: usize,
}

impl DeleteResult {
    /// Create a new result.
    pub fn new(deleted_count: usize) -> Self {
        Self { deleted_count }
    }
}

impl std::fmt::Display for DeleteResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeleteResult(deleted_count={})", self.deleted_count)
    }
}

// ---------------------------------------------------------------------------
// @group UnitTests : Result type construction and display
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_one_result_stores_id() {
        let r = InsertOneResult::new("abc-123");
        assert_eq!(r.inserted_id, "abc-123");
    }

    #[test]
    fn insert_one_result_display() {
        let r = InsertOneResult::new("abc-123");
        assert_eq!(r.to_string(), "InsertOneResult(inserted_id='abc-123')");
    }

    #[test]
    fn insert_many_result_stores_ids() {
        let r = InsertManyResult::new(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(r.inserted_ids.len(), 3);
        assert_eq!(r.inserted_ids[1], "b");
    }

    #[test]
    fn insert_many_result_display() {
        let r = InsertManyResult::new(vec!["x".into()]);
        assert!(r.to_string().contains("InsertManyResult"));
    }

    #[test]
    fn update_result_counts() {
        let r = UpdateResult::new(5, 3);
        assert_eq!(r.matched_count, 5);
        assert_eq!(r.modified_count, 3);
    }

    #[test]
    fn update_result_display() {
        let r = UpdateResult::new(5, 3);
        assert_eq!(r.to_string(), "UpdateResult(matched=5, modified=3)");
    }

    #[test]
    fn delete_result_count() {
        let r = DeleteResult::new(4);
        assert_eq!(r.deleted_count, 4);
    }

    #[test]
    fn delete_result_display() {
        let r = DeleteResult::new(4);
        assert_eq!(r.to_string(), "DeleteResult(deleted_count=4)");
    }

    #[test]
    fn update_result_no_matches() {
        let r = UpdateResult::new(0, 0);
        assert_eq!(r.matched_count, 0);
        assert_eq!(r.modified_count, 0);
    }
}
