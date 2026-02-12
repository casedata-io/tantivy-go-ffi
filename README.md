# tantivy-go-ffi

Generic [Tantivy](https://github.com/quickwit-oss/tantivy) FFI bindings for Go via CGO. Schema-agnostic — works with any data type.

**Pre-built native libraries are included** — no Rust toolchain required to use this in your Go project.

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

## Prerequisites

- **Go** (1.21+)
- **C compiler** — Xcode Command Line Tools (macOS) or `gcc`/`clang` (Linux)
- **Rust** — Only needed if you want to rebuild the native library yourself

## Installation

### Just `go get` it — no Rust needed!

Pre-built static libraries for all supported platforms are included in the `libs/` directory. CGO links against them automatically.

```bash
go get github.com/casedata-io/tantivy-go-ffi
```

```go
import "github.com/casedata-io/tantivy-go-ffi/go/tantivy"
```

That's it. No Rust, no `make`, no system-wide install.

### For local development

If you want to hack on this package alongside your project:

```go
// In your project's go.mod:
require github.com/casedata-io/tantivy-go-ffi v0.0.0

replace github.com/casedata-io/tantivy-go-ffi => ../tantivy-go-ffi
```

### Rebuilding the native library (optional)

If you need to rebuild from Rust source (e.g., to update Tantivy version):

```bash
# Requires Rust: https://rustup.rs/
make build    # builds + copies to libs/<platform>/
```

## Quick Start

```go
package main

import (
	"fmt"
	"log"

	"github.com/casedata-io/tantivy-go-ffi/go/tantivy"
)

func main() {
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
	idx, err := tantivy.Create("/tmp/myindex", schema)
	if err != nil {
		log.Fatal(err)
	}
	defer idx.Close()

	// 3. Add documents (any struct or map)
	idx.AddDoc(map[string]interface{}{
		"id": "tt0111161", "title": "The Shawshank Redemption",
		"year": 1994, "rating": 9.3,
	})
	idx.Commit()

	fmt.Printf("Indexed %d documents\n", idx.NumDocs())

	// 4. Search
	results, _ := idx.Search(tantivy.TextQuery("shawshank", 10))
	for _, r := range results.Results {
		fmt.Printf("  %s — %s (%.1f)\n", r["id"], r["title"], r["rating"])
	}
}
```

See [`example/main.go`](example/main.go) for a complete working example with all query types.

Run it with:

```bash
make example
```

## Using with Structs

```go
type Movie struct {
	ID     string  `json:"id"`
	Title  string  `json:"title"`
	Year   int64   `json:"year"`
	Rating float64 `json:"rating"`
}

movie := Movie{
	ID:     "tt0111161",
	Title:  "The Shawshank Redemption",
	Year:   1994,
	Rating: 9.3,
}
idx.AddDoc(movie)
```

Or raw JSON:

```go
idx.AddDocJSON([]byte(`{"id":"tt0111161","title":"The Shawshank Redemption","year":1994,"rating":9.3}`))
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

### Go Query Helpers

```go
tantivy.TextQuery("batman", 100)
tantivy.FuzzyQuery("batmna", 2, 100)
tantivy.PhraseQuery("the dark knight", 100)
tantivy.PrefixQuery("bat", 100)
tantivy.TermMatchQuery("id", "tt0111161", 1)
tantivy.RangeI64Query("year", &min, &max, 100)
tantivy.RangeF64Query("rating", &minRating, nil, 100)
tantivy.BoolQuery(must, should, mustNot, 100)
```

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

**Tokenizers:** `default` (standard English), `raw` (exact match / no tokenization), `en_stem` (English stemming)

**Fast fields:** Enable columnar access for efficient sorting/filtering/aggregation

## API Reference

| Go Function | Description |
|---|---|
| `tantivy.Create(path, schema)` | Create a new index at path with the given schema |
| `tantivy.Open(path)` | Open an existing index (reads `_schema.json` from index dir) |
| `idx.Close()` | Free the index handle |
| `idx.AddDoc(doc)` | Add a document (struct or map, marshaled to JSON) |
| `idx.AddDocJSON(json)` | Add a document from raw JSON bytes |
| `idx.Commit()` | Commit pending writes to disk |
| `idx.NumDocs()` | Get the number of indexed documents |
| `idx.Search(query)` | Search using a query map (from helper functions) |
| `idx.SearchJSON(json)` | Search using raw JSON query bytes |

## Platform Support

Pre-built static libraries are provided for:

| Platform | Status |
|---|---|
| macOS arm64 (Apple Silicon) | ✅ |
| macOS amd64 (Intel) | ✅ |
| Linux amd64 | ✅ |
| Linux arm64 | ✅ |
| Windows | ❌ Not yet |

Libraries are built automatically via GitHub Actions on each tagged release and committed to `libs/`.

## Makefile Targets

| Target | Description |
|---|---|
| `make build` | Build Rust lib + copy to `libs/` (requires Rust) |
| `make example` | Run the example (no Rust needed) |
| `make install` | Install lib system-wide to `/usr/local` |
| `make uninstall` | Remove system-wide install |
| `make clean` | Remove build artifacts |

## Files

```
tantivy-go-ffi/
├── Cargo.toml              # Rust crate config
├── Cargo.lock              # Rust dependency lock
├── Makefile                # Build, install, example targets
├── go.mod                  # Go module definition
├── tantivy_go.h            # C header for FFI
├── src/
│   ├── lib.rs              # Generic Tantivy wrapper
│   └── ffi.rs              # C FFI layer (7 functions)
├── go/
│   └── tantivy/
│       └── client.go       # Go client + query helpers
├── libs/                   # Pre-built static libraries (committed to git)
│   ├── tantivy_go.h        # Header copy for CGO include path
│   ├── darwin_arm64/
│   │   └── libtantivy_go.a
│   ├── darwin_amd64/
│   │   └── libtantivy_go.a
│   ├── linux_amd64/
│   │   └── libtantivy_go.a
│   └── linux_arm64/
│       └── libtantivy_go.a
├── example/
│   ├── go.mod
│   └── main.go             # Full working example (movies)
└── .github/
    └── workflows/
        └── build-libs.yml  # CI: cross-compile for all platforms
```

## License

MIT
