//! Generic Tantivy FFI Crate
//!
//! Schema-agnostic wrapper. Schema defined via JSON, documents added as JSON,
//! queries expressed as a JSON DSL. Reusable for any data type.

pub mod ffi;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{
    BooleanQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, QueryParser,
    RegexQuery, TermQuery,
};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

// ========== Schema Definition ==========

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,   // "text", "i64", "f64"
    #[serde(default = "yes")]
    pub stored: bool,
    #[serde(default = "yes")]
    pub indexed: bool,
    #[serde(default)]
    pub fast: bool,
    #[serde(default = "default_tok")]
    pub tokenizer: String,    // "default", "raw", "en_stem"
}

fn yes() -> bool { true }
fn default_tok() -> String { "default".to_string() }

#[derive(Serialize, Deserialize, Debug)]
pub struct SchemaDef {
    pub fields: Vec<FieldDef>,
    #[serde(default)]
    pub search_fields: Vec<String>,
}

// ========== Query DSL ==========

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum QueryDef {
    #[serde(rename = "text")]
    Text {
        query: String,
        #[serde(default)]
        fields: Vec<String>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "fuzzy")]
    Fuzzy {
        term: String,
        #[serde(default = "default_dist")]
        distance: u8,
        #[serde(default)]
        fields: Vec<String>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "phrase")]
    Phrase {
        phrase: String,
        #[serde(default)]
        fields: Vec<String>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "prefix")]
    Prefix {
        prefix: String,
        #[serde(default)]
        fields: Vec<String>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "term_match")]
    TermMatch {
        field: String,
        value: serde_json::Value,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "range_i64")]
    RangeI64 {
        field: String,
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "range_f64")]
    RangeF64 {
        field: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "bool")]
    Bool {
        #[serde(default)]
        must: Vec<QueryDef>,
        #[serde(default)]
        should: Vec<QueryDef>,
        #[serde(default)]
        must_not: Vec<QueryDef>,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
    #[serde(rename = "all")]
    All {
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
    },
}

fn default_limit() -> usize { 100 }
fn default_dist() -> u8 { 2 }

impl QueryDef {
    fn limit(&self) -> usize {
        match self {
            QueryDef::Text { limit, .. } => *limit,
            QueryDef::Fuzzy { limit, .. } => *limit,
            QueryDef::Phrase { limit, .. } => *limit,
            QueryDef::Prefix { limit, .. } => *limit,
            QueryDef::TermMatch { limit, .. } => *limit,
            QueryDef::RangeI64 { limit, .. } => *limit,
            QueryDef::RangeF64 { limit, .. } => *limit,
            QueryDef::Bool { limit, .. } => *limit,
            QueryDef::All { limit, .. } => *limit,
        }
    }

    fn offset(&self) -> usize {
        match self {
            QueryDef::Text { offset, .. } => *offset,
            QueryDef::Fuzzy { offset, .. } => *offset,
            QueryDef::Phrase { offset, .. } => *offset,
            QueryDef::Prefix { offset, .. } => *offset,
            QueryDef::TermMatch { offset, .. } => *offset,
            QueryDef::RangeI64 { offset, .. } => *offset,
            QueryDef::RangeF64 { offset, .. } => *offset,
            QueryDef::Bool { offset, .. } => *offset,
            QueryDef::All { offset, .. } => *offset,
        }
    }
}

// ========== Results ==========

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchResults {
    pub results: Vec<serde_json::Value>,
    pub count: usize,
    pub total_count: usize,
    pub limit: usize,
    pub offset: usize,
}

// ========== Index ==========

pub struct TantivyIndex {
    index: Index,
    reader: IndexReader,
    writer: Mutex<IndexWriter>,
    #[allow(dead_code)]
    schema: Schema,
    field_map: HashMap<String, (Field, FieldDef)>,
    search_fields: Vec<Field>,
}

impl TantivyIndex {
    pub fn create(path: &str, schema_json: &str) -> Result<Self, String> {
        let _ = std::fs::remove_dir_all(path);
        std::fs::create_dir_all(path).map_err(|e| format!("mkdir: {}", e))?;

        let schema_def: SchemaDef =
            serde_json::from_str(schema_json).map_err(|e| format!("schema: {}", e))?;
        let (schema, fmap) = Self::build_schema(&schema_def)?;

        std::fs::write(Path::new(path).join("_schema.json"), schema_json)
            .map_err(|e| format!("save schema: {}", e))?;

        let index =
            Index::create_in_dir(Path::new(path), schema.clone()).map_err(|e| e.to_string())?;
        let sf = Self::resolve_search_fields(&schema_def, &fmap);
        Self::finish(index, schema, fmap, sf)
    }

    pub fn open(path: &str) -> Result<Self, String> {
        let sj = std::fs::read_to_string(Path::new(path).join("_schema.json"))
            .map_err(|e| format!("read schema: {}", e))?;
        let schema_def: SchemaDef =
            serde_json::from_str(&sj).map_err(|e| format!("schema: {}", e))?;
        let (schema, fmap) = Self::build_schema(&schema_def)?;
        let index = Index::open_in_dir(Path::new(path)).map_err(|e| e.to_string())?;
        let sf = Self::resolve_search_fields(&schema_def, &fmap);
        Self::finish(index, schema, fmap, sf)
    }

    fn finish(index: Index, schema: Schema, fmap: HashMap<String, (Field, FieldDef)>, sf: Vec<Field>) -> Result<Self, String> {
        let reader = index.reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into().map_err(|e| format!("reader: {}", e))?;
        let writer = index.writer(256_000_000).map_err(|e| format!("writer: {}", e))?;
        Ok(TantivyIndex { index, reader, writer: Mutex::new(writer), schema, field_map: fmap, search_fields: sf })
    }

    fn build_schema(def: &SchemaDef) -> Result<(Schema, HashMap<String, (Field, FieldDef)>), String> {
        let mut sb = Schema::builder();
        let mut fm = HashMap::new();
        for fd in &def.fields {
            let field = match fd.field_type.as_str() {
                "text" => {
                    let mut o = TextOptions::default();
                    if fd.stored { o = o.set_stored(); }
                    if fd.indexed {
                        let tok = match fd.tokenizer.as_str() { "raw" => "raw", "en_stem" => "en_stem", _ => "default" };
                        let rec = if fd.tokenizer == "raw" { IndexRecordOption::Basic } else { IndexRecordOption::WithFreqsAndPositions };
                        o = o.set_indexing_options(TextFieldIndexing::default().set_tokenizer(tok).set_index_option(rec));
                    }
                    if fd.fast { o = o.set_fast(None); }
                    sb.add_text_field(&fd.name, o)
                }
                "i64" => {
                    let mut o = NumericOptions::default();
                    if fd.stored { o = o.set_stored(); }
                    if fd.indexed { o = o.set_indexed(); }
                    if fd.fast { o = o.set_fast(); }
                    sb.add_i64_field(&fd.name, o)
                }
                "f64" => {
                    let mut o = NumericOptions::default();
                    if fd.stored { o = o.set_stored(); }
                    if fd.indexed { o = o.set_indexed(); }
                    if fd.fast { o = o.set_fast(); }
                    sb.add_f64_field(&fd.name, o)
                }
                t => return Err(format!("unknown type: {}", t)),
            };
            fm.insert(fd.name.clone(), (field, fd.clone()));
        }
        Ok((sb.build(), fm))
    }

    fn resolve_search_fields(def: &SchemaDef, fm: &HashMap<String, (Field, FieldDef)>) -> Vec<Field> {
        if !def.search_fields.is_empty() {
            def.search_fields.iter().filter_map(|n| fm.get(n).map(|(f, _)| *f)).collect()
        } else {
            fm.values().filter(|(_, d)| d.field_type == "text" && d.indexed && d.tokenizer != "raw").map(|(f, _)| *f).collect()
        }
    }

    // ===== Document Operations =====

    pub fn add_doc(&self, doc_json: &str) -> Result<(), String> {
        let map: HashMap<String, serde_json::Value> =
            serde_json::from_str(doc_json).map_err(|e| format!("doc: {}", e))?;
        let mut doc = TantivyDocument::new();
        for (name, val) in &map {
            if let Some((field, fd)) = self.field_map.get(name) {
                match fd.field_type.as_str() {
                    "text" => { if let Some(s) = val.as_str() { doc.add_text(*field, s); } }
                    "i64" => {
                        if let Some(n) = val.as_i64() { doc.add_i64(*field, n); }
                        else if let Some(n) = val.as_f64() { doc.add_i64(*field, n as i64); }
                    }
                    "f64" => { if let Some(n) = val.as_f64() { doc.add_f64(*field, n); } }
                    _ => {}
                }
            }
        }
        let w = self.writer.lock().map_err(|e| e.to_string())?;
        w.add_document(doc).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn commit(&self) -> Result<(), String> {
        let mut w = self.writer.lock().map_err(|e| e.to_string())?;
        w.commit().map_err(|e| e.to_string())?;
        self.reader.reload().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn num_docs(&self) -> u64 { self.reader.searcher().num_docs() }

    // ===== Search =====

    pub fn search(&self, query_json: &str) -> Result<SearchResults, String> {
        let qd: QueryDef = serde_json::from_str(query_json).map_err(|e| format!("query: {}", e))?;
        let limit = qd.limit();
        let offset = qd.offset();
        let query = self.build_query(&qd)?;
        self.exec(query, limit, offset)
    }

    fn exec(&self, query: Box<dyn Query>, limit: usize, offset: usize) -> Result<SearchResults, String> {
        let searcher = self.reader.searcher();

        // Use TopDocs with offset for proper pagination + Count for total matching docs
        let collector = TopDocs::with_limit(limit).and_offset(offset);
        let (total_count, top) = searcher.search(&query, &(Count, collector)).map_err(|e| e.to_string())?;

        let mut results = Vec::with_capacity(top.len());
        for (score, addr) in &top {
            let doc: TantivyDocument = searcher.doc(*addr).map_err(|e| e.to_string())?;
            let mut obj = serde_json::Map::new();
            for (name, (field, fd)) in &self.field_map {
                match fd.field_type.as_str() {
                    "text" => {
                        if let Some(v) = doc.get_first(*field) {
                            if let Some(s) = v.as_str() { obj.insert(name.clone(), serde_json::Value::String(s.to_string())); }
                        }
                    }
                    "i64" => {
                        if let Some(v) = doc.get_first(*field) {
                            if let Some(n) = v.as_i64() { obj.insert(name.clone(), serde_json::json!(n)); }
                        }
                    }
                    "f64" => {
                        if let Some(v) = doc.get_first(*field) {
                            if let Some(n) = v.as_f64() { obj.insert(name.clone(), serde_json::json!(n)); }
                        }
                    }
                    _ => {}
                }
            }
            obj.insert("_score".to_string(), serde_json::json!(score));
            results.push(serde_json::Value::Object(obj));
        }
        let count = results.len();
        Ok(SearchResults { results, count, total_count, limit, offset })
    }

    fn build_query(&self, qd: &QueryDef) -> Result<Box<dyn Query>, String> {
        match qd {
            QueryDef::Text { query, fields, .. } => {
                let f = self.resolve_fields(fields);
                let qp = QueryParser::for_index(&self.index, f);
                qp.parse_query(query).map_err(|e| e.to_string())
            }
            QueryDef::Fuzzy { term, distance, fields, .. } => {
                let f = self.resolve_fields(fields);
                let words: Vec<String> = term.split_whitespace()
                    .map(|w| w.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>())
                    .filter(|w| w.len() > 1)
                    .collect();
                if words.is_empty() {
                    return Ok(Box::new(BooleanQuery::new(vec![])));
                }
                let mut word_clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                for word in &words {
                    // Adaptive distance: short words use dist 1 to avoid over-matching
                    let eff_dist = if word.len() <= 5 { 1u8.min(*distance) } else { *distance };
                    let per_field: Vec<(Occur, Box<dyn Query>)> = f.iter().map(|fld| {
                        let t = tantivy::Term::from_field_text(*fld, word);
                        (Occur::Should, Box::new(FuzzyTermQuery::new(t, eff_dist, true)) as Box<dyn Query>)
                    }).collect();
                    word_clauses.push((Occur::Must, Box::new(BooleanQuery::new(per_field)) as Box<dyn Query>));
                }
                Ok(Box::new(BooleanQuery::new(word_clauses)))
            }
            QueryDef::Phrase { phrase, fields, .. } => {
                let f = self.resolve_fields(fields);
                let words: Vec<&str> = phrase.split_whitespace().collect();
                if words.len() < 2 {
                    let qp = QueryParser::for_index(&self.index, f);
                    return qp.parse_query(phrase).map_err(|e| e.to_string());
                }
                let clauses: Vec<(Occur, Box<dyn Query>)> = f.iter().map(|fld| {
                    let terms: Vec<tantivy::Term> = words.iter()
                        .map(|w| tantivy::Term::from_field_text(*fld, &w.to_lowercase()))
                        .collect();
                    (Occur::Should, Box::new(PhraseQuery::new(terms)) as Box<dyn Query>)
                }).collect();
                Ok(Box::new(BooleanQuery::new(clauses)))
            }
            QueryDef::Prefix { prefix, fields, .. } => {
                let f = self.resolve_fields(fields);
                let pat = format!("{}.*", regex_escape(&prefix.to_lowercase()));
                let clauses: Result<Vec<(Occur, Box<dyn Query>)>, String> = f.iter().map(|fld| {
                    let rq = RegexQuery::from_pattern(&pat, *fld).map_err(|e| e.to_string())?;
                    Ok((Occur::Should, Box::new(rq) as Box<dyn Query>))
                }).collect();
                Ok(Box::new(BooleanQuery::new(clauses?)))
            }
            QueryDef::TermMatch { field, value, .. } => {
                let (fld, fd) = self.field_map.get(field)
                    .ok_or_else(|| format!("unknown field: {}", field))?;
                match fd.field_type.as_str() {
                    "text" => {
                        let s = value.as_str().unwrap_or("");
                        let t = tantivy::Term::from_field_text(*fld, s);
                        Ok(Box::new(TermQuery::new(t, IndexRecordOption::Basic)))
                    }
                    "i64" => {
                        let n = value.as_i64().unwrap_or(0);
                        let t = tantivy::Term::from_field_i64(*fld, n);
                        Ok(Box::new(TermQuery::new(t, IndexRecordOption::Basic)))
                    }
                    "f64" => {
                        let n = value.as_f64().unwrap_or(0.0);
                        let t = tantivy::Term::from_field_f64(*fld, n);
                        Ok(Box::new(TermQuery::new(t, IndexRecordOption::Basic)))
                    }
                    _ => Err("unsupported term type".to_string()),
                }
            }
            QueryDef::RangeI64 { field, min, max, .. } => {
                let lo = min.unwrap_or(i64::MIN);
                let hi = max.map(|v| v + 1).unwrap_or(i64::MAX);
                let q = tantivy::query::RangeQuery::new_i64(field.clone(), lo..hi);
                Ok(Box::new(q))
            }
            QueryDef::RangeF64 { field, min, max, .. } => {
                let lo_bound = match min {
                    Some(v) => std::ops::Bound::Included(*v),
                    None => std::ops::Bound::Unbounded,
                };
                let hi_bound = match max {
                    Some(v) => std::ops::Bound::Included(*v),
                    None => std::ops::Bound::Unbounded,
                };
                let q = tantivy::query::RangeQuery::new_f64_bounds(field.clone(), lo_bound, hi_bound);
                Ok(Box::new(q))
            }
            QueryDef::Bool { must, should, must_not, .. } => {
                let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                for sub in must { clauses.push((Occur::Must, self.build_query(sub)?)); }
                for sub in should { clauses.push((Occur::Should, self.build_query(sub)?)); }
                for sub in must_not { clauses.push((Occur::MustNot, self.build_query(sub)?)); }
                Ok(Box::new(BooleanQuery::new(clauses)))
            }
            QueryDef::All { .. } => {
                Ok(Box::new(tantivy::query::AllQuery))
            }
        }
    }

    fn resolve_fields(&self, names: &[String]) -> Vec<Field> {
        if names.is_empty() {
            self.search_fields.clone()
        } else {
            names.iter().filter_map(|n| self.field_map.get(n).map(|(f, _)| *f)).collect()
        }
    }
}

fn regex_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if "\\.*+?()[]{}^$|".contains(c) { o.push('\\'); }
        o.push(c);
    }
    o
}
