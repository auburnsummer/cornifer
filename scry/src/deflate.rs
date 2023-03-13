// use std::{io::{Read, Error, ErrorKind}, cmp::min};

// use crate::{reader::ScryByteReader, errors::ScryError, huffman::HuffmanTree};
// use anyhow::{ Result, bail };
// use circular_queue::CircularQueue;

// const THIRTY_TWO_KILOBYTES: usize = 32768;

// use thiserror::Error;

// #[derive(Error, Debug)]
// #[error(transparent)]
// pub struct AsStdError(#[from] anyhow::Error);

// #[derive(Debug, PartialEq)]
// pub enum BlockType {
//     NoCompression,
//     FixedHuffman,
//     DynamicHuffman
// }

// #[derive(PartialEq)]
// pub enum DeflatorState {
//     ReadHeader,
//     ReadNoCompressionBlockHeader,
//     ReadNoCompressionBlock(u16),
//     DecodeDynamicHuffman,
//     DecodeBlock(HuffmanTree)
// }

// #[derive(Debug, PartialEq)]
// pub struct BlockHeader {
//     block_type: BlockType,
//     is_final: bool
// }

// struct Deflator<R> {
//     pub buffer: CircularQueue<u8>,
//     state: DeflatorState,
//     reader: ScryByteReader<R>
// }

// impl<R: Read> Deflator<R> {
//     pub fn new(reader: ScryByteReader<R>) -> Self {
//         Self {
//             buffer: CircularQueue::with_capacity(THIRTY_TWO_KILOBYTES),
//             reader,
//             state: DeflatorState::ReadHeader
//         }
//     }

//     pub fn buffer_add(&mut self, byte: u8) -> u8 {
//         self.buffer.push(byte);
//         byte
//     }

//     pub fn read_block_header(&mut self) -> Result<BlockHeader, ScryError> {
//         let is_final = self.reader.read_bit()?;
//         let b = self.reader.read_two_bits()?;
//         let block_type = match b {
//             0b00 => BlockType::NoCompression,
//             0b01 => BlockType::FixedHuffman,
//             0b10 => BlockType::DynamicHuffman,
//             _ => return Err(ScryError::InvalidBlockType)
//         };
//         Ok(BlockHeader {
//             block_type,
//             is_final: is_final == 1
//         })
//     }

//     fn read_internal(&mut self, buf: &mut [u8]) -> Result<usize, ScryError> {
//         let mut bytes_read = 0;
//         while bytes_read < 1024 {
//             self.state = match &mut self.state {
//                 DeflatorState::ReadHeader => {
//                     match self.read_block_header() {
//                         Err(_) => return Ok(bytes_read),
//                         Ok(bh) => {
//                             match bh.block_type {
//                                 BlockType::NoCompression => DeflatorState::ReadNoCompressionBlockHeader,
//                                 BlockType::DynamicHuffman => DeflatorState::DecodeDynamicHuffman,
//                                 BlockType::FixedHuffman => {
//                                     let huff = HuffmanTree::fixed();
//                                     DeflatorState::DecodeBlock(huff)
//                                 }
//                             }
//                         }
//                     }
//                 }
//                 DeflatorState::ReadNoCompressionBlockHeader => {
//                     self.reader.discard_until_next_byte();
//                     let len = self.reader.read_u16_le()?;
//                     let _nlen = self.reader.read_u16_le()?;
//                     DeflatorState::ReadNoCompressionBlock(len)
//                 }
//                 DeflatorState::DecodeDynamicHuffman => todo!(),
//                 DeflatorState::DecodeBlock(tree) => todo!(),
//                 DeflatorState::ReadNoCompressionBlock(size) => {
//                     let number_of_bytes_to_read = min(*size as usize, buf.len());
//                     for i in 0..number_of_bytes_to_read {
//                         let byte = self.buffer_add(self.reader.read_u8()?);
//                         buf[i] = byte;
//                     }
//                     bytes_read += number_of_bytes_to_read;
//                     if number_of_bytes_to_read == *size as usize {
//                         DeflatorState::ReadHeader
//                     } else {
//                         let remaining_bytes = *size - number_of_bytes_to_read as u16;
//                         DeflatorState::ReadNoCompressionBlock(remaining_bytes)
//                     }
//                 }
//             }
//         }
//         Ok(bytes_read)
//     }
// }

// impl<R: Read> Read for Deflator<R> {
//     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
//         match self.read_internal(buf) {
//             Ok(n) => std::io::Result::Ok(n),
//             Err(e) => std::io::Result::Err(Error::new(ErrorKind::Other, e))
//         }
//     }
// }

// #[cfg(test)]
// mod test {
//     use std::io::{Write, Read};

//     use rstest::rstest;

//     use flate2::Compression;
//     use flate2::write::DeflateEncoder;

//     use crate::deflate::{Deflator, BlockType};
//     use crate::reader::ScryByteReader;
//     use anyhow::{ Result };

//     #[rstest]
//     pub fn test_read_block_header() {
//         let v: Vec<u8> = Vec::new();
//         let mut e = DeflateEncoder::new(v, Compression::fast());
//         e.write_all(b"hello world").unwrap();
//         let v = e.finish().unwrap();
//         let v = v.as_slice();
//         let reader = ScryByteReader::new(v);
//         let mut deflator = Deflator::new(reader);

//         assert_eq!(deflator.read_block_header().unwrap().block_type, BlockType::FixedHuffman)
//     }

//     #[rstest]
//     pub fn test_deflate_non_compressed_block() {
//         let v: Vec<u8> = Vec::new();
//         let mut e = DeflateEncoder::new(v, Compression::none());
//         e.write_all(b"hello world").unwrap();
//         let v = e.finish().unwrap();
//         let v = v.as_slice();
//         let reader = ScryByteReader::new(v);
//         let mut deflator = Deflator::new(reader);

//         let mut dest: Vec<u8> = vec![0; 300];

//         deflator.read(&mut dest).unwrap();
//         let dest = &dest[0..11];
//         let dest = String::from_utf8(dest.to_vec()).unwrap();

//         assert_eq!(dest, "hello world".to_string());
//     }
// }