// Example: tantivy-go-ffi in action
//
// Run with: cd example && go run .
// Or: make example
package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	"github.com/casedata-io/tantivy-go-ffi/go/tantivy"
)

type Movie struct {
	ID     string  `json:"id"`
	Title  string  `json:"title"`
	Year   int64   `json:"year"`
	Rating float64 `json:"rating"`
}

var movies = []Movie{
	{ID: "tt0111161", Title: "The Shawshank Redemption", Year: 1994, Rating: 9.3},
	{ID: "tt0068646", Title: "The Godfather", Year: 1972, Rating: 9.2},
	{ID: "tt0468569", Title: "The Dark Knight", Year: 2008, Rating: 9.0},
	{ID: "tt0108052", Title: "Schindler's List", Year: 1993, Rating: 9.0},
	{ID: "tt0167260", Title: "The Lord of the Rings: The Return of the King", Year: 2003, Rating: 9.0},
	{ID: "tt0137523", Title: "Fight Club", Year: 1999, Rating: 8.8},
	{ID: "tt0109830", Title: "Forrest Gump", Year: 1994, Rating: 8.8},
	{ID: "tt0120737", Title: "The Lord of the Rings: The Fellowship of the Ring", Year: 2001, Rating: 8.8},
	{ID: "tt0080684", Title: "Star Wars: Episode V - The Empire Strikes Back", Year: 1980, Rating: 8.7},
	{ID: "tt0133093", Title: "The Matrix", Year: 1999, Rating: 8.7},
}

func main() {
	// Create a temp dir for the index
	indexPath := filepath.Join(os.TempDir(), "tantivy-example-movies")
	fmt.Printf("📁 Index path: %s\n\n", indexPath)

	// Define schema
	schema := tantivy.Schema{
		Fields: []tantivy.FieldDef{
			{Name: "id", Type: "text", Stored: true, Indexed: true, Tokenizer: "raw"},
			{Name: "title", Type: "text", Stored: true, Indexed: true},
			{Name: "year", Type: "i64", Stored: true, Indexed: true, Fast: true},
			{Name: "rating", Type: "f64", Stored: true, Indexed: true, Fast: true},
		},
		SearchFields: []string{"title"},
	}

	// Create index
	idx, err := tantivy.Create(indexPath, schema)
	if err != nil {
		log.Fatalf("Create: %v", err)
	}
	defer idx.Close()

	// Index movies
	for _, m := range movies {
		if err := idx.AddDoc(m); err != nil {
			log.Fatalf("AddDoc: %v", err)
		}
	}
	if err := idx.Commit(); err != nil {
		log.Fatalf("Commit: %v", err)
	}
	fmt.Printf("✅ Indexed %d movies\n\n", idx.NumDocs())

	// --- Text search ---
	fmt.Println("🔍 Text search: 'dark knight'")
	results, err := idx.Search(tantivy.TextQuery("dark knight", 10))
	if err != nil {
		log.Fatalf("Search: %v", err)
	}
	printResults(results)

	// --- Fuzzy search (typo-tolerant) ---
	fmt.Println("🔍 Fuzzy search: 'godfahter' (with typo)")
	results, _ = idx.Search(tantivy.FuzzyQuery("godfahter", 2, 10))
	printResults(results)

	// --- Phrase search ---
	fmt.Println("🔍 Phrase search: 'lord of the rings'")
	results, _ = idx.Search(tantivy.PhraseQuery("lord of the rings", 10))
	printResults(results)

	// --- Prefix search ---
	fmt.Println("🔍 Prefix search: 'star'")
	results, _ = idx.Search(tantivy.PrefixQuery("star", 10))
	printResults(results)

	// --- Exact term match ---
	fmt.Println("🔍 Exact term match: id = 'tt0133093'")
	results, _ = idx.Search(tantivy.TermMatchQuery("id", "tt0133093", 10))
	printResults(results)

	// --- Range query ---
	minYear := int64(1990)
	maxYear := int64(1999)
	fmt.Println("🔍 Range query: year 1990–1999")
	results, _ = idx.Search(tantivy.RangeI64Query("year", &minYear, &maxYear, 100))
	printResults(results)

	// --- Float range ---
	minRating := 9.0
	fmt.Println("🔍 Float range: rating >= 9.0")
	results, _ = idx.Search(tantivy.RangeF64Query("rating", &minRating, nil, 100))
	printResults(results)

	// --- Boolean query (text + range) ---
	fmt.Println("🔍 Bool query: title contains 'ring' AND year 2000–2010")
	min2000 := int64(2000)
	max2010 := int64(2010)
	results, err = idx.Search(tantivy.BoolQuery(
		[]map[string]interface{}{
			tantivy.TextQuery("ring", 0),
			tantivy.RangeI64Query("year", &min2000, &max2010, 0),
		},
		nil, nil, 100,
	))
	if err != nil {
		log.Fatalf("Bool search: %v", err)
	}
	printResults(results)

	// --- Re-open existing index ---
	idx.Close()
	fmt.Println("📂 Re-opening existing index...")
	idx2, err := tantivy.Open(indexPath)
	if err != nil {
		log.Fatalf("Open: %v", err)
	}
	defer idx2.Close()
	fmt.Printf("   Found %d documents\n\n", idx2.NumDocs())

	fmt.Println("✅ All done!")
}

func printResults(results *tantivy.SearchResults) {
	fmt.Printf("   Found %d results:\n", results.Count)
	for _, r := range results.Results {
		title, _ := r["title"].(string)
		id, _ := r["id"].(string)
		year, _ := r["year"].(float64) // JSON numbers → float64
		rating, _ := r["rating"].(float64)
		fmt.Printf("   • [%s] %s (%d) ⭐ %.1f\n", id, title, int(year), rating)
	}
	fmt.Println()
}
