const THIRTY_TWO_KILOBYTES: usize = 32768;

use std::io::{Read, ErrorKind, Error};
use std::cmp::min;
use std::mem::discriminant;

use crate::{circle::CircularBuffer, reader::ScryByteReader, errors::ScryError, huffman::HuffmanTree};

#[derive(Debug, PartialEq)]
pub enum BlockType {
    NoCompression,
    FixedHuffman,
    DynamicHuffman
}

#[derive(PartialEq)]
pub enum DeflatorState {
    ReadHeader,
    ReadNoCompressionBlockHeader,
    ReadNoCompressionBlock(u16),  // length of block
    DecodeDynamicHuffman,
    DecodeBlock(HuffmanTree),
    CheckIfFinalBlock,
    Done
}

#[derive(Debug, PartialEq)]
pub struct BlockHeader {
    block_type: BlockType,
    is_final: bool
}

pub struct Deflator<R> {
    pub buffer: CircularBuffer,
    state: DeflatorState,
    in_final_block: bool,
    reader: ScryByteReader<R>
}

impl<R: Read> Deflator<R> {
    pub fn new(reader: ScryByteReader<R>) -> Self {
        Self {
            buffer: CircularBuffer::new(THIRTY_TWO_KILOBYTES),
            state: DeflatorState::ReadHeader,
            in_final_block: false,
            reader
        }
    }

    pub fn read_block_header(&mut self) -> Result<BlockHeader, ScryError> {
        let is_final = self.reader.read_bit()?;
        let block_bits = self.reader.read_n_bits_le(2)?;
        let block_type = match block_bits {
            0b00 => BlockType::NoCompression,
            0b01 => BlockType::FixedHuffman,
            0b10 => BlockType::DynamicHuffman,
            _ => return Err(ScryError::InvalidBlockType)
        };
        Ok(BlockHeader {
            block_type,
            is_final: is_final == 1
        })
    }

    fn state_transition(&mut self, buf: &mut [u8]) -> Result<usize, ScryError> {
        let mut bytes_written = 0;
        self.state = match &self.state {
            DeflatorState::ReadHeader => {
                let block_header = self.read_block_header()?;
                self.in_final_block = block_header.is_final;
                match block_header.block_type {
                    BlockType::NoCompression => DeflatorState::ReadNoCompressionBlockHeader,
                    BlockType::DynamicHuffman => DeflatorState::DecodeDynamicHuffman,
                    BlockType::FixedHuffman => {
                        let huff = HuffmanTree::fixed();
                        DeflatorState::DecodeBlock(huff)
                    }
                }
            }
            DeflatorState::ReadNoCompressionBlockHeader => {
                self.reader.discard_until_next_byte();
                let len = self.reader.read_u16_le()?;
                let _nlen = self.reader.read_u16_le()?;
                DeflatorState::ReadNoCompressionBlock(len)
            }
            DeflatorState::ReadNoCompressionBlock(size) => {
                let len = buf.len() as u16;
                let num_bytes = min(*size, len);
                for i in 0..num_bytes {
                    let i = i as usize;
                    let byte = self.reader.read_u8()?;
                    self.buffer.push(byte);
                    buf[i] = byte;
                }
                bytes_written = num_bytes as usize; 
                let remaining_bytes = size - num_bytes;
                if remaining_bytes <= 0 {
                    DeflatorState::CheckIfFinalBlock
                } else {
                    DeflatorState::ReadNoCompressionBlock(remaining_bytes)
                }
            }
            DeflatorState::DecodeDynamicHuffman => todo!(),
            DeflatorState::DecodeBlock(_) => todo!(),
            DeflatorState::CheckIfFinalBlock => {
                if self.in_final_block {
                    DeflatorState::Done
                } else {
                    DeflatorState::ReadHeader
                }
            }
            DeflatorState::Done => DeflatorState::Done
        };

        Ok(bytes_written)
    }

    fn read_internal(&mut self, buf: &mut [u8]) -> Result<usize, ScryError> {
        let mut bytes_written = 0;
        while bytes_written == 0 {
            bytes_written += self.state_transition(buf)?;
            let new_state = discriminant(&self.state);
            if new_state == discriminant(&DeflatorState::Done) {
                break;
            }
    
        }
        Ok(bytes_written)
    }


}

impl<R: Read> Read for Deflator<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.read_internal(buf) {
            Ok(n) => std::io::Result::Ok(n),
            Err(e) => std::io::Result::Err(Error::new(ErrorKind::Other, e))
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::{Write, Read};

    use flate2::{write::DeflateEncoder, Compression};
    use rstest::rstest;

    use crate::{reader::ScryByteReader, deflate::{Deflator, BlockType}};

    #[rstest]
    pub fn test_read_block_header() {
        let v: Vec<u8> = Vec::new();
        let mut e = DeflateEncoder::new(v, Compression::fast());
        e.write_all(b"hello world").unwrap();
        let v = e.finish().unwrap();
        let v = v.as_slice();
        let reader = ScryByteReader::new(v);
        let mut deflator = Deflator::new(reader);
        let block_header = deflator.read_block_header().unwrap();

        assert_eq!(block_header.block_type, BlockType::FixedHuffman);
        assert_eq!(block_header.is_final, true);
    }

    #[rstest]
    pub fn test_deflate_non_compressed_block() {
        let v: Vec<u8> = Vec::new();
        let mut e = DeflateEncoder::new(v, Compression::none());
        e.write_all(b"hello world").unwrap();
        let v = e.finish().unwrap();
        let v = v.as_slice();
        let reader = ScryByteReader::new(v);
        let mut deflator = Deflator::new(reader);

        let mut dest: Vec<u8> = Vec::new();

        // deflator.read(&mut dest).unwrap();
        deflator.read_to_end(&mut dest).unwrap();
        let dest = &dest[0..11];
        let dest = String::from_utf8(dest.to_vec()).unwrap();

        assert_eq!(dest, "hello world".to_string());
    }
}