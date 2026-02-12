//! Minimal generic C FFI — only 7 functions needed.

use crate::TantivyIndex;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

fn set_err(out: *mut *mut c_char, msg: &str) {
    if !out.is_null() {
        if let Ok(c) = CString::new(msg) { unsafe { *out = c.into_raw() }; }
    }
}

fn cstr<'a>(p: *const c_char) -> Result<&'a str, String> {
    if p.is_null() { return Ok(""); }
    unsafe { CStr::from_ptr(p) }.to_str().map_err(|e| e.to_string())
}

fn ret_json(s: &str) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn tantivy_free_string(s: *mut c_char) {
    if !s.is_null() { unsafe { let _ = CString::from_raw(s); } }
}

#[no_mangle]
pub extern "C" fn tantivy_free_index(idx: *mut TantivyIndex) {
    if !idx.is_null() { unsafe { let _ = Box::from_raw(idx); } }
}

/// Create index with JSON schema. Returns handle or null.
#[no_mangle]
pub extern "C" fn tantivy_create_index(
    path: *const c_char, schema_json: *const c_char, err: *mut *mut c_char,
) -> *mut TantivyIndex {
    let r = (|| -> Result<*mut TantivyIndex, String> {
        let p = cstr(path)?;
        let s = cstr(schema_json)?;
        let idx = TantivyIndex::create(p, s)?;
        Ok(Box::into_raw(Box::new(idx)))
    })();
    match r { Ok(p) => p, Err(e) => { set_err(err, &e); ptr::null_mut() } }
}

/// Open existing index (schema read from _schema.json inside index dir).
#[no_mangle]
pub extern "C" fn tantivy_open_index(
    path: *const c_char, err: *mut *mut c_char,
) -> *mut TantivyIndex {
    let r = (|| -> Result<*mut TantivyIndex, String> {
        let p = cstr(path)?;
        let idx = TantivyIndex::open(p)?;
        Ok(Box::into_raw(Box::new(idx)))
    })();
    match r { Ok(p) => p, Err(e) => { set_err(err, &e); ptr::null_mut() } }
}

/// Add a JSON document. Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn tantivy_add_doc(
    idx: *mut TantivyIndex, doc_json: *const c_char, err: *mut *mut c_char,
) -> i32 {
    let idx = unsafe { &*idx };
    match cstr(doc_json).and_then(|s| idx.add_doc(s)) {
        Ok(()) => 0,
        Err(e) => { set_err(err, &e); -1 }
    }
}

/// Commit pending writes. Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn tantivy_commit(idx: *mut TantivyIndex, err: *mut *mut c_char) -> i32 {
    let idx = unsafe { &*idx };
    match idx.commit() { Ok(()) => 0, Err(e) => { set_err(err, &e); -1 } }
}

/// Get number of documents in the index.
#[no_mangle]
pub extern "C" fn tantivy_num_docs(idx: *mut TantivyIndex) -> u64 {
    unsafe { &*idx }.num_docs()
}

/// Search with JSON query DSL. Returns JSON results string (caller frees with tantivy_free_string).
#[no_mangle]
pub extern "C" fn tantivy_search(
    idx: *mut TantivyIndex, query_json: *const c_char, err: *mut *mut c_char,
) -> *mut c_char {
    let idx = unsafe { &*idx };
    let r = cstr(query_json).and_then(|s| idx.search(s));
    match r {
        Ok(results) => ret_json(&serde_json::to_string(&results).unwrap_or_default()),
        Err(e) => { set_err(err, &e); ptr::null_mut() }
    }
}
