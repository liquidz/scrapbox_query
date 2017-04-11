#![recursion_limit = "1024"]

extern crate clap;
extern crate rustc_serialize;
extern crate tantivy;
extern crate toml;
#[macro_use]
extern crate error_chain;

mod errors {
    error_chain!{}
}

use errors::*;
use clap::{Arg, App, SubCommand};
use rustc_serialize::json;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use tantivy::{Index, DocAddress};
use tantivy::collector::TopCollector;
use tantivy::query::QueryParser;
use tantivy::schema::*;

static CONFIG_FILE: &'static str = ".config/scrapq/config.toml";

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

#[derive(Debug, Clone, RustcDecodable)]
pub struct Config {
    index_path: String,
}

impl Default for Config {
    fn default() -> Config {
        Config { index_path: "".to_string() }
    }
}

fn read_file(filename: &str) -> Result<String> {
    let mut res = String::new();
    let mut f = File::open(filename).chain_err(|| "failed to open")?;
    f.read_to_string(&mut res).chain_err(|| "failed to read")?;
    Ok(res)
}

fn read_config(filename: &str) -> Result<Config> {
    let data = read_file(filename)?;
    let config = toml::decode_str(data.as_str()).ok_or("failed to decode toml")?;
    Ok(config)
}

fn doc_address_to_string(addr: DocAddress) -> String {
    format!("{}:{}", addr.segment_ord(), addr.doc())
}

fn string_to_doc_address(s: &str) -> Result<DocAddress> {
    let mut iter = s.split(':');
    let x = iter.next().ok_or("fixme")?;
    let y = iter.next().ok_or("fixme")?;
    let ux = x.parse::<u32>().chain_err(|| "failed to parse u32")?;
    let uy = y.parse::<u32>().chain_err(|| "failed to parse u32")?;
    Ok(DocAddress(ux, uy))
}

fn create_index(json_file: &str, index_path: &str) -> Result<bool> {
    let buf = read_file(json_file)?;
    let res: ScrapBox = json::decode(buf.as_str()).chain_err(|| "failed to decode json")?;

    let path = Path::new(index_path);

    let mut schema_builder = SchemaBuilder::default();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    let schema = schema_builder.build();

    let index = Index::create(path, schema.clone()).map_err(|_| "failed to create index")?;
    let mut index_writer = index.writer(100_000_000).map_err(|_| "failed to write index")?;

    for page in res.pages {
        let page_title = page.title.clone();
        let page_body = page.lines.clone().join("\n");

        let title = schema.get_field("title").ok_or("title filed is not exits")?;
        let body = schema.get_field("body").ok_or("body field is not exits")?;
        let mut doc = Document::default();

        doc.add_text(title, page_title.as_str());
        doc.add_text(body, page_body.as_str());
        index_writer.add_document(doc);
    }
    index_writer.commit().map_err(|_| "failed to commit index")?;

    Ok(true)
}

fn search(index_path: &str, query: &str) -> Result<Vec<SearchResult>> {
    let path = Path::new(index_path);
    let index = Index::open(path).map_err(|_| "failed to open index")?;

    let searcher = index.searcher();
    let schema = index.schema();
    let title_field = schema.get_field("title").ok_or("title field is not exists")?;
    let body_field = schema.get_field("body").ok_or("body field is not exists")?;
    let query_parser = QueryParser::new(schema.clone(), vec![title_field, body_field]);
    let query = query_parser.parse_query(query).map_err(|_| "failed to parse query")?;
    let mut top_collector = TopCollector::with_limit(10);
    query.search(&searcher, &mut top_collector).map_err(|_| "failed to search")?;

    let mut result: Vec<SearchResult> = vec![];
    for doc_address in top_collector.docs() {
        let retrieved_doc = searcher.doc(&doc_address).map_err(|_| "failed to retrieve document")?;
        let title = retrieved_doc.get_first(title_field).ok_or("document title is not exists")?;
        result.push(SearchResult {
            address: doc_address_to_string(doc_address),
            title: title.text().to_string(),
        });
    }
    Ok(result)
}

fn get(index_path: &str, address: &str) -> Result<String> {
    let path = Path::new(index_path);
    let index = Index::open(path).map_err(|_| "failed to open index")?;
    let searcher = index.searcher();
    let schema = index.schema();
    let body_field = schema.get_field("body").ok_or("body field is not exists")?;

    let doc_address = string_to_doc_address(address)?;
    let retrieved_doc = searcher.doc(&doc_address).map_err(|_| "failed to search document")?;
    let body = retrieved_doc.get_first(body_field).ok_or("document body is not exists")?;

    Ok(body.text().to_string())
}

fn main() {
    let matches = App::new("Scrapbox Query")
        .version("1.0")
        .author("Masashi Iizuka <liquidz.uo@gmail.com>")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Sets a custom config file")
            .takes_value(true))
        .subcommand(SubCommand::with_name("init")
            .about("Initialize index from a specified JSON file")
            .arg(Arg::with_name("json")
                .help("Sets a exported JSON file")
                .required(true)
                .index(1)
                .takes_value(true)))
        .subcommand(SubCommand::with_name("search")
            .about("Search documents and print search results")
            .arg(Arg::with_name("json")
                .long("json")
                .help("JSON format"))
            .arg(Arg::with_name("query")
                .required(true)
                .index(1)
                .takes_value(true)))
        .subcommand(SubCommand::with_name("get")
            .about("Retrieve a specified document")
            .arg(Arg::with_name("address")
                .help("Sets a document address")
                .required(true)
                .index(1)
                .takes_value(true)))
        .get_matches();

    let conf_name = match matches.value_of("config") {
        Some(path) => path.to_string(),
        None => {
            let home = env::home_dir().expect("HOME should be defined");
            let home = home.to_str().expect("failed to parse &str");
            format!("{}/{}", home, CONFIG_FILE)
        }
    };

    if let Ok(config) = read_config(conf_name.as_str()) {
        match matches.subcommand_name() {
            Some("init") => {
                let matches = matches.subcommand_matches("init")
                    .expect("should match 'init' subcommand");
                let json_file = matches.value_of("json").expect("should match 'json' value");
                println!("start to create index");
                if let Err(ref e) = create_index(json_file, config.index_path.as_str()) {
                    println!("{:?}", e);
                } else {
                    println!("finish to create index");
                }
            }
            Some("search") => {
                let matches = matches.subcommand_matches("search")
                    .expect("should match 'search' subcommand");
                let query = matches.value_of("query").expect("should match 'query' value");
                match search(config.index_path.as_str(), query) {
                    Ok(results) => {
                        if matches.is_present("json") {
                            println!("{}", json::encode(&results).expect("failed to encode JSON"));
                        } else {
                            for result in results {
                                println!("{}\t{}", result.address, result.title);
                            }
                        }
                    }
                    Err(ref e) => {
                        println!("{:?}", e);
                    }
                }

            }
            Some("get") => {
                let matches = matches.subcommand_matches("get")
                    .expect("should match 'get' subcommand");
                let address = matches.value_of("address").expect("should match 'address' value");
                match get(config.index_path.as_str(), address) {
                    Ok(body) => println!("{}", body),
                    Err(ref e) => println!("{:?}", e),
                }
            }
            e => println!("error: {:?}", e),
        }
    }
}


#[test]
fn test_convert_doc_address() {
    let s = "1:2";
    let addr = string_to_doc_address(s).unwrap();
    assert_eq!(doc_address_to_string(addr), s);
}
