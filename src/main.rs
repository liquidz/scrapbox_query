#![recursion_limit = "1024"]

extern crate clap;
extern crate rustc_serialize;
extern crate toml;
#[macro_use]
extern crate error_chain;

mod errors {
    error_chain!{}
}

use clap::{Arg, App, SubCommand};
use errors::*;
use rustc_serialize::json;
use std::env;
use std::fs::File;
use std::io::prelude::*;

mod scrapbox;

static CONFIG_FILE: &'static str = ".config/scrapq/config.toml";

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
                let json_data = read_file(json_file).expect("FIXME");
                if let Err(ref e) = scrapbox::initialize_index(json_data.as_str(),
                                                               config.index_path.as_str()) {
                    println!("{:?}", e);
                } else {
                    println!("finish to create index");
                }
            }
            Some("search") => {
                let matches = matches.subcommand_matches("search")
                    .expect("should match 'search' subcommand");
                let query = matches.value_of("query").expect("should match 'query' value");
                match scrapbox::search_documents(config.index_path.as_str(), query) {
                    Ok(results) => {
                        if matches.is_present("json") {
                            println!("{}", json::encode(&results).expect("failed to encode JSON"));
                        } else {
                            for result in results {
                                println!("{}", result);
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
                match scrapbox::retrieve_document(config.index_path.as_str(), address) {
                    Ok(body) => println!("{}", body),
                    Err(ref e) => println!("{:?}", e),
                }
            }
            e => println!("error: {:?}", e),
        }
    }
}
