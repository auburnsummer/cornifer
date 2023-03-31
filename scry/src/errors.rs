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
    UTF8Invalid(#[from] FromUtf8Error),

    #[error("Header is not a GZIP header.")]
    NotGZIPHeader,

    #[error("Compression method must be 8")]
    InvalidCompressionMethod,

    #[error("Header CRC is incorrect, expected 0x{expected:X} but got 0x{found:X}")]
    InvalidHeaderCRC { expected: u16, found: u16 },

    #[error("Block type 0b11 not supported")]
    InvalidBlockType,

    #[error("Invalid non-compressed block NLEN, expected 0x{expected:X} but got 0x{found:X}")]
    InvalidNonCompressedBlockHeader {
        expected: u16,
        found: u16
    },

    #[error("GZIP member CRC is incorrect at 0x{position:X}, expected 0x{expected:X} but got 0x{found:X}")]
    InvalidGZIPCRC { position: u32, expected: u32, found: u32 },

    #[error("GZIP member ISIZE is incorrect at 0x{position:X}, expected 0x{expected:X} but got 0x{found:X}")]
    InvalidGZIPIsize { position: u32, expected: u32, found: u32 },

    #[error("Invalid length/distance code, got size {size} and lookback {lookback}")]
    InvalidLengthDistancePair { lookback: u16, size: u16 },

    #[error("Tried to read too many bits at once, {num}")]
    InvalidNumberOfBits { num: u8 },

    #[error("Invalid Huffman code, {code} at position 0x{position:X}:{bit}")]
    InvalidHuffmanCode { code: u16, position: u32, bit: u8 },

    #[error("Invalid Dynamic Block due to attempting to copy a code length at 0")]
    InvalidDynamicBlockCodeLength,

    #[error("EOF")]
    EOF,  // could be expected! maybe not.

    #[error("Expected EOF")]
    ExpectedEOF,

    /// Represents all other cases of `std::io::Error`.
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
