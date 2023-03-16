use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScryError {
    #[error("Buffer size too large")]
    BufferSizeTooLarge,

    /// Represents a failure to read from input.
    #[error("Read error")]
    ReadError { source: std::io::Error },

    #[error("Invalid UTF-8 string error")]
    UTF8Invalid(#[from] FromUtf8Error ),

    #[error("Header is not a GZIP header.")]
    NotGZIPHeader,

    #[error("Compression method must be 8")]
    InvalidCompressionMethod,

    #[error("Header CRC is incorrect, expected 0x{expected:X} but got 0x{found:X}")]
    InvalidHeaderCRC {
        expected: u16,
        found: u16
    },

    #[error("Block type 0b11 not supported")]
    InvalidBlockType, 

    #[error("Invalid length/distance code, got size {size} and lookback {lookback}")]
    InvalidLengthDistancePair {
        lookback: u16,
        size: u16
    },

    #[error("Tried to read too many bits at once")]
    InvalidNumberOfBits {
        num: u8
    },

    /// Represents all other cases of `std::io::Error`.
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
