use std::env;

pub mod header;
pub mod reader;
pub mod huffman;

fn main() {
    let args: Vec<String> = env::args().collect();
    dbg!(args);
}
