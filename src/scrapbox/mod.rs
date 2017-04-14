extern crate tantivy;

use errors::*;
use rustc_serialize::json;
use self::tantivy::collector::TopCollector;
use self::tantivy::query::QueryParser;
use self::tantivy::schema::*;
use self::tantivy::Index;
use std::fmt;
use std::path::Path;

mod address;

#[derive(Debug, Clone, RustcDecodable, RustcEncodable)]
pub struct ScrapBox {
    pub name: String,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, RustcDecodable, RustcEncodable)]
pub struct Page {
    title: String,
    lines: Vec<String>,
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct SearchResult {
    address: String,
    title: String,
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct GetResult {
    title: String,
    body: String,
}

impl fmt::Display for SearchResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\t{}", self.address, self.title)
    }
}

pub fn initialize_index(json_data: &str, index_path: &str) -> Result<bool> {
    let res: ScrapBox = json::decode(json_data)
        .chain_err(|| "failed to decode json")?;

    let path = Path::new(index_path);

    let mut schema_builder = SchemaBuilder::default();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    let schema = schema_builder.build();

    let index = Index::create(path, schema.clone())
        .map_err(|_| "failed to create index")?;
    let mut index_writer = index
        .writer(100_000_000)
        .map_err(|_| "failed to write index")?;

    for page in res.pages {
        let page_title = page.title.clone();
        let page_body = page.lines.clone().join("\n");

        let title = schema
            .get_field("title")
            .ok_or("title filed is not exits")?;
        let body = schema
            .get_field("body")
            .ok_or("body field is not exits")?;
        let mut doc = Document::default();

        doc.add_text(title, page_title.as_str());
        doc.add_text(body, page_body.as_str());
        index_writer.add_document(doc);
    }
    index_writer
        .commit()
        .map_err(|e| format!("failed to commit index: {:?}", e))?;

    Ok(true)
}

pub fn search_documents(index_path: &str, query: &str) -> Result<Vec<SearchResult>> {
    let path = Path::new(index_path);
    let index = Index::open(path).map_err(|_| "failed to open index")?;

    let searcher = index.searcher();
    let schema = index.schema();
    let title_field = schema
        .get_field("title")
        .ok_or("title field is not exists")?;
    let body_field = schema
        .get_field("body")
        .ok_or("body field is not exists")?;
    let query_parser = QueryParser::new(schema.clone(), vec![title_field, body_field]);
    let query = query_parser
        .parse_query(query)
        .map_err(|_| "failed to parse query")?;
    let mut top_collector = TopCollector::with_limit(10);
    query
        .search(&searcher, &mut top_collector)
        .map_err(|_| "failed to search")?;

    let mut result: Vec<SearchResult> = vec![];
    for doc_address in top_collector.docs() {
        let retrieved_doc = searcher
            .doc(&doc_address)
            .map_err(|_| "failed to retrieve document")?;
        let title = retrieved_doc
            .get_first(title_field)
            .ok_or("document title is not exists")?;
        result.push(SearchResult {
                        address: address::doc_address_to_string(doc_address),
                        title: title.text().to_string(),
                    });
    }
    Ok(result)
}

pub fn retrieve_document(index_path: &str, address: &str) -> Result<String> {
    let path = Path::new(index_path);
    let index = Index::open(path).map_err(|_| "failed to open index")?;
    let searcher = index.searcher();
    let schema = index.schema();
    let body_field = schema
        .get_field("body")
        .ok_or("body field is not exists")?;

    let doc_address = address::string_to_doc_address(address)?;
    let retrieved_doc = searcher
        .doc(&doc_address)
        .map_err(|_| "failed to search document")?;
    let body = retrieved_doc
        .get_first(body_field)
        .ok_or("document body is not exists")?;

    Ok(body.text().to_string())
}
