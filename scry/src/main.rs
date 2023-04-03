use std::fs;
use std::io;
use std::io::BufReader;
use std::io::sink;
use clap::Parser;
use flate2::CrcWriter;
use scry::decompress::Deflator;
use scry::reader::ScryByteReader;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// File to decompress.
    file_name: String
}

fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();
    let file_name = cli.file_name;
    let file = fs::File::open(file_name)?;

    let bf = BufReader::new(file);

    let mut decompressor = Deflator::new(ScryByteReader::new(bf));

    let mut dest = CrcWriter::new(sink());

    println!("The CRC32 of the decompressed data is...");

    io::copy(&mut decompressor, &mut dest)?;

    let result = dest.crc().sum();
    println!("{:#x}", result);

    Ok(())
}
