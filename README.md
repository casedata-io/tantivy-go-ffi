# tantivy-go-ffi

Generic Tantivy FFI bindings for Go. Schema-agnostic — works with any data type.

## Architecture

```
┌──────────────────────────────────────────┐
│  Your Go Application                      │
│  (IMDB, CaseData, anything...)           │
└──────────┬───────────────────────────────┘
           │ Go structs / maps
┌──────────▼───────────────────────────────┐
│  go/tantivy/client.go                     │
│  Generic Go client (JSON in, JSON out)    │
│  - Create(path, schema)                   │
│  - AddDoc(doc)                            │
│  - Search(queryDSL)                       │
│  - Query helpers: TextQuery, BoolQuery... │
└──────────┬───────────────────────────────┘
           │ CGO (C strings)
┌──────────▼───────────────────────────────┐
│  src/ffi.rs — 7 C functions               │
│  tantivy_create_index(path, schema_json)  │
│  tantivy_open_index(path)                 │
│  tantivy_add_doc(handle, doc_json)        │
│  tantivy_commit(handle)                   │
│  tantivy_num_docs(handle)                 │
│  tantivy_search(handle, query_json)       │
│  tantivy_free_string / tantivy_free_index │
└──────────┬───────────────────────────────┘
           │ Rust native calls
┌──────────▼───────────────────────────────┐
│  src/lib.rs — Generic Tantivy wrapper     │
│  - Dynamic schema from JSON               │
│  - JSON document ingestion                │
│  - JSON Query DSL → Tantivy queries       │
│  - Typed fields: text, i64, f64           │
│  - Fast fields for columnar access        │
└───────────────────────────────────────────┘
```

## Quick Start (Go)

```go
import "github.com/your/tantivy-go-ffi/go/tantivy"

// 1. Define schema
schema := tantivy.Schema{
    Fields: []tantivy.FieldDef{
        {Name: "id", Type: "text", Stored: true, Indexed: true, Tokenizer: "raw"},
        {Name: "title", Type: "text", Stored: true, Indexed: true},
        {Name: "year", Type: "i64", Stored: true, Indexed: true, Fast: true},
        {Name: "rating", Type: "f64", Stored: true, Indexed: true, Fast: true},
    },
    SearchFields: []string{"title"},
}

// 2. Create index
idx, _ := tantivy.Create("/tmp/myindex", schema)
defer idx.Close()

// 3. Add documents (any struct or map)
idx.AddDoc(map[string]interface{}{
    "id": "tt0111161", "title": "The Shawshank Redemption",
    "year": 1994, "rating": 9.3,
})
idx.Commit()

// 4. Search with Query DSL
results, _ := idx.Search(tantivy.TextQuery("shawshank", 10))
// results.Results = [{"id":"tt0111161","title":"The Shawshank Redemption","year":1994,"rating":9.3,"_score":1.23}]

// 5. Structured queries
results, _ = idx.Search(tantivy.RangeI64Query("year", ptr(int64(2000)), ptr(int64(2010)), 100))

// 6. Combined (text + structured) — single indexed pass
results, _ = idx.Search(tantivy.BoolQuery(
    []map[string]interface{}{
        tantivy.TextQuery("war", 0),
        tantivy.RangeI64Query("year", ptr(int64(2000)), ptr(int64(2010)), 0),
    },
    nil, nil, 100,
))
```

## Query DSL

All queries are JSON objects with a `type` field:

| Type | Description | Example |
|------|-------------|---------|
| `text` | Full-text search | `{"type":"text","query":"batman","limit":100}` |
| `fuzzy` | Typo-tolerant search | `{"type":"fuzzy","term":"batmna","distance":2,"limit":100}` |
| `phrase` | Exact phrase match | `{"type":"phrase","phrase":"the dark knight","limit":100}` |
| `prefix` | Prefix/autocomplete | `{"type":"prefix","prefix":"bat","limit":100}` |
| `term_match` | Exact field match | `{"type":"term_match","field":"id","value":"tt0111161"}` |
| `range_i64` | Integer range | `{"type":"range_i64","field":"year","min":2000,"max":2010}` |
| `range_f64` | Float range | `{"type":"range_f64","field":"rating","min":8.0}` |
| `bool` | Boolean combination | `{"type":"bool","must":[...],"should":[...],"must_not":[...]}` |
| `all` | Match all docs | `{"type":"all","limit":10}` |

## Schema Definition

```json
{
  "fields": [
    {"name": "id",     "type": "text", "stored": true, "indexed": true, "tokenizer": "raw"},
    {"name": "title",  "type": "text", "stored": true, "indexed": true},
    {"name": "year",   "type": "i64",  "stored": true, "indexed": true, "fast": true},
    {"name": "rating", "type": "f64",  "stored": true, "indexed": true, "fast": true}
  ],
  "search_fields": ["title"]
}
```

**Field types:** `text`, `i64`, `f64`
**Tokenizers:** `default` (English), `raw` (exact match), `en_stem` (stemming)
**Fast fields:** Enable columnar access for sorting/filtering (like Tantivy's equivalent of DuckDB columns)

## Building

```bash
# Build the Rust static library
cargo build --release
# Output: target/release/libtantivy_go.a

# From your Go project, link against it (see CGO flags in client.go)
```

## Files

```
tantivy-go-ffi/
├── Cargo.toml          # Rust crate config
├── src/
│   ├── lib.rs          # Generic Tantivy wrapper (437 lines)
│   └── ffi.rs          # C FFI layer (96 lines)
├── tantivy_go.h        # C header (30 lines)
├── go/
│   └── tantivy/
│       └── client.go   # Go client + query helpers (215 lines)
└── target/release/
    └── libtantivy_go.a # Static library (~12MB)
```
