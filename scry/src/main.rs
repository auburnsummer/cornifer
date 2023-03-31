use std::fs;
use std::io;
use std::io::BufReader;
use clap::Parser;
use scry::decompress::Deflator;
use scry::errors::ScryError;
use scry::reader::ScryByteReader;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// File to decompress.
    file_name: String,
    /// File to output to.
    output_file_name: String
}

fn main() -> Result<(), ScryError> {
    let cli = Cli::parse();
    let file_name = cli.file_name;
    let file = match fs::File::open(file_name) {
        Ok(f) => f,
        Err(err) => return Err(ScryError::IOError(err))
    };

    // let bf = BufReader::new(file);

    let output_file_name = cli.output_file_name;
    let mut output_file = match fs::File::create(output_file_name) {
        Ok(f) => f,
        Err(err) => return Err(ScryError::IOError(err))
    };

    let mut decompressor = Deflator::new(ScryByteReader::new(file));

    match io::copy(&mut decompressor, &mut output_file) {
        Ok(_bytes) => (),
        Err(err) => return Err(ScryError::IOError(err))
    }


    Ok(())
}
