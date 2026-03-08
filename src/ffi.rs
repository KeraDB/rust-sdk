//! FFI bindings to the KeraDB native shared library.
//!
//! Mirrors the Python SDK's `ffi.py` module.  All raw pointer traffic is
//! isolated to this module so that higher-level code remains safe Rust.

// @group FFICore      : Library loading and handle lifecycle
// @group FFIDocument  : Document-level C function wrappers
// @group FFIVector    : Vector-level C function wrappers
// @group FFIError     : Error retrieval and string management

use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_ulonglong, c_void},
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use libloading::{Library, Symbol};

use crate::error::{KeraDbError, Result};

// ---------------------------------------------------------------------------
// @group FFICore : Library loading and handle lifecycle
// ---------------------------------------------------------------------------

/// Opaque database handle (mirrors Python's `db_handle`).
pub type DbHandle = *mut c_void;

/// Platform-specific library names, in priority order.
fn candidate_lib_names() -> Vec<&'static str> {
    #[cfg(target_os = "windows")]
    return vec!["keradb.dll"];
    #[cfg(target_os = "macos")]
    return vec!["libkeradb.dylib"];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return vec!["libkeradb.so"];
}

/// Candidate search paths (mirrors Python's `search_paths` list).
fn search_paths(lib_name: &str) -> Vec<PathBuf> {
    // Directory of the compiled test / example binary
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    // Workspace target/release
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_release = manifest
        .ancestors()
        .nth(2) // sdks/rust -> sdks -> workspace root
        .map(|p| p.join("target").join("release").join(lib_name));

    let mut paths: Vec<PathBuf> = Vec::new();

    if let Some(dir) = exe_dir {
        paths.push(dir.join(lib_name));
    }
    paths.push(manifest.join(lib_name));
    if let Some(p) = workspace_release {
        paths.push(p);
    }
    // Project root relative to manifest (sdks/rust/../../..)
    paths.push(
        manifest
            .ancestors()
            .nth(3)
            .unwrap_or(&manifest)
            .join("target")
            .join("release")
            .join(lib_name),
    );

    paths
}

// ---------------------------------------------------------------------------
// Raw function pointer type aliases – one per exported C symbol
// ---------------------------------------------------------------------------

type FnCreate = unsafe extern "C" fn(*const c_char) -> DbHandle;
type FnOpen = unsafe extern "C" fn(*const c_char) -> DbHandle;
type FnClose = unsafe extern "C" fn(DbHandle);
type FnInsert = unsafe extern "C" fn(DbHandle, *const c_char, *const c_char) -> *const c_char;
type FnFindById =
    unsafe extern "C" fn(DbHandle, *const c_char, *const c_char) -> *const c_char;
type FnUpdate =
    unsafe extern "C" fn(DbHandle, *const c_char, *const c_char, *const c_char) -> *const c_char;
type FnDelete = unsafe extern "C" fn(DbHandle, *const c_char, *const c_char) -> c_int;
type FnFindAll =
    unsafe extern "C" fn(DbHandle, *const c_char, c_int, c_int) -> *const c_char;
type FnCount = unsafe extern "C" fn(DbHandle, *const c_char) -> c_int;
type FnListCollections = unsafe extern "C" fn(DbHandle) -> *const c_char;
type FnSync = unsafe extern "C" fn(DbHandle) -> c_int;
type FnLastError = unsafe extern "C" fn() -> *const c_char;
type FnFreeString = unsafe extern "C" fn(*const c_char);

// Vector function pointers
type FnCreateVectorCollection =
    unsafe extern "C" fn(DbHandle, *const c_char, *const c_char) -> *const c_char;
type FnListVectorCollections = unsafe extern "C" fn(DbHandle) -> *const c_char;
type FnDropVectorCollection = unsafe extern "C" fn(DbHandle, *const c_char) -> c_int;
type FnInsertVector = unsafe extern "C" fn(
    DbHandle,
    *const c_char,
    *const c_char,
    *const c_char,
) -> *const c_char;
type FnInsertText = unsafe extern "C" fn(
    DbHandle,
    *const c_char,
    *const c_char,
    *const c_char,
) -> *const c_char;
type FnVectorSearch =
    unsafe extern "C" fn(DbHandle, *const c_char, *const c_char, c_int) -> *const c_char;
type FnVectorSearchText =
    unsafe extern "C" fn(DbHandle, *const c_char, *const c_char, c_int) -> *const c_char;
type FnVectorSearchFiltered = unsafe extern "C" fn(
    DbHandle,
    *const c_char,
    *const c_char,
    c_int,
    *const c_char,
) -> *const c_char;
type FnGetVector =
    unsafe extern "C" fn(DbHandle, *const c_char, c_ulonglong) -> *const c_char;
type FnDeleteVector = unsafe extern "C" fn(DbHandle, *const c_char, c_ulonglong) -> c_int;
type FnVectorStats = unsafe extern "C" fn(DbHandle, *const c_char) -> *const c_char;

// ---------------------------------------------------------------------------
// KeraDbFfi – loaded function table
// ---------------------------------------------------------------------------

/// Holds the loaded `Library` and all resolved function pointers.
///
/// Kept in a process-global [`OnceLock`] so it is initialised once and shared.
pub struct KeraDbFfi {
    // The library must stay alive as long as function pointers are used.
    // SAFETY: `_lib` is never dropped before `KeraDbFfi` itself.
    _lib: Arc<Library>,

    // @group FFIDocument : Document function pointers
    pub(crate) fn_create: FnCreate,
    pub(crate) fn_open: FnOpen,
    pub(crate) fn_close: FnClose,
    pub(crate) fn_insert: FnInsert,
    pub(crate) fn_find_by_id: FnFindById,
    pub(crate) fn_update: FnUpdate,
    pub(crate) fn_delete: FnDelete,
    pub(crate) fn_find_all: FnFindAll,
    pub(crate) fn_count: FnCount,
    pub(crate) fn_list_collections: FnListCollections,
    pub(crate) fn_sync: FnSync,
    pub(crate) fn_last_error: FnLastError,
    pub(crate) fn_free_string: FnFreeString,

    // @group FFIVector : Vector function pointers (optional)
    pub(crate) has_vector_support: bool,
    pub(crate) fn_create_vector_collection: Option<FnCreateVectorCollection>,
    pub(crate) fn_list_vector_collections: Option<FnListVectorCollections>,
    pub(crate) fn_drop_vector_collection: Option<FnDropVectorCollection>,
    pub(crate) fn_insert_vector: Option<FnInsertVector>,
    pub(crate) fn_insert_text: Option<FnInsertText>,
    pub(crate) fn_vector_search: Option<FnVectorSearch>,
    pub(crate) fn_vector_search_text: Option<FnVectorSearchText>,
    pub(crate) fn_vector_search_filtered: Option<FnVectorSearchFiltered>,
    pub(crate) fn_get_vector: Option<FnGetVector>,
    pub(crate) fn_delete_vector: Option<FnDeleteVector>,
    pub(crate) fn_vector_stats: Option<FnVectorStats>,
}

// SAFETY: All function pointers originate from a single loaded shared library
// and are used only while that library is alive.  `DbHandle` (raw pointer) is
// managed solely by the Client which enforces single-threaded access patterns
// matching the underlying C API.
unsafe impl Send for KeraDbFfi {}
unsafe impl Sync for KeraDbFfi {}

impl KeraDbFfi {
    /// Load the native library and resolve all symbols.
    fn load() -> Result<Arc<Self>> {
        let names = candidate_lib_names();
        let lib_name = names[0];

        let lib = {
            let mut loaded: Option<Library> = None;
            for path in search_paths(lib_name) {
                if path.exists() {
                    match unsafe { Library::new(&path) } {
                        Ok(l) => {
                            loaded = Some(l);
                            break;
                        }
                        Err(_) => continue,
                    }
                }
            }
            // Last resort – let the OS linker search
            if loaded.is_none() {
                loaded = unsafe { Library::new(lib_name) }.ok();
            }
            loaded.ok_or_else(|| {
                KeraDbError::LibraryLoad(format!(
                    "Could not find {} in any search path. \
                     Build keradb first (cargo build --release) so the \
                     library is present in target/release/.",
                    lib_name
                ))
            })?
        };

        let lib = Arc::new(lib);

        // @group FFIDocument : Resolve required document symbols
        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                let s: Symbol<$ty> = unsafe { lib.get($name) }.map_err(|e| {
                    KeraDbError::LibraryLoad(format!("Missing symbol '{}': {}", unsafe {
                        std::str::from_utf8_unchecked($name)
                    }, e))
                })?;
                *s
            }};
        }

        let fn_create: FnCreate = sym!(b"keradb_create\0", FnCreate);
        let fn_open: FnOpen = sym!(b"keradb_open\0", FnOpen);
        let fn_close: FnClose = sym!(b"keradb_close\0", FnClose);
        let fn_insert: FnInsert = sym!(b"keradb_insert\0", FnInsert);
        let fn_find_by_id: FnFindById = sym!(b"keradb_find_by_id\0", FnFindById);
        let fn_update: FnUpdate = sym!(b"keradb_update\0", FnUpdate);
        let fn_delete: FnDelete = sym!(b"keradb_delete\0", FnDelete);
        let fn_find_all: FnFindAll = sym!(b"keradb_find_all\0", FnFindAll);
        let fn_count: FnCount = sym!(b"keradb_count\0", FnCount);
        let fn_list_collections: FnListCollections =
            sym!(b"keradb_list_collections\0", FnListCollections);
        let fn_sync: FnSync = sym!(b"keradb_sync\0", FnSync);
        let fn_last_error: FnLastError = sym!(b"keradb_last_error\0", FnLastError);
        let fn_free_string: FnFreeString = sym!(b"keradb_free_string\0", FnFreeString);

        // @group FFIVector : Resolve optional vector symbols
        macro_rules! opt_sym {
            ($name:literal, $ty:ty) => {
                unsafe { lib.get::<$ty>($name).ok().map(|s| *s) }
            };
        }

        let fn_create_vector_collection =
            opt_sym!(b"keradb_create_vector_collection\0", FnCreateVectorCollection);
        let fn_list_vector_collections =
            opt_sym!(b"keradb_list_vector_collections\0", FnListVectorCollections);
        let fn_drop_vector_collection =
            opt_sym!(b"keradb_drop_vector_collection\0", FnDropVectorCollection);
        let fn_insert_vector = opt_sym!(b"keradb_insert_vector\0", FnInsertVector);
        let fn_insert_text = opt_sym!(b"keradb_insert_text\0", FnInsertText);
        let fn_vector_search = opt_sym!(b"keradb_vector_search\0", FnVectorSearch);
        let fn_vector_search_text =
            opt_sym!(b"keradb_vector_search_text\0", FnVectorSearchText);
        let fn_vector_search_filtered =
            opt_sym!(b"keradb_vector_search_filtered\0", FnVectorSearchFiltered);
        let fn_get_vector = opt_sym!(b"keradb_get_vector\0", FnGetVector);
        let fn_delete_vector = opt_sym!(b"keradb_delete_vector\0", FnDeleteVector);
        let fn_vector_stats = opt_sym!(b"keradb_vector_stats\0", FnVectorStats);

        let has_vector_support = fn_create_vector_collection.is_some();

        Ok(Arc::new(Self {
            _lib: lib,
            fn_create,
            fn_open,
            fn_close,
            fn_insert,
            fn_find_by_id,
            fn_update,
            fn_delete,
            fn_find_all,
            fn_count,
            fn_list_collections,
            fn_sync,
            fn_last_error,
            fn_free_string,
            has_vector_support,
            fn_create_vector_collection,
            fn_list_vector_collections,
            fn_drop_vector_collection,
            fn_insert_vector,
            fn_insert_text,
            fn_vector_search,
            fn_vector_search_text,
            fn_vector_search_filtered,
            fn_get_vector,
            fn_delete_vector,
            fn_vector_stats,
        }))
    }

    // @group FFIError : Error string helpers

    /// Retrieve the last error message set by the native library.
    pub fn last_error(&self) -> String {
        let ptr = unsafe { (self.fn_last_error)() };
        if ptr.is_null() {
            return "Unknown error".to_owned();
        }
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    /// Free a string that was allocated by the native library.
    ///
    /// # Safety
    /// `ptr` must have been returned by a native KeraDB function.
    pub unsafe fn free_string(&self, ptr: *const c_char) {
        if !ptr.is_null() {
            (self.fn_free_string)(ptr);
        }
    }

    // @group FFIDocument : Convenience helpers for native calls

    /// Convert a raw C string pointer to an owned `String`, then free it.
    ///
    /// # Safety
    /// `ptr` must be a valid, NUL-terminated C string allocated by KeraDB.
    pub unsafe fn c_str_to_string_and_free(&self, ptr: *const c_char) -> Result<String> {
        if ptr.is_null() {
            return Err(KeraDbError::NullPointer("expected string result".into()));
        }
        let s = CStr::from_ptr(ptr)
            .to_str()
            .map(|s| s.to_owned())
            .map_err(|e| KeraDbError::Utf8(e.to_string()))?;
        self.free_string(ptr);
        Ok(s)
    }

    /// Convert a Rust `&str` to a `CString`, returning an error on interior NUL bytes.
    pub fn to_cstring(s: &str) -> Result<CString> {
        CString::new(s).map_err(|e| KeraDbError::Other(format!("CString error: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// @group FFICore : Global singleton
// ---------------------------------------------------------------------------

static GLOBAL_FFI: OnceLock<Arc<KeraDbFfi>> = OnceLock::new();

/// Return the process-global [`KeraDbFfi`] instance, loading it on first call.
pub fn get_ffi() -> Result<Arc<KeraDbFfi>> {
    // OnceLock::get_or_try_init would be ideal but is nightly-only.
    // Instead we use a layered approach: try to get, then load if missing.
    if let Some(ffi) = GLOBAL_FFI.get() {
        return Ok(Arc::clone(ffi));
    }
    let ffi = KeraDbFfi::load()?;
    // If another thread beat us to it, use the winner's value.
    Ok(Arc::clone(GLOBAL_FFI.get_or_init(|| ffi)))
}
