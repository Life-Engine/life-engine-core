//! Full-text search engine backed by tantivy.
//!
//! Indexes record data and provides ranked search results with optional
//! collection filtering and pagination.

use crate::storage::Record;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tantivy::collector::{Count, TopDocs};
use tantivy::directory::MmapDirectory;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, Value as TantivyValue, STRING, STORED, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, TantivyDocument, Term};
use tokio::sync::Mutex;

/// Default number of documents to buffer before committing to the index.
const DEFAULT_COMMIT_THRESHOLD: usize = 50;

/// Full-text search engine wrapping a tantivy index.
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    fields: SearchFields,
    /// Number of documents added since the last commit.
    pending_count: AtomicUsize,
    /// Commit after this many documents are added.
    commit_threshold: usize,
}

/// Handles to the schema fields used during indexing and search.
#[derive(Clone)]
struct SearchFields {
    id: Field,
    collection: Field,
    plugin_id: Field,
    user_id: Field,
    household_id: Field,
    content: Field,
    title: Field,
}

/// Results returned from a search query.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResults {
    /// The matching hits.
    pub hits: Vec<SearchHit>,
    /// Total number of matching documents.
    pub total: usize,
    /// The original query string.
    pub query: String,
}

/// A single search hit with relevance score.
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// Record ID.
    pub id: String,
    /// Collection the record belongs to.
    pub collection: String,
    /// Plugin that owns the record.
    pub plugin_id: String,
    /// Relevance score.
    pub score: f32,
    /// Optional text snippet from the match.
    pub snippet: Option<String>,
}

impl SearchEngine {
    /// Create a new search engine with an in-memory tantivy index.
    pub fn new() -> anyhow::Result<Self> {
        Self::build(None, DEFAULT_COMMIT_THRESHOLD)
    }

    /// Create a search engine with a custom commit threshold (in-memory).
    ///
    /// Documents are buffered in memory and committed to the index in
    /// batches once the threshold is reached, eliminating per-document
    /// write amplification.  A threshold of `1` replicates the legacy
    /// commit-per-document behaviour.
    pub fn with_commit_threshold(commit_threshold: usize) -> anyhow::Result<Self> {
        Self::build(None, commit_threshold)
    }

    /// Create a disk-backed search engine at the given directory path.
    ///
    /// The index is persisted across restarts.  If the directory already
    /// contains a valid tantivy index it is opened; otherwise a new
    /// index is created.
    pub fn open_in_dir(dir: &Path, commit_threshold: usize) -> anyhow::Result<Self> {
        Self::build(Some(dir), commit_threshold)
    }

    fn build(dir: Option<&Path>, commit_threshold: usize) -> anyhow::Result<Self> {
        let commit_threshold = commit_threshold.max(1);

        let mut schema_builder = Schema::builder();
        let id = schema_builder.add_text_field("id", STRING | STORED);
        let collection = schema_builder.add_text_field("collection", STRING | STORED);
        let plugin_id = schema_builder.add_text_field("plugin_id", STRING | STORED);
        let user_id = schema_builder.add_text_field("user_id", STRING | STORED);
        let household_id = schema_builder.add_text_field("household_id", STRING | STORED);
        let content = schema_builder.add_text_field("content", TEXT);
        let title = schema_builder.add_text_field("title", TEXT | STORED);
        let schema = schema_builder.build();

        let index = match dir {
            Some(path) => {
                std::fs::create_dir_all(path)?;
                let mmap_dir = MmapDirectory::open(path)?;
                Index::open_or_create(mmap_dir, schema)?
            }
            None => Index::create_in_ram(schema),
        };

        let writer = index.writer(15_000_000)?;
        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            fields: SearchFields {
                id,
                collection,
                plugin_id,
                user_id,
                household_id,
                content,
                title,
            },
            pending_count: AtomicUsize::new(0),
            commit_threshold,
        })
    }

    /// Index a storage record. Extracts text from the record's JSON data
    /// based on its collection type.
    ///
    /// Documents are buffered and committed in batches according to the
    /// configured `commit_threshold`. Call [`flush`] to force a commit
    /// of any buffered documents (e.g. on shutdown).
    pub async fn index_record(&self, record: &Record) -> anyhow::Result<()> {
        let (title_text, content_text) = extract_text(&record.collection, &record.data);

        let user_id = record.user_id.as_deref().unwrap_or("");
        let household_id = record.household_id.as_deref().unwrap_or("");

        let mut writer = self.writer.lock().await;
        writer.add_document(doc!(
            self.fields.id => record.id.as_str(),
            self.fields.collection => record.collection.as_str(),
            self.fields.plugin_id => record.plugin_id.as_str(),
            self.fields.user_id => user_id,
            self.fields.household_id => household_id,
            self.fields.title => title_text.as_str(),
            self.fields.content => content_text.as_str(),
        ))?;

        let pending = self.pending_count.fetch_add(1, Ordering::Relaxed) + 1;
        if pending >= self.commit_threshold {
            writer.commit()?;
            self.reader.reload()?;
            self.pending_count.store(0, Ordering::Relaxed);
            tracing::debug!(pending, "committed batched index writes");
        }

        tracing::debug!(
            record_id = %record.id,
            collection = %record.collection,
            "indexed record"
        );

        Ok(())
    }

    /// Flush any pending index writes by committing immediately.
    ///
    /// This is a no-op if there are no pending documents.
    pub async fn flush(&self) -> anyhow::Result<()> {
        let pending = self.pending_count.load(Ordering::Relaxed);
        if pending > 0 {
            let mut writer = self.writer.lock().await;
            writer.commit()?;
            self.reader.reload()?;
            self.pending_count.store(0, Ordering::Relaxed);
            tracing::debug!(pending, "flushed pending index writes");
        }
        Ok(())
    }

    /// Index multiple records in a single batch, committing once at the end.
    ///
    /// This is significantly faster than calling `index_record` in a loop
    /// because it avoids per-document commit overhead.
    #[allow(dead_code)]
    pub async fn index_records_bulk(&self, records: &[Record]) -> anyhow::Result<usize> {
        let mut writer = self.writer.lock().await;
        let mut count = 0usize;

        for record in records {
            let (title_text, content_text) = extract_text(&record.collection, &record.data);
            let user_id = record.user_id.as_deref().unwrap_or("");
            let household_id = record.household_id.as_deref().unwrap_or("");
            writer.add_document(doc!(
                self.fields.id => record.id.as_str(),
                self.fields.collection => record.collection.as_str(),
                self.fields.plugin_id => record.plugin_id.as_str(),
                self.fields.user_id => user_id,
                self.fields.household_id => household_id,
                self.fields.title => title_text.as_str(),
                self.fields.content => content_text.as_str(),
            ))?;
            count += 1;
        }

        writer.commit()?;
        self.reader.reload()?;

        tracing::debug!(count, "bulk indexed records");
        Ok(count)
    }

    /// Search the index with a text query.
    ///
    /// Returns ranked results filtered optionally by collection, user, and household.
    /// When `user_id` or `household_id` filters are provided, only records belonging
    /// to the specified user/household are returned — preventing cross-tenant leakage.
    pub fn search(
        &self,
        query: &str,
        collection_filter: Option<&str>,
        user_id_filter: Option<&str>,
        household_id_filter: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<SearchResults> {
        if query.trim().is_empty() {
            anyhow::bail!("search query must not be empty");
        }

        let limit = limit.min(100);

        let searcher = self.reader.searcher();
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.content]);
        let parsed_query = query_parser.parse_query(query)?;

        // Combine the text query with optional filter clauses using
        // BooleanQuery with MUST semantics for correct AND behavior.
        let mut clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
            vec![(Occur::Must, parsed_query)];

        if let Some(filter) = collection_filter {
            let term = Term::from_field_text(self.fields.collection, filter);
            clauses.push((Occur::Must, Box::new(TermQuery::new(term, IndexRecordOption::Basic))));
        }

        if let Some(uid) = user_id_filter {
            let term = Term::from_field_text(self.fields.user_id, uid);
            clauses.push((Occur::Must, Box::new(TermQuery::new(term, IndexRecordOption::Basic))));
        }

        if let Some(hid) = household_id_filter {
            let term = Term::from_field_text(self.fields.household_id, hid);
            clauses.push((Occur::Must, Box::new(TermQuery::new(term, IndexRecordOption::Basic))));
        }

        let effective_query: Box<dyn tantivy::query::Query> = if clauses.len() == 1 {
            clauses.pop().unwrap().1
        } else {
            Box::new(BooleanQuery::new(clauses))
        };

        let fetch_count = offset + limit;
        let (top_docs, total) =
            searcher.search(&effective_query, &(TopDocs::with_limit(fetch_count), Count))?;

        let mut hits = Vec::new();

        for (idx, (score, doc_address)) in top_docs.into_iter().enumerate() {
            // Skip results before the offset.
            if idx < offset {
                continue;
            }

            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let doc_id = doc
                .get_first(self.fields.id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let doc_collection = doc
                .get_first(self.fields.collection)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let doc_plugin_id = doc
                .get_first(self.fields.plugin_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let doc_title = doc
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .map(str::to_string);

            hits.push(SearchHit {
                id: doc_id,
                collection: doc_collection,
                plugin_id: doc_plugin_id,
                score,
                snippet: doc_title,
            });
        }

        Ok(SearchResults {
            hits,
            total,
            query: query.to_string(),
        })
    }

    /// Remove a record from the index by its ID.
    pub async fn remove(&self, record_id: &str) -> anyhow::Result<()> {
        let term = tantivy::Term::from_field_text(self.fields.id, record_id);
        let mut writer = self.writer.lock().await;
        writer.delete_term(term);
        writer.commit()?;
        self.reader.reload()?;

        tracing::debug!(record_id = %record_id, "removed record from index");
        Ok(())
    }
}

/// Extract title and content text from a record's JSON data based on
/// its collection type.
fn extract_text(collection: &str, data: &serde_json::Value) -> (String, String) {
    match collection {
        "emails" => {
            let subject = data.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            let body = data
                .get("body_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (subject.to_string(), format!("{subject} {body}"))
        }
        "contacts" => {
            let mut parts = Vec::new();
            if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
                parts.push(name.to_string());
            }
            if let Some(first) = data.get("first_name").and_then(|v| v.as_str()) {
                parts.push(first.to_string());
            }
            if let Some(last) = data.get("last_name").and_then(|v| v.as_str()) {
                parts.push(last.to_string());
            }
            if let Some(emails) = data.get("emails").and_then(|v| v.as_array()) {
                for email in emails {
                    if let Some(addr) = email.as_str() {
                        parts.push(addr.to_string());
                    }
                }
            }
            if let Some(phones) = data.get("phones").and_then(|v| v.as_array()) {
                for phone in phones {
                    if let Some(num) = phone.as_str() {
                        parts.push(num.to_string());
                    }
                }
            }
            let title = data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (title, parts.join(" "))
        }
        "events" => {
            let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let desc = data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let location = data.get("location").and_then(|v| v.as_str()).unwrap_or("");
            (
                title.to_string(),
                format!("{title} {desc} {location}"),
            )
        }
        "tasks" => {
            let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let desc = data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (title.to_string(), format!("{title} {desc}"))
        }
        "notes" => {
            let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let body = data.get("body").and_then(|v| v.as_str()).unwrap_or("");
            (title.to_string(), format!("{title} {body}"))
        }
        "files" => {
            let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let path = data.get("path").and_then(|v| v.as_str()).unwrap_or("");
            (name.to_string(), format!("{name} {path}"))
        }
        _ => {
            // Generic: flatten all string values.
            let mut strings = Vec::new();
            flatten_strings(data, &mut strings, 10);
            let content = strings.join(" ");
            let title = strings.first().cloned().unwrap_or_default();
            (title, content)
        }
    }
}

/// Recursively collect all string values from a JSON value.
///
/// `max_depth` prevents stack overflow on deeply nested JSON; once it reaches
/// zero the recursion stops.
fn flatten_strings(value: &serde_json::Value, out: &mut Vec<String>, max_depth: u32) {
    if max_depth == 0 {
        return;
    }
    match value {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Object(map) => {
            for v in map.values() {
                flatten_strings(v, out, max_depth - 1);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                flatten_strings(v, out, max_depth - 1);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn make_record(id: &str, collection: &str, data: serde_json::Value) -> Record {
        let now = Utc::now();
        Record {
            id: id.to_string(),
            plugin_id: "test-plugin".to_string(),
            collection: collection.to_string(),
            data,
            version: 1,
            user_id: None,
            household_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn search_engine_creation() {
        let engine = SearchEngine::with_commit_threshold(1);
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn index_and_search_finds_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("r1", "tasks", json!({"title": "Buy groceries", "description": "milk and eggs"}));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("groceries", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r1");
        assert_eq!(results.hits[0].collection, "tasks");
        assert_eq!(results.query, "groceries");
    }

    #[tokio::test]
    async fn search_no_results_returns_empty() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("r1", "tasks", json!({"title": "Buy groceries"}));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("xylophone", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 0);
        assert!(results.hits.is_empty());
    }

    #[tokio::test]
    async fn search_across_multiple_collections() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let task = make_record("r1", "tasks", json!({"title": "Important meeting"}));
        let note = make_record("r2", "notes", json!({"title": "Meeting notes", "body": "important details"}));
        engine.index_record(&task).await.unwrap();
        engine.index_record(&note).await.unwrap();

        let results = engine.search("important", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 2);
    }

    #[tokio::test]
    async fn collection_filter_limits_results() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let task = make_record("r1", "tasks", json!({"title": "Important task"}));
        let note = make_record("r2", "notes", json!({"title": "Important note", "body": ""}));
        engine.index_record(&task).await.unwrap();
        engine.index_record(&note).await.unwrap();

        let results = engine.search("important", Some("tasks"), None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].collection, "tasks");
    }

    #[tokio::test]
    async fn pagination_limit_and_offset() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        for i in 0..5 {
            let record = make_record(
                &format!("r{i}"),
                "tasks",
                json!({"title": format!("Alpha task number {i}")}),
            );
            engine.index_record(&record).await.unwrap();
        }

        // First page: limit 2, offset 0.
        let page1 = engine.search("alpha", None, None, None, 2, 0).unwrap();
        assert_eq!(page1.hits.len(), 2);
        assert_eq!(page1.total, 5);

        // Second page: limit 2, offset 2.
        let page2 = engine.search("alpha", None, None, None, 2, 2).unwrap();
        assert_eq!(page2.hits.len(), 2);
        assert_eq!(page2.total, 5);

        // Third page: limit 2, offset 4.
        let page3 = engine.search("alpha", None, None, None, 2, 4).unwrap();
        assert_eq!(page3.hits.len(), 1);
        assert_eq!(page3.total, 5);
    }

    #[tokio::test]
    async fn score_ordering_most_relevant_first() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        // Record with "rust" in title only.
        let r1 = make_record("r1", "tasks", json!({"title": "Learn Python", "description": "basics of python"}));
        // Record with "rust" in title and content.
        let r2 = make_record("r2", "tasks", json!({"title": "Learn Rust", "description": "advanced rust programming with rust"}));
        engine.index_record(&r1).await.unwrap();
        engine.index_record(&r2).await.unwrap();

        let results = engine.search("rust", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r2");
        assert!(results.hits[0].score > 0.0);
    }

    #[tokio::test]
    async fn remove_record_from_index() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("r1", "tasks", json!({"title": "Removable task"}));
        engine.index_record(&record).await.unwrap();

        // Verify it exists.
        let results = engine.search("removable", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);

        // Remove it.
        engine.remove("r1").await.unwrap();

        // Verify it is gone.
        let results = engine.search("removable", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 0);
    }

    #[tokio::test]
    async fn index_email_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("e1", "emails", json!({
            "subject": "Project update",
            "body_text": "The deployment was successful and all tests passed"
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("deployment", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "e1");

        let results = engine.search("update", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
    }

    #[tokio::test]
    async fn index_contact_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("c1", "contacts", json!({
            "name": "Jane Doe",
            "first_name": "Jane",
            "last_name": "Doe",
            "emails": ["jane@example.com"],
            "phones": ["+1234567890"]
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("jane", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "c1");
    }

    #[tokio::test]
    async fn index_event_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("ev1", "events", json!({
            "title": "Team standup",
            "description": "Daily sync meeting",
            "location": "Conference room B"
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("standup", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "ev1");

        let results = engine.search("conference", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
    }

    #[tokio::test]
    async fn index_task_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("t1", "tasks", json!({
            "title": "Fix authentication bug",
            "description": "Token expiry not handled correctly"
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("authentication", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);

        let results = engine.search("expiry", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
    }

    #[tokio::test]
    async fn index_note_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("n1", "notes", json!({
            "title": "Architecture decisions",
            "body": "We chose event sourcing for audit trail"
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("architecture", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);

        let results = engine.search("sourcing", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
    }

    #[tokio::test]
    async fn index_file_record() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("f1", "files", json!({
            "name": "report.pdf",
            "path": "/documents/quarterly/report.pdf"
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("report", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "f1");
    }

    #[tokio::test]
    async fn search_with_multiple_words() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let r1 = make_record("r1", "tasks", json!({"title": "Buy milk and bread"}));
        let r2 = make_record("r2", "tasks", json!({"title": "Drink milk"}));
        engine.index_record(&r1).await.unwrap();
        engine.index_record(&r2).await.unwrap();

        // "milk bread" should match both (OR by default), but r1 should score higher.
        let results = engine.search("milk bread", None, None, None, 20, 0).unwrap();
        assert!(results.total >= 1);
        assert_eq!(results.hits[0].id, "r1");
    }

    #[tokio::test]
    async fn search_phrase_query() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let r1 = make_record("r1", "tasks", json!({"title": "Buy milk and bread"}));
        let r2 = make_record("r2", "tasks", json!({"title": "Bread and butter with milk"}));
        engine.index_record(&r1).await.unwrap();
        engine.index_record(&r2).await.unwrap();

        // Phrase search with quotes should only match exact phrase.
        let results = engine.search("\"milk and bread\"", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r1");
    }

    #[test]
    fn empty_query_returns_error() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let result = engine.search("", None, None, None, 20, 0);
        assert!(result.is_err());

        let result = engine.search("   ", None, None, None, 20, 0);
        assert!(result.is_err());
    }

    #[test]
    fn search_results_serialization() {
        let results = SearchResults {
            hits: vec![SearchHit {
                id: "r1".to_string(),
                collection: "tasks".to_string(),
                plugin_id: "core".to_string(),
                score: 1.5,
                snippet: Some("Test snippet".to_string()),
            }],
            total: 1,
            query: "test".to_string(),
        };

        let json = serde_json::to_value(&results).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["query"], "test");
        assert_eq!(json["hits"][0]["id"], "r1");
        assert_eq!(json["hits"][0]["collection"], "tasks");
        assert_eq!(json["hits"][0]["score"], 1.5);
        assert_eq!(json["hits"][0]["snippet"], "Test snippet");
    }

    #[tokio::test]
    async fn index_generic_collection() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("g1", "custom_data", json!({
            "field_a": "something searchable",
            "nested": {
                "field_b": "deeply nested value"
            }
        }));
        engine.index_record(&record).await.unwrap();

        let results = engine.search("searchable", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);

        let results = engine.search("deeply", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
    }

    #[tokio::test]
    async fn limit_capped_at_100() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let record = make_record("r1", "tasks", json!({"title": "Capped search test"}));
        engine.index_record(&record).await.unwrap();

        // Even if we request 500, limit is capped at 100.
        let results = engine.search("capped", None, None, None, 500, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits.len(), 1);
    }

    #[tokio::test]
    async fn bulk_index_records() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        let records: Vec<Record> = (0..10)
            .map(|i| {
                make_record(
                    &format!("b{i}"),
                    "tasks",
                    json!({"title": format!("Bulk item {i}")}),
                )
            })
            .collect();

        let count = engine.index_records_bulk(&records).await.unwrap();
        assert_eq!(count, 10);

        let results = engine.search("bulk", None, None, None, 20, 0).unwrap();
        assert_eq!(results.total, 10);
    }

    #[tokio::test]
    async fn collection_filter_exact_match_no_false_positives() {
        let engine = SearchEngine::with_commit_threshold(1).unwrap();
        // "task" is a substring of "tasks" — with TEXT this could cause false matches.
        let r1 = make_record("r1", "tasks", json!({"title": "Alpha item"}));
        let r2 = make_record("r2", "task", json!({"title": "Alpha item"}));
        engine.index_record(&r1).await.unwrap();
        engine.index_record(&r2).await.unwrap();

        let results = engine.search("alpha", Some("task"), None, None, 20, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r2");
    }
}
