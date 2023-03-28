const THIRTY_TWO_KILOBYTES: usize = 32768;

// base lengths for codes from 257..=285
static BASE_LENGTHS: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];

/* Extra bits for length codes 257..=285 */
static LENGTH_EXTRA_BITS: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

// base offsets for distance codes 0..=29
static BASE_DISTS: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

static DIST_EXTRA_BITS: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

static CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15
];

const MAX_SYMBOL_CODES: usize = 286;
const MAX_DISTANCE_CODES: usize = 30;

use std::cmp::min;
use std::io::{Error, ErrorKind, Read};
use std::mem::{self, discriminant};

use crate::header::read_header;
use crate::huffman::MAX_HUFFMAN_BITS;
use crate::{
    circle::CircularBuffer, errors::ScryError, huffman::HuffmanTree, reader::ScryByteReader,
};

#[derive(Debug, PartialEq)]
pub enum BlockType {
    NoCompression,
    FixedHuffman,
    DynamicHuffman,
}

#[derive(PartialEq)] 
pub enum DeflatorState {
    // read a GZIP member header.
    GZIPHeader,
    // read a DEFLATE block header. This tells us if it's the final block; and what type of block it is.
    BlockHeader,
    // read header of non-compressed block (BTYPE=00), which tells us how many bytes to read.
    PrepareNonCompressedBlock,
    // copy bytes directly from input to output.
    NonCompressedBlock { len: u16 },
    // if BTYPE=10, decode huffman trees encoded in the stream.
    PrepareDynamicBlock,
    // if BTYPE=01, or BTYPE=10, decode the input stream.
    DecodeBlock {
        symbol_tree: HuffmanTree,
        distance_tree: HuffmanTree
    },
    // copy bytes from the buffer to the output.
    WriteLookback {
        current: u16,
        len: u16,
        symbol_tree: HuffmanTree,
        distance_tree: HuffmanTree
    },
    // state that checks if we're in the final block.
    CheckIfFinalBlock,
    // read GZIP CRC and ISIZE
    GZIPFooter,
    // we're done.
    Done,
}

#[derive(Debug, PartialEq)]
pub struct BlockHeader {
    block_type: BlockType,
    is_final: bool,
}

pub struct Deflator<R> {
    pub buffer: CircularBuffer,
    state: DeflatorState,
    in_final_block: bool,
    reader: ScryByteReader<R>,
}

impl<R: Read> Deflator<R> {
    pub fn new(reader: ScryByteReader<R>) -> Self {
        Self {
            buffer: CircularBuffer::new(THIRTY_TWO_KILOBYTES),
            state: DeflatorState::GZIPHeader,
            in_final_block: false,
            reader,
        }
    }

    pub fn read_block_header(&mut self) -> Result<BlockHeader, ScryError> {
        let is_final = self.reader.read_bit()?;
        let block_bits = self.reader.read_n_bits_le(2)?;
        let block_type = match block_bits {
            0b00 => BlockType::NoCompression,
            0b01 => BlockType::FixedHuffman,
            0b10 => BlockType::DynamicHuffman,
            _ => return Err(ScryError::InvalidBlockType),
        };
        Ok(BlockHeader {
            block_type,
            is_final: is_final == 1,
        })
    }

    pub fn decode(reader: &mut ScryByteReader<R>, tree: &HuffmanTree) -> Result<u16, ScryError> {
        let mut byte: u16 = 0;
        let mut len = 0;
        loop {
            let bit = reader.read_bit()? as u16;
            byte = (byte << 1) | bit;
            len += 1;
            if let Some(symbol) = tree.decode(byte, len) {
                break Ok(symbol);
            };
            if (len as u16) == MAX_HUFFMAN_BITS {
                break Err(ScryError::InvalidHuffmanCode { code: byte });
            };
        }
    }

    fn state_transition(&mut self, buf: &mut [u8]) -> Result<usize, ScryError> {
        let mut bytes_written = 0;
        self.state = match &mut self.state {
            DeflatorState::GZIPHeader => {
                match read_header(&mut self.reader) {
                    Ok(_header) => DeflatorState::BlockHeader,
                    Err(err) => match err {
                        ScryError::ExpectedEOF => DeflatorState::Done,
                        _ => return Err(err)
                    }
                }
            },
            DeflatorState::BlockHeader => {
                let block_header = self.read_block_header()?;
                self.in_final_block = block_header.is_final; // read in CheckIfFinalBlock later.
                match block_header.block_type {
                    BlockType::NoCompression => DeflatorState::PrepareNonCompressedBlock,
                    BlockType::DynamicHuffman => DeflatorState::PrepareDynamicBlock,
                    BlockType::FixedHuffman => {
                        let symbol_tree = HuffmanTree::fixed();
                        let distance_tree = HuffmanTree::fixed_dist();
                        DeflatorState::DecodeBlock {
                            symbol_tree,
                            distance_tree
                        }
                    }
                }
            }
            DeflatorState::PrepareNonCompressedBlock => {
                self.reader.discard_until_next_byte();
                let len = self.reader.read_u16_le()?;
                let _nlen = self.reader.read_u16_le()?;
                DeflatorState::NonCompressedBlock { len }
            }
            DeflatorState::NonCompressedBlock { len: size } => {
                let len = buf.len() as u16;
                let num_bytes = min(*size, len);
                for i in 0..num_bytes {
                    let i = i as usize;
                    let byte = self.reader.read_u8()?;
                    self.buffer.push(byte);
                    buf[i] = byte;
                }
                bytes_written = num_bytes as usize;
                let remaining_bytes = *size - num_bytes;
                if remaining_bytes == 0 {
                    DeflatorState::CheckIfFinalBlock
                } else {
                    DeflatorState::NonCompressedBlock {
                        len: remaining_bytes,
                    }
                }
            }
            DeflatorState::PrepareDynamicBlock => {
                let num_literals = self.reader.read_n_bits_le(5)? + 257;  // # of literal/length codes
                let num_dists = self.reader.read_n_bits_le(5)? + 1;  // # of distance codes
                let num_code_lengths = self.reader.read_n_bits_le(4)? + 4; // # of code length codes

                // first make the code length tree.
                let mut code_lengths = [0; 19];
                for i in 0..num_code_lengths {
                    code_lengths[CODE_LENGTH_ORDER[i as usize]] = self.reader.read_n_bits_le(3)? as u8;
                }
                let cl_tree = HuffmanTree::new(&code_lengths);

                // use this tree to construct the other two trees.
                // the code lengths for the symbol and distance trees are in the same array.
                let mut combined_cls = [0; MAX_DISTANCE_CODES+MAX_SYMBOL_CODES];

                let mut index = 0;
                while index < (num_literals+num_dists) as usize {
                    // let last_len = 0;
                    let symbol = Self::decode(&mut self.reader, &cl_tree)? as u8;

                    if symbol < 16 {
                        // literal
                        combined_cls[index] = symbol;
                        index += 1;
                    } else {
                        // repeat instruction
                        let mut to_copy = 0;
                        let mut times_to_copy = 0;
                        if symbol == 16 {
                            // Copy the previous code length 3 - 6 times.
                            if index == 0 {
                                return Err(ScryError::InvalidDynamicBlockCodeLength)
                            }
                            to_copy = combined_cls[index - 1];
                            times_to_copy = 3 + self.reader.read_n_bits_le(2)?;
                        }
                        if symbol == 17 {
                            // Repeat a code length of 0 for 3 - 10 times.
                            to_copy = 0;
                            times_to_copy = 3 + self.reader.read_n_bits_le(3)?;
                        }
                        if symbol == 18 {
                            // Repeat a code length of 0 for 11 - 138 times
                            to_copy = 0;
                            times_to_copy = 11 + self.reader.read_n_bits_le(7)?;
                        }

                        for _ in 0..times_to_copy {
                            combined_cls[index] = to_copy;
                            index += 1;
                        }
                    }
                }
                let num_literals = num_literals as usize;
                let symbol_tree = HuffmanTree::new(&combined_cls[0..num_literals]);
                let distance_tree = HuffmanTree::new(&combined_cls[num_literals..combined_cls.len()]);

                DeflatorState::DecodeBlock { symbol_tree, distance_tree }
            },
            DeflatorState::DecodeBlock{symbol_tree, distance_tree} => {
                let mut i = 0;
                let next_state = loop {
                    if i >= buf.len() {
                        // we've written all we can, but we haven't finished decoding the block.
                        // next time state_transition is called we'll pick up where we left off.
                        break DeflatorState::DecodeBlock {
                            symbol_tree: mem::take(symbol_tree),
                            distance_tree: mem::take(distance_tree)
                        }
                    }
                    let symbol = Self::decode(&mut self.reader, symbol_tree)?;
                    if symbol < 256 {
                        let symbol = symbol as u8;
                        // literal
                        self.buffer.push(symbol);
                        buf[i] = symbol;
                        i += 1;
                        continue;
                    }
                    if symbol == 256 {
                        self.reader.discard_until_next_byte();
                        break DeflatorState::CheckIfFinalBlock;
                    }
                    // value between 257 and 285
                    let index = (symbol - 257) as usize;
                    let len = BASE_LENGTHS[index];
                    let len_bits = LENGTH_EXTRA_BITS[index];
                    let len = len + self.reader.read_n_bits_le(len_bits)?;

                    let dist_symbol = Self::decode(&mut self.reader, distance_tree)? as usize;
                    let dist = BASE_DISTS[dist_symbol];
                    let dist_bits = DIST_EXTRA_BITS[dist_symbol];
                    let dist = dist + self.reader.read_n_bits_le(dist_bits)?;

                    self.buffer.push_from_buffer(dist, len)?;
                    break DeflatorState::WriteLookback {
                        current: 0,
                        len,
                        symbol_tree: mem::take(symbol_tree),
                        distance_tree: mem::take(distance_tree)
                    }
                };
                bytes_written = i;
                next_state
            },
            DeflatorState::WriteLookback { current, len, symbol_tree, distance_tree } => {
                let buf_len = buf.len();
                let len = *len;
                let current = *current;
                let num_bytes = min(len - current, buf_len as u16);

                let head = self.buffer.head(len)?;

                for i in current..current+num_bytes {
                    buf[bytes_written] = head[i as usize];
                    bytes_written += 1;
                }

                if current + num_bytes == len {
                    DeflatorState::DecodeBlock {
                        symbol_tree: mem::take(symbol_tree),
                        distance_tree: mem::take(distance_tree)
                    }
                } else {
                    DeflatorState::WriteLookback {
                        current: current + (bytes_written as u16),
                        len,
                        symbol_tree: mem::take(symbol_tree),
                        distance_tree: mem::take(distance_tree)
                    }
                }
            },
            DeflatorState::CheckIfFinalBlock => {
                if self.in_final_block {
                    DeflatorState::GZIPFooter
                } else {
                    DeflatorState::BlockHeader
                }
            },
            DeflatorState::GZIPFooter => {
                // uhhhhh
                // read four bytes crc32
                let _crc32 = self.reader.read_u32_le()?;
                let _isize = self.reader.read_u32_le()?;
                DeflatorState::GZIPHeader
            },
            DeflatorState::Done => DeflatorState::Done,
        };
        Ok(bytes_written)
    }

    fn read_internal(&mut self, buf: &mut [u8]) -> Result<usize, ScryError> {
        let mut bytes_written = 0;
        while bytes_written == 0 {
            bytes_written += self.state_transition(buf)?;
            if discriminant(&self.state) == discriminant(&DeflatorState::Done) {
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
            Err(e) => std::io::Result::Err(Error::new(ErrorKind::Other, e)),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        io::{Read, Write},
        mem::discriminant,
    };

    use flate2::{write::{DeflateEncoder, GzEncoder}, Compression};
    use rstest::rstest;

    use crate::{
        decompress::{BlockType, Deflator},
        reader::ScryByteReader,
    };

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
        let mut e = GzEncoder::new(v, Compression::none());
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

    #[rstest]
    pub fn test_deflate_fixed_compressed_block() {
        let v: Vec<u8> = Vec::new();
        let mut e = GzEncoder::new(v, Compression::fast());
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

    #[rstest]
    pub fn test_deflate_fixed_compressed_block_2() {
        // check bytes() works
        let v: Vec<u8> = Vec::new();
        let mut e = GzEncoder::new(v, Compression::fast());
        e.write_all(b"hello world").unwrap();
        let v = e.finish().unwrap();
        let v = v.as_slice();
        let reader = ScryByteReader::new(v);
        let deflator = Deflator::new(reader);

        let mut deflator = deflator.bytes();

        assert_eq!(0x68, deflator.next().unwrap().unwrap());
        assert_eq!(0x65, deflator.next().unwrap().unwrap());
        assert_eq!(0x6c, deflator.next().unwrap().unwrap());
        assert_eq!(0x6c, deflator.next().unwrap().unwrap());
        assert_eq!(0x6f, deflator.next().unwrap().unwrap());
        assert_eq!(0x20, deflator.next().unwrap().unwrap());
        assert_eq!(0x77, deflator.next().unwrap().unwrap());
        assert_eq!(0x6f, deflator.next().unwrap().unwrap());
        assert_eq!(0x72, deflator.next().unwrap().unwrap());
        assert_eq!(0x6c, deflator.next().unwrap().unwrap());
        assert_eq!(0x64, deflator.next().unwrap().unwrap());
        assert_eq!(discriminant(&None), discriminant(&deflator.next()));
    }

    #[rstest]
    pub fn test_deflate_fixed_compressed_block_3() {
        // hello world is all literals.
        // try something which repeats a bit more.
        let v: Vec<u8> = Vec::new();
        let mut e = GzEncoder::new(v, Compression::fast());
        e.write_all(b"aaaaaaaaaaaaaaaaaaaaaabbbbbbb").unwrap();
        let v = e.finish().unwrap();
        let v = v.as_slice();
        let reader = ScryByteReader::new(v);
        let mut deflator = Deflator::new(reader);
        let mut dest: Vec<u8> = Vec::new();

        // deflator.read(&mut dest).unwrap();
        deflator.read_to_end(&mut dest).unwrap();
        let dest = String::from_utf8(dest.to_vec()).unwrap();

        assert_eq!(dest, "aaaaaaaaaaaaaaaaaaaaaabbbbbbb".to_string());
    }

    #[rstest]
    pub fn test_deflate_dynamic_block() {
        // hello world is all literals.
        // try something which repeats a bit more.
        let v: Vec<u8> = Vec::new();
        let mut e = GzEncoder::new(v, Compression::fast());
        e.write_all(b"AYAYA waenfiopnwaeiofon vnvnvnvnvnvna lklklkklkl ffffff AYAYAYA FFFFFFF").unwrap();
        let v = e.finish().unwrap();
        let v = v.as_slice();
        let reader = ScryByteReader::new(v);
        let mut deflator = Deflator::new(reader);
        let mut dest: Vec<u8> = vec![0; 0];

        // deflator.read(&mut dest).unwrap();
        deflator.read_to_end(&mut dest).unwrap();
        let dest = String::from_utf8(dest.to_vec()).unwrap();

        assert_eq!(dest, "AYAYA waenfiopnwaeiofon vnvnvnvnvnvna lklklkklkl ffffff AYAYAYA FFFFFFF".to_string());
    }

    #[rstest]
    pub fn test_multiple_gzip_members() {
        let v: Vec<u8> = Vec::new();
        let mut e = GzEncoder::new(v, Compression::fast());
        e.write_all(b"hello world").unwrap();
        let mut v = e.finish().unwrap();

        let v2: Vec<u8> = Vec::new();
        let mut e2 = GzEncoder::new(v2, Compression::fast());
        e2.write_all(b"hello world").unwrap();
        let mut v2 = e2.finish().unwrap();

        v.append(&mut v2);
        let v = v.as_slice();

        let reader = ScryByteReader::new(v);
        let mut deflator = Deflator::new(reader);
        let mut dest: Vec<u8> = vec![0; 0];

        // deflator.read(&mut dest).unwrap();
        deflator.read_to_end(&mut dest).unwrap();
        let dest = String::from_utf8(dest.to_vec()).unwrap();

        assert_eq!(dest, "hello worldhello world".to_string());
    }
}
