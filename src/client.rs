//! MongoDB-compatible client, database, and collection types.
//!
//! Mirrors the Python SDK's `client.py` and the vector operations in `vector.py`.

// @group QueryHelpers   : Client-side filter matching and update application
// @group Cursor         : Lazy result iterator with limit / skip
// @group Collection     : CRUD operations on a document collection
// @group Database       : Database handle and collection access
// @group Client         : Top-level entry point and lifecycle management
// @group VectorOps      : Vector collection management and search

use std::sync::Arc;

use serde_json::{json, Value};

use crate::{
    error::{KeraDbError, Result},
    ffi::{get_ffi, DbHandle, KeraDbFfi},
    results::{DeleteResult, InsertManyResult, InsertOneResult, UpdateResult},
    vector::{
        MetadataFilter, VectorCollectionInfo, VectorCollectionStats,
        VectorConfig, VectorDocument, VectorSearchResult,
    },
};

// ---------------------------------------------------------------------------
// @group QueryHelpers : Client-side filter matching and update application
// ---------------------------------------------------------------------------

/// Return `true` if `doc` satisfies the MongoDB-style `filter`.
///
/// Supported: `$and`, `$or`, `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`,
/// `$in`, `$nin`, and direct equality.
pub fn matches_filter(doc: &Value, filter: &Value) -> bool {
    let filter_obj = match filter.as_object() {
        Some(o) => o,
        None => return true,
    };

    for (key, value) in filter_obj {
        match key.as_str() {
            // Logical operators
            "$and" => {
                if let Some(conditions) = value.as_array() {
                    if !conditions.iter().all(|c| matches_filter(doc, c)) {
                        return false;
                    }
                }
            }
            "$or" => {
                if let Some(conditions) = value.as_array() {
                    if !conditions.iter().any(|c| matches_filter(doc, c)) {
                        return false;
                    }
                }
            }
            // Field comparisons
            field => {
                let doc_val = &doc[field];
                if let Some(ops) = value.as_object() {
                    for (op, op_val) in ops {
                        let matched = match op.as_str() {
                            "$eq" => doc_val == op_val,
                            "$ne" => doc_val != op_val,
                            "$gt" => cmp_values(doc_val, op_val) == Some(std::cmp::Ordering::Greater),
                            "$gte" => matches!(
                                cmp_values(doc_val, op_val),
                                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                            ),
                            "$lt" => cmp_values(doc_val, op_val) == Some(std::cmp::Ordering::Less),
                            "$lte" => matches!(
                                cmp_values(doc_val, op_val),
                                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                            ),
                            "$in" => {
                                if let Some(arr) = op_val.as_array() {
                                    arr.contains(doc_val)
                                } else {
                                    false
                                }
                            }
                            "$nin" => {
                                if let Some(arr) = op_val.as_array() {
                                    !arr.contains(doc_val)
                                } else {
                                    true
                                }
                            }
                            _ => true, // unknown operators pass through
                        };
                        if !matched {
                            return false;
                        }
                    }
                } else {
                    // Direct equality
                    if doc_val != value {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Compare two JSON values numerically.  Returns `None` when comparison is
/// not meaningful (e.g. comparing a string to a number).
fn cmp_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a.as_f64(), b.as_f64()) {
        (Some(av), Some(bv)) => av.partial_cmp(&bv),
        _ => None,
    }
}

/// Apply MongoDB-style update operators (`$set`, `$unset`, `$inc`, `$push`)
/// to `doc`, returning the modified document.  If no operators are present
/// the document is replaced (preserving `_id`).
pub fn apply_update(doc: &Value, update: &Value) -> Value {
    let doc_obj = match doc.as_object() {
        Some(o) => o,
        None => return doc.clone(),
    };
    let update_obj = match update.as_object() {
        Some(o) => o,
        None => return doc.clone(),
    };

    let has_operators = update_obj.keys().any(|k| k.starts_with('$'));

    if !has_operators {
        // Replacement – keep original `_id`
        let mut result = update_obj.clone();
        if let Some(id) = doc_obj.get("_id") {
            result.insert("_id".to_owned(), id.clone());
        }
        return Value::Object(result);
    }

    let mut result = doc_obj.clone();

    for (op, fields) in update_obj {
        match op.as_str() {
            "$set" => {
                if let Some(obj) = fields.as_object() {
                    for (k, v) in obj {
                        result.insert(k.clone(), v.clone());
                    }
                }
            }
            "$unset" => {
                if let Some(obj) = fields.as_object() {
                    for k in obj.keys() {
                        result.remove(k);
                    }
                }
            }
            "$inc" => {
                if let Some(obj) = fields.as_object() {
                    for (k, v) in obj {
                        let current = result.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
                        let delta = v.as_f64().unwrap_or(0.0);
                        // Use integer if both values are integers
                        let new_val = if current.fract() == 0.0 && delta.fract() == 0.0 {
                            json!((current + delta) as i64)
                        } else {
                            json!(current + delta)
                        };
                        result.insert(k.clone(), new_val);
                    }
                }
            }
            "$push" => {
                if let Some(obj) = fields.as_object() {
                    for (k, v) in obj {
                        let arr = result
                            .entry(k.clone())
                            .or_insert_with(|| Value::Array(vec![]));
                        if let Some(a) = arr.as_array_mut() {
                            a.push(v.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Value::Object(result)
}

// ---------------------------------------------------------------------------
// @group Cursor : Lazy result iterator with limit / skip
// ---------------------------------------------------------------------------

/// A cursor holding query results that supports `limit()`, `skip()`, and
/// iteration – mirroring Python's `Cursor` class.
pub struct Cursor {
    documents: Vec<Value>,
    limit: Option<usize>,
    skip: usize,
}

impl Cursor {
    /// Create a cursor from a pre-fetched document list.
    pub fn new(documents: Vec<Value>) -> Self {
        Self {
            documents,
            limit: None,
            skip: 0,
        }
    }

    /// Limit the number of results returned.
    pub fn limit(mut self, count: usize) -> Self {
        self.limit = Some(count);
        self
    }

    /// Skip the first `count` results.
    pub fn skip(mut self, count: usize) -> Self {
        self.skip = count;
        self
    }

    /// Consume the cursor and return all matching documents as a `Vec`.
    pub fn all(self) -> Vec<Value> {
        let docs = &self.documents[self.skip.min(self.documents.len())..];
        match self.limit {
            Some(n) => docs.iter().take(n).cloned().collect(),
            None => docs.to_vec(),
        }
    }

    /// Return the first document, or `None` if the cursor is empty.
    pub fn first(self) -> Option<Value> {
        self.limit(1).all().into_iter().next()
    }
}

impl IntoIterator for Cursor {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.all().into_iter()
    }
}

// ---------------------------------------------------------------------------
// @group Collection : CRUD operations on a document collection
// ---------------------------------------------------------------------------

/// A handle to a named document collection inside a [`Database`].
///
/// Provides MongoDB-compatible methods: `insert_one`, `find_one`, `find`,
/// `update_one`, `update_many`, `delete_one`, `delete_many`, `count_documents`.
pub struct Collection {
    db: DbHandle,
    name: String,
    ffi: Arc<KeraDbFfi>,
}

// SAFETY: DbHandle is an opaque pointer managed by the C library.
// Collection is not Clone/Copy and its lifetime is tied to Client.
unsafe impl Send for Collection {}

impl Collection {
    fn new(db: DbHandle, name: impl Into<String>, ffi: Arc<KeraDbFfi>) -> Self {
        Self {
            db,
            name: name.into(),
            ffi,
        }
    }

    /// The collection name.
    pub fn name(&self) -> &str {
        &self.name
    }

    // -----------------------------------------------------------------------
    // Insert
    // -----------------------------------------------------------------------

    /// Insert a single document and return its assigned ID.
    ///
    /// ```no_run
    /// # use keradb_sdk::*; use serde_json::json;
    /// # let mut client = connect("test.ndb").unwrap();
    /// let coll = client.database().collection("users");
    /// let result = coll.insert_one(json!({"name": "Alice", "age": 30})).unwrap();
    /// println!("inserted: {}", result.inserted_id);
    /// ```
    pub fn insert_one(&self, document: Value) -> Result<InsertOneResult> {
        let json_data = document.to_string();
        let c_collection = KeraDbFfi::to_cstring(&self.name)?;
        let c_json = KeraDbFfi::to_cstring(&json_data)?;

        let ptr =
            unsafe { (self.ffi.fn_insert)(self.db, c_collection.as_ptr(), c_json.as_ptr()) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let id = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        Ok(InsertOneResult::new(id))
    }

    /// Insert multiple documents and return all assigned IDs.
    pub fn insert_many(&self, documents: Vec<Value>) -> Result<InsertManyResult> {
        let mut ids = Vec::with_capacity(documents.len());
        for doc in documents {
            ids.push(self.insert_one(doc)?.inserted_id);
        }
        Ok(InsertManyResult::new(ids))
    }

    // -----------------------------------------------------------------------
    // Find
    // -----------------------------------------------------------------------

    /// Find a single document matching `filter`, or `None` if not found.
    ///
    /// Supports `{"_id": "..."}` for fast lookup as well as arbitrary
    /// MongoDB-style operators.
    pub fn find_one(&self, filter: Option<&Value>) -> Result<Option<Value>> {
        match filter {
            None => {
                let docs = self.find(None)?.limit(1).all();
                Ok(docs.into_iter().next())
            }
            Some(f) => {
                if let Some(id) = f.get("_id").and_then(|v| v.as_str()) {
                    // Fast path – look up by ID via native function
                    let c_coll = KeraDbFfi::to_cstring(&self.name)?;
                    let c_id = KeraDbFfi::to_cstring(id)?;
                    let ptr = unsafe {
                        (self.ffi.fn_find_by_id)(self.db, c_coll.as_ptr(), c_id.as_ptr())
                    };
                    if ptr.is_null() {
                        return Ok(None);
                    }
                    let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
                    let doc: Value = serde_json::from_str(&json_str)?;
                    return Ok(Some(doc));
                }
                // Slow path – scan all documents
                for doc in self.find(None)? {
                    if matches_filter(&doc, f) {
                        return Ok(Some(doc));
                    }
                }
                Ok(None)
            }
        }
    }

    /// Return a [`Cursor`] over all documents that match `filter`.
    ///
    /// Pass `None` to return every document in the collection.
    pub fn find(&self, filter: Option<&Value>) -> Result<Cursor> {
        let c_coll = KeraDbFfi::to_cstring(&self.name)?;
        let ptr = unsafe { (self.ffi.fn_find_all)(self.db, c_coll.as_ptr(), -1, -1) };

        if ptr.is_null() {
            return Ok(Cursor::new(vec![]));
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let docs: Vec<Value> = serde_json::from_str(&json_str)?;

        let filtered = match filter {
            Some(f) => docs.into_iter().filter(|d| matches_filter(d, f)).collect(),
            None => docs,
        };
        Ok(Cursor::new(filtered))
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    /// Update the first document that matches `filter` using `update` operators.
    ///
    /// Supports `$set`, `$unset`, `$inc`, `$push`, and full replacement.
    pub fn update_one(&self, filter: &Value, update: &Value) -> Result<UpdateResult> {
        let doc = match self.find_one(Some(filter))? {
            Some(d) => d,
            None => return Ok(UpdateResult::new(0, 0)),
        };

        let mut updated = apply_update(&doc, update);

        // Extract and remove `_id` before passing update data to native fn
        let id = match updated.as_object_mut().and_then(|o| o.remove("_id")) {
            Some(Value::String(s)) => s,
            Some(other) => other.to_string().trim_matches('"').to_owned(),
            None => return Err(KeraDbError::Other("Document missing _id".into())),
        };

        let json_data = updated.to_string();
        let c_coll = KeraDbFfi::to_cstring(&self.name)?;
        let c_id = KeraDbFfi::to_cstring(&id)?;
        let c_json = KeraDbFfi::to_cstring(&json_data)?;

        let ptr = unsafe {
            (self.ffi.fn_update)(self.db, c_coll.as_ptr(), c_id.as_ptr(), c_json.as_ptr())
        };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        unsafe { self.ffi.free_string(ptr) };
        Ok(UpdateResult::new(1, 1))
    }

    /// Update all documents that match `filter`.
    pub fn update_many(&self, filter: &Value, update: &Value) -> Result<UpdateResult> {
        let docs = self.find(Some(filter))?.all();
        let matched = docs.len();
        let mut modified = 0;
        for doc in docs {
            let id_filter = json!({"_id": doc["_id"]});
            self.update_one(&id_filter, update)?;
            modified += 1;
        }
        Ok(UpdateResult::new(matched, modified))
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Delete the first document that matches `filter`.
    pub fn delete_one(&self, filter: &Value) -> Result<DeleteResult> {
        let doc = match self.find_one(Some(filter))? {
            Some(d) => d,
            None => return Ok(DeleteResult::new(0)),
        };

        let id = doc["_id"].as_str().unwrap_or("").to_owned();
        let c_coll = KeraDbFfi::to_cstring(&self.name)?;
        let c_id = KeraDbFfi::to_cstring(&id)?;

        let result = unsafe { (self.ffi.fn_delete)(self.db, c_coll.as_ptr(), c_id.as_ptr()) };
        Ok(DeleteResult::new(if result != 0 { 1 } else { 0 }))
    }

    /// Delete all documents that match `filter`.
    pub fn delete_many(&self, filter: &Value) -> Result<DeleteResult> {
        let docs = self.find(Some(filter))?.all();
        let mut deleted = 0;
        for doc in docs {
            let id_filter = json!({"_id": doc["_id"]});
            deleted += self.delete_one(&id_filter)?.deleted_count;
        }
        Ok(DeleteResult::new(deleted))
    }

    // -----------------------------------------------------------------------
    // Count
    // -----------------------------------------------------------------------

    /// Count documents matching `filter`, or all documents when `None`.
    pub fn count_documents(&self, filter: Option<&Value>) -> Result<usize> {
        if let Some(f) = filter {
            return Ok(self.find(Some(f))?.all().len());
        }
        let c_coll = KeraDbFfi::to_cstring(&self.name)?;
        let count = unsafe { (self.ffi.fn_count)(self.db, c_coll.as_ptr()) };
        Ok(count.max(0) as usize)
    }
}

// ---------------------------------------------------------------------------
// @group Database : Database handle and collection access
// ---------------------------------------------------------------------------

/// A handle to an open KeraDB database.
///
/// Obtain one via [`Client::database`].
pub struct Database {
    db: DbHandle,
    ffi: Arc<KeraDbFfi>,
}

unsafe impl Send for Database {}

impl Database {
    fn new(db: DbHandle, ffi: Arc<KeraDbFfi>) -> Self {
        Self { db, ffi }
    }

    /// Return a [`Collection`] for the given name.  Collections are created
    /// implicitly on first write.
    pub fn collection(&self, name: &str) -> Collection {
        Collection::new(self.db, name, Arc::clone(&self.ffi))
    }

    /// Return the names of all collections in this database.
    pub fn list_collection_names(&self) -> Result<Vec<String>> {
        let ptr = unsafe { (self.ffi.fn_list_collections)(self.db) };
        if ptr.is_null() {
            return Ok(vec![]);
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let raw: Vec<Value> = serde_json::from_str(&json_str)?;
        Ok(raw
            .into_iter()
            .filter_map(|v| v.get(0).and_then(|n| n.as_str()).map(String::from))
            .collect())
    }
}

// ---------------------------------------------------------------------------
// @group Client : Top-level entry point and lifecycle management
// ---------------------------------------------------------------------------

/// The top-level KeraDB client.
///
/// # Example
/// ```no_run
/// use keradb_sdk::{connect, Client};
/// use serde_json::json;
///
/// let mut client = connect("mydb.ndb").unwrap();
/// let coll = client.database().collection("users");
/// let res = coll.insert_one(json!({"name": "Alice"})).unwrap();
/// println!("inserted: {}", res.inserted_id);
/// client.close();
/// ```
///
/// Or use the `Drop`-based RAII via the `with` / scope pattern – the
/// database is flushed and closed when [`Client`] is dropped.
pub struct Client {
    pub(crate) db: DbHandle,
    ffi: Arc<KeraDbFfi>,
    closed: bool,
}

unsafe impl Send for Client {}

impl Client {
    /// Open (or create) a database at `path`.
    fn open(path: &str) -> Result<Self> {
        let ffi = get_ffi()?;
        let c_path = KeraDbFfi::to_cstring(path)?;

        // Try open first, then create
        let db = unsafe { (ffi.fn_open)(c_path.as_ptr()) };
        let db = if db.is_null() {
            let db = unsafe { (ffi.fn_create)(c_path.as_ptr()) };
            if db.is_null() {
                return Err(KeraDbError::Native(ffi.last_error()));
            }
            db
        } else {
            db
        };

        Ok(Self {
            db,
            ffi,
            closed: false,
        })
    }

    /// Return the [`Database`] associated with this client.
    ///
    /// The optional `name` argument is accepted for MongoDB API compatibility
    /// but is ignored (KeraDB is single-database per file).
    pub fn database(&self) -> Database {
        Database::new(self.db, Arc::clone(&self.ffi))
    }

    /// Sync all pending writes to disk.
    pub fn sync(&self) -> Result<()> {
        if self.closed {
            return Err(KeraDbError::Closed);
        }
        unsafe { (self.ffi.fn_sync)(self.db) };
        Ok(())
    }

    /// Close the database.  Subsequent calls are safe (no-ops).
    pub fn close(&mut self) {
        if !self.closed {
            unsafe { (self.ffi.fn_close)(self.db) };
            self.closed = true;
        }
    }

    // -----------------------------------------------------------------------
    // @group VectorOps : Vector collection management and search
    // -----------------------------------------------------------------------

    fn require_vector_fn<T>(&self, f: Option<T>, fname: &str) -> Result<T> {
        f.ok_or_else(|| {
            KeraDbError::Other(format!(
                "Vector function '{}' is not available in this build of KeraDB",
                fname
            ))
        })
    }

    /// Create a new vector collection with the given configuration.
    pub fn create_vector_collection(&self, name: &str, config: &VectorConfig) -> Result<()> {
        let f = self.require_vector_fn(
            self.ffi.fn_create_vector_collection,
            "keradb_create_vector_collection",
        )?;
        let c_name = KeraDbFfi::to_cstring(name)?;
        let c_cfg = KeraDbFfi::to_cstring(&config.to_json())?;
        let ptr = unsafe { f(self.db, c_name.as_ptr(), c_cfg.as_ptr()) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        unsafe { self.ffi.free_string(ptr) };
        Ok(())
    }

    /// Return a list of all vector collections.
    pub fn list_vector_collections(&self) -> Result<Vec<VectorCollectionInfo>> {
        let f = self.require_vector_fn(
            self.ffi.fn_list_vector_collections,
            "keradb_list_vector_collections",
        )?;
        let ptr = unsafe { f(self.db) };
        if ptr.is_null() {
            return Ok(vec![]);
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let raw: Vec<Value> = serde_json::from_str(&json_str)?;
        Ok(raw
            .into_iter()
            .filter_map(|v| {
                let name = v.get("Name").or_else(|| v.get("name"))?.as_str()?.to_owned();
                let count = v
                    .get("Count")
                    .or_else(|| v.get("count"))
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0) as usize;
                Some(VectorCollectionInfo { name, count })
            })
            .collect())
    }

    /// Drop a vector collection, returning `true` on success.
    pub fn drop_vector_collection(&self, name: &str) -> Result<bool> {
        let f = self.require_vector_fn(
            self.ffi.fn_drop_vector_collection,
            "keradb_drop_vector_collection",
        )?;
        let c_name = KeraDbFfi::to_cstring(name)?;
        let result = unsafe { f(self.db, c_name.as_ptr()) };
        Ok(result != 0)
    }

    /// Insert a vector embedding with optional JSON metadata.
    ///
    /// Returns the numeric ID assigned by KeraDB.
    pub fn insert_vector(
        &self,
        collection: &str,
        embedding: &[f32],
        metadata: Option<&Value>,
    ) -> Result<u64> {
        let f = self.require_vector_fn(self.ffi.fn_insert_vector, "keradb_insert_vector")?;
        let embedding_json = serde_json::to_string(embedding)?;
        let meta_json = match metadata {
            Some(m) => serde_json::to_string(m)?,
            None => "{}".to_owned(),
        };
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let c_emb = KeraDbFfi::to_cstring(&embedding_json)?;
        let c_meta = KeraDbFfi::to_cstring(&meta_json)?;

        let ptr = unsafe { f(self.db, c_coll.as_ptr(), c_emb.as_ptr(), c_meta.as_ptr()) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let id_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        id_str
            .parse::<u64>()
            .map_err(|e| KeraDbError::Other(format!("Failed to parse vector ID: {}", e)))
    }

    /// Insert text (requires a lazy-embedding collection) and return its ID.
    pub fn insert_text(
        &self,
        collection: &str,
        text: &str,
        metadata: Option<&Value>,
    ) -> Result<u64> {
        let f = self.require_vector_fn(self.ffi.fn_insert_text, "keradb_insert_text")?;
        let meta_json = match metadata {
            Some(m) => serde_json::to_string(m)?,
            None => "{}".to_owned(),
        };
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let c_text = KeraDbFfi::to_cstring(text)?;
        let c_meta = KeraDbFfi::to_cstring(&meta_json)?;

        let ptr = unsafe { f(self.db, c_coll.as_ptr(), c_text.as_ptr(), c_meta.as_ptr()) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let id_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        id_str
            .parse::<u64>()
            .map_err(|e| KeraDbError::Other(format!("Failed to parse vector ID: {}", e)))
    }

    /// Perform a k-nearest-neighbour vector search.
    pub fn vector_search(
        &self,
        collection: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        let f = self.require_vector_fn(self.ffi.fn_vector_search, "keradb_vector_search")?;
        let query_json = serde_json::to_string(query)?;
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let c_query = KeraDbFfi::to_cstring(&query_json)?;

        let ptr = unsafe { f(self.db, c_coll.as_ptr(), c_query.as_ptr(), k as i32) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let raw: Vec<Value> = serde_json::from_str(&json_str)?;
        Ok(raw
            .iter()
            .filter_map(VectorSearchResult::from_value)
            .collect())
    }

    /// Perform a text-based similarity search (requires lazy-embedding collection).
    pub fn vector_search_text(
        &self,
        collection: &str,
        query: &str,
        k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        let f =
            self.require_vector_fn(self.ffi.fn_vector_search_text, "keradb_vector_search_text")?;
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let c_query = KeraDbFfi::to_cstring(query)?;

        let ptr = unsafe { f(self.db, c_coll.as_ptr(), c_query.as_ptr(), k as i32) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let raw: Vec<Value> = serde_json::from_str(&json_str)?;
        Ok(raw
            .iter()
            .filter_map(VectorSearchResult::from_value)
            .collect())
    }

    /// Perform a filtered k-NN search.
    pub fn vector_search_filtered(
        &self,
        collection: &str,
        query: &[f32],
        k: usize,
        filter: &MetadataFilter,
    ) -> Result<Vec<VectorSearchResult>> {
        let f = self.require_vector_fn(
            self.ffi.fn_vector_search_filtered,
            "keradb_vector_search_filtered",
        )?;
        let query_json = serde_json::to_string(query)?;
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let c_query = KeraDbFfi::to_cstring(&query_json)?;
        let c_filter = KeraDbFfi::to_cstring(&filter.to_json())?;

        let ptr =
            unsafe { f(self.db, c_coll.as_ptr(), c_query.as_ptr(), k as i32, c_filter.as_ptr()) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let raw: Vec<Value> = serde_json::from_str(&json_str)?;
        Ok(raw
            .iter()
            .filter_map(VectorSearchResult::from_value)
            .collect())
    }

    /// Retrieve a single vector document by its numeric ID.
    pub fn get_vector(&self, collection: &str, id: u64) -> Result<Option<VectorDocument>> {
        let f = self.require_vector_fn(self.ffi.fn_get_vector, "keradb_get_vector")?;
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let ptr = unsafe { f(self.db, c_coll.as_ptr(), id) };
        if ptr.is_null() {
            return Ok(None);
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let v: Value = serde_json::from_str(&json_str)?;
        Ok(VectorDocument::from_value(&v))
    }

    /// Delete a vector document by its numeric ID.  Returns `true` if deleted.
    pub fn delete_vector(&self, collection: &str, id: u64) -> Result<bool> {
        let f = self.require_vector_fn(self.ffi.fn_delete_vector, "keradb_delete_vector")?;
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let result = unsafe { f(self.db, c_coll.as_ptr(), id) };
        Ok(result != 0)
    }

    /// Return statistics for a vector collection.
    pub fn vector_stats(&self, collection: &str) -> Result<VectorCollectionStats> {
        let f = self.require_vector_fn(self.ffi.fn_vector_stats, "keradb_vector_stats")?;
        let c_coll = KeraDbFfi::to_cstring(collection)?;
        let ptr = unsafe { f(self.db, c_coll.as_ptr()) };
        if ptr.is_null() {
            return Err(KeraDbError::Native(self.ffi.last_error()));
        }
        let json_str = unsafe { self.ffi.c_str_to_string_and_free(ptr) }?;
        let v: Value = serde_json::from_str(&json_str)?;
        VectorCollectionStats::from_value(&v)
            .ok_or_else(|| KeraDbError::Other("Failed to parse vector stats".into()))
    }

    /// Returns `true` when the loaded native library includes vector support.
    pub fn has_vector_support(&self) -> bool {
        self.ffi.has_vector_support
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.close();
    }
}

// ---------------------------------------------------------------------------
// Top-level convenience constructor
// ---------------------------------------------------------------------------

/// Open or create a KeraDB database file.
///
/// ```no_run
/// use keradb_sdk::connect;
///
/// let mut client = connect("mydb.ndb").unwrap();
/// // ... use client ...
/// client.close();
/// ```
pub fn connect(path: &str) -> Result<Client> {
    Client::open(path)
}
