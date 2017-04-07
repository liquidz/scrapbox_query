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
    json_file: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            index_path: "".to_string(),
            json_file: "".to_string(),
        }
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
        index_writer.add_document(doc).chain_err(|| "failed to add document to index")?;
    }
    index_writer.commit().map_err(|_| "failed to commit index")?;

    Ok(true)
}

fn search(index_path: &str, query: &str) -> Vec<SearchResult> {
    let path = Path::new(index_path);
    let index = Index::open(path).unwrap();

    let searcher = index.searcher();
    let schema = index.schema();
    let title_field = schema.get_field("title").unwrap();
    let body_field = schema.get_field("body").unwrap();
    let query_parser = QueryParser::new(schema.clone(), vec![title_field, body_field]);
    let query = query_parser.parse_query(query).unwrap();
    let mut top_collector = TopCollector::with_limit(10);
    query.search(&searcher, &mut top_collector).unwrap();

    let mut result: Vec<SearchResult> = vec![];
    for doc_address in top_collector.docs() {
        let retrieved_doc = searcher.doc(&doc_address).unwrap();
        let title = retrieved_doc.get_first(title_field).unwrap();
        result.push(SearchResult {
            address: doc_address_to_string(doc_address),
            title: title.text().to_string(),
        });
    }
    result
}

fn get(index_path: &str, address: &str) -> Result<String> {
    let path = Path::new(index_path);
    //let index = Index::open(path).chain_err(|| "failed to open index")?;
    let index = Index::open(path).map_err(|_| "failed to open index")?;
    let searcher = index.searcher();
    let schema = index.schema();
    //let body_field = schema.get_field("body").chain_err(|| "failed to get body field")?;
    let body_field = schema.get_field("body").ok_or("body field is not exists")?;

    let doc_address = string_to_doc_address(address)?;
    //let retrieved_doc = searcher.doc(&doc_address).chain_err(|| "failed to search document")?;
    let retrieved_doc = searcher.doc(&doc_address).map_err(|_| "failed to search document")?;
    let body = retrieved_doc.get_first(body_field).ok_or("document body is not exists")?;

    Ok(body.text().to_string())
}

fn main() {
    let matches = App::new("My Super Program")
        .version("1.0")
        .author("Masashi Iizuka <liquidz.uo@gmail.com>")
        .about("Does awesome things")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Sets a custom config file")
            .takes_value(true))
        .subcommand(SubCommand::with_name("index").about("create index"))
        .subcommand(SubCommand::with_name("search")
            .about("create index")
            .arg(Arg::with_name("json")
                .long("json")
                .help("JSON format"))
            .arg(Arg::with_name("query")
                .required(true)
                .index(1)
                .takes_value(true)))
        .subcommand(SubCommand::with_name("get")
            .about("get document")
            .arg(Arg::with_name("address")
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
            Some("index") => {
                println!("start to create index");
                if let Err(ref e) = create_index(config.json_file.as_str(),
                                                 config.index_path.as_str()) {
                    println!("{:?}", e);
                } else {
                    println!("finish to create index");
                }
            }
            Some("search") => {
                let matches = matches.subcommand_matches("search").unwrap();
                let query = matches.value_of("query").unwrap();
                let results = search(config.index_path.as_str(), query);

                if matches.is_present("json") {
                    println!("{}", json::encode(&results).expect("failed to encode JSON"));
                } else {
                    for result in results {
                        println!("{}\t{}", result.address, result.title);
                    }
                }
            }
            Some("get") => {
                let matches = matches.subcommand_matches("get").unwrap();
                let address = matches.value_of("address").unwrap();
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