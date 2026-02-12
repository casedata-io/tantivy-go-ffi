// Package tantivy provides a generic Go client for the Tantivy FFI crate.
// Schema-agnostic — define your fields, add JSON documents, query with JSON DSL.
package tantivy

/*
#cgo CFLAGS: -I${SRCDIR}/../../libs -I${SRCDIR}/../../ -I/usr/local/include
#cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/../../libs/darwin_arm64 -L/usr/local/lib -ltantivy_go -lm -ldl -framework Security -framework CoreFoundation
#cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/../../libs/darwin_amd64 -L/usr/local/lib -ltantivy_go -lm -ldl -framework Security -framework CoreFoundation
#cgo linux,amd64 LDFLAGS: -L${SRCDIR}/../../libs/linux_amd64 -L/usr/local/lib -ltantivy_go -lm -ldl -lpthread
#cgo linux,arm64 LDFLAGS: -L${SRCDIR}/../../libs/linux_arm64 -L/usr/local/lib -ltantivy_go -lm -ldl -lpthread
#include "tantivy_go.h"
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"fmt"
	"unsafe"
)

// FieldDef defines a field in the schema.
type FieldDef struct {
	Name      string `json:"name"`
	Type      string `json:"type"` // "text", "i64", "f64"
	Stored    bool   `json:"stored"`
	Indexed   bool   `json:"indexed"`
	Fast      bool   `json:"fast,omitempty"`
	Tokenizer string `json:"tokenizer,omitempty"` // "default", "raw", "en_stem"
}

// Schema defines the index schema.
type Schema struct {
	Fields       []FieldDef `json:"fields"`
	SearchFields []string   `json:"search_fields,omitempty"` // default text search fields
}

// SearchResults is the generic result from a search.
type SearchResults struct {
	Results    []map[string]interface{} `json:"results"`
	Count      int                      `json:"count"`
	TotalCount int                      `json:"total_count"`
	Limit      int                      `json:"limit"`
	Offset     int                      `json:"offset"`
}

// Index is a handle to a Tantivy index.
type Index struct {
	handle C.TantivyIndexHandle
}

// Create creates a new index at the given path with the given schema.
func Create(path string, schema Schema) (*Index, error) {
	schemaJSON, err := json.Marshal(schema)
	if err != nil {
		return nil, fmt.Errorf("marshal schema: %w", err)
	}

	cPath := C.CString(path)
	cSchema := C.CString(string(schemaJSON))
	defer C.free(unsafe.Pointer(cPath))
	defer C.free(unsafe.Pointer(cSchema))

	var errOut *C.char
	h := C.tantivy_create_index(cPath, cSchema, &errOut)
	if h == nil {
		return nil, ffiErr(errOut, "create")
	}
	return &Index{handle: h}, nil
}

// Open opens an existing index (schema is read from _schema.json in the index dir).
func Open(path string) (*Index, error) {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))

	var errOut *C.char
	h := C.tantivy_open_index(cPath, &errOut)
	if h == nil {
		return nil, ffiErr(errOut, "open")
	}
	return &Index{handle: h}, nil
}

// Close frees the index handle.
func (idx *Index) Close() {
	if idx.handle != nil {
		C.tantivy_free_index(idx.handle)
		idx.handle = nil
	}
}

// AddDoc adds a document (as a map or struct that marshals to JSON).
func (idx *Index) AddDoc(doc interface{}) error {
	docJSON, err := json.Marshal(doc)
	if err != nil {
		return fmt.Errorf("marshal doc: %w", err)
	}

	cDoc := C.CString(string(docJSON))
	defer C.free(unsafe.Pointer(cDoc))

	var errOut *C.char
	if C.tantivy_add_doc(idx.handle, cDoc, &errOut) != 0 {
		return ffiErr(errOut, "add_doc")
	}
	return nil
}

// AddDocJSON adds a document from raw JSON bytes.
func (idx *Index) AddDocJSON(docJSON []byte) error {
	cDoc := C.CString(string(docJSON))
	defer C.free(unsafe.Pointer(cDoc))

	var errOut *C.char
	if C.tantivy_add_doc(idx.handle, cDoc, &errOut) != 0 {
		return ffiErr(errOut, "add_doc")
	}
	return nil
}

// Commit commits all pending writes to disk.
func (idx *Index) Commit() error {
	var errOut *C.char
	if C.tantivy_commit(idx.handle, &errOut) != 0 {
		return ffiErr(errOut, "commit")
	}
	return nil
}

// NumDocs returns the number of documents in the index.
func (idx *Index) NumDocs() uint64 {
	return uint64(C.tantivy_num_docs(idx.handle))
}

// Search executes a query using the JSON DSL and returns results.
func (idx *Index) Search(query interface{}) (*SearchResults, error) {
	queryJSON, err := json.Marshal(query)
	if err != nil {
		return nil, fmt.Errorf("marshal query: %w", err)
	}
	return idx.SearchJSON(queryJSON)
}

// SearchJSON executes a raw JSON query.
func (idx *Index) SearchJSON(queryJSON []byte) (*SearchResults, error) {
	cQuery := C.CString(string(queryJSON))
	defer C.free(unsafe.Pointer(cQuery))

	var errOut *C.char
	result := C.tantivy_search(idx.handle, cQuery, &errOut)
	if result == nil {
		return nil, ffiErr(errOut, "search")
	}
	defer C.tantivy_free_string(result)

	jsonStr := C.GoString(result)
	var sr SearchResults
	if err := json.Unmarshal([]byte(jsonStr), &sr); err != nil {
		return nil, fmt.Errorf("parse results: %w", err)
	}
	return &sr, nil
}

// ===== Query Builder Helpers =====

// TextQuery builds a text search query.
func TextQuery(q string, limit int) map[string]interface{} {
	return map[string]interface{}{"type": "text", "query": q, "limit": limit}
}

// FuzzyQuery builds a fuzzy search query.
func FuzzyQuery(term string, distance int, limit int) map[string]interface{} {
	return map[string]interface{}{"type": "fuzzy", "term": term, "distance": distance, "limit": limit}
}

// PhraseQuery builds a phrase search query.
func PhraseQuery(phrase string, limit int) map[string]interface{} {
	return map[string]interface{}{"type": "phrase", "phrase": phrase, "limit": limit}
}

// PrefixQuery builds a prefix search query.
func PrefixQuery(prefix string, limit int) map[string]interface{} {
	return map[string]interface{}{"type": "prefix", "prefix": prefix, "limit": limit}
}

// TermMatchQuery builds an exact term match query.
func TermMatchQuery(field string, value interface{}, limit int) map[string]interface{} {
	return map[string]interface{}{"type": "term_match", "field": field, "value": value, "limit": limit}
}

// RangeI64Query builds an integer range query.
func RangeI64Query(field string, min, max *int64, limit int) map[string]interface{} {
	q := map[string]interface{}{"type": "range_i64", "field": field, "limit": limit}
	if min != nil {
		q["min"] = *min
	}
	if max != nil {
		q["max"] = *max
	}
	return q
}

// RangeF64Query builds a float range query.
func RangeF64Query(field string, min, max *float64, limit int) map[string]interface{} {
	q := map[string]interface{}{"type": "range_f64", "field": field, "limit": limit}
	if min != nil {
		q["min"] = *min
	}
	if max != nil {
		q["max"] = *max
	}
	return q
}

// BoolQuery builds a boolean combination query.
func BoolQuery(must, should, mustNot []map[string]interface{}, limit int) map[string]interface{} {
	if must == nil {
		must = []map[string]interface{}{}
	}
	if should == nil {
		should = []map[string]interface{}{}
	}
	if mustNot == nil {
		mustNot = []map[string]interface{}{}
	}
	return map[string]interface{}{
		"type": "bool", "must": must, "should": should, "must_not": mustNot, "limit": limit,
	}
}

func ffiErr(errOut *C.char, context string) error {
	if errOut != nil {
		msg := C.GoString(errOut)
		C.tantivy_free_string(errOut)
		return fmt.Errorf("tantivy %s: %s", context, msg)
	}
	return fmt.Errorf("tantivy %s: unknown error", context)
}
