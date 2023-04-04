use clap::Parser;
use flate2::CrcWriter;
use indicatif::{ProgressBar, ProgressStyle};
use scry::checkpoint::Checkpointer;
use scry::decompress::Deflator;
use scry::reader::ScryByteReader;
use std::fs;
use std::io::sink;
use std::io::BufReader;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// File to generate checkpoints for
    file_name: String,

    /// File to write the checkpoints to. Should not already exist.
    #[arg(short, long)]
    output_checkpoint: String
}

fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();
    let file_name = cli.file_name;
    let checkpoint_file_name = cli.output_checkpoint;
    let file = fs::File::open(file_name)?;
    let file_len = file.metadata()?.len();
    let progress_bar = ProgressBar::new(file_len);
    progress_bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:80.cyan/blue} {pos}/{len} {msg}").unwrap().progress_chars("=>."));

    let bf = BufReader::new(progress_bar.wrap_read(file));
    let checkpointer = match Checkpointer::init(checkpoint_file_name) {
        Ok(c) => c,
        Err(_) => {
            println!("Could not create the checkpoint file. Exiting.");
            exit(1);
        }
    };
    println!("Beginning checkpointing...");
    let mut decompressor = Deflator::new(ScryByteReader::new(bf), checkpointer);

    let mut dest = CrcWriter::new(sink());

    std::io::copy(&mut decompressor, &mut dest)?;

    let final_crc = dest.crc().sum();
    println!("🎉🎉🎉 Done! 🎉🎉🎉");
    println!("I think the CRC of the decompressed file is {:#x}. Check this before using the checkpoint file.", final_crc);

    Ok(())
}
