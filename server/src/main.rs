#![feature(test)]
use crate::server::Server;
use crate::simple_sublist::SimpleSubList;
use crate::sublist::TrieSubList;
use std::error::Error;

mod client;
mod error;
mod parser;
mod server;
mod simple_sublist;
mod sublist;
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("server start..");
    let s: Server<TrieSubList> = Server::default();
    s.start().await
}
