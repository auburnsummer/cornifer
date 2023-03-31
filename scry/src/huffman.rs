use std::{collections::HashMap, hash::BuildHasherDefault};

use nohash_hasher::{IntMap, NoHashHasher};

pub const MAX_HUFFMAN_BITS: u16 = 15;

#[derive(PartialEq, Default)]
pub struct HuffmanTree {
    lut: HashMap<u16, HuffmanCode, BuildHasherDefault<NoHashHasher<u16>>>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct HuffmanCode {
    pub symbol: u16,
    pub len: u8,
}

impl HuffmanTree {
    pub fn new(bit_lengths: &[u8]) -> Self {
        // Count the number of codes for each code length.  Let
        // bl_count[N] be the number of codes of length N, N >= 1.
        let mut bl_count = [0_u16; (MAX_HUFFMAN_BITS + 1) as usize];
        for &len in bit_lengths {
            let len = len as usize;
            bl_count[len] += 1;
        }
        bl_count[0] = 0;

        // 2)  Find the numerical value of the smallest code for each
        // code length:
        let mut next_code = [0_u16; (MAX_HUFFMAN_BITS + 1) as usize];

        let mut code: u16 = 0;
        for bits in 1..=MAX_HUFFMAN_BITS {
            let bits = bits as usize;
            code = (code + bl_count[bits-1]) << 1;
            next_code[bits] = code;
        }
        // Assign numerical values to all codes.
        let mut final_codes = vec![0_u16; bit_lengths.len()];
        for (i, &len) in bit_lengths.iter().enumerate() {
            if len != 0 {
                let len = len as usize;
                let code = next_code[len];
                next_code[len] += 1;
                final_codes[i] = code;
            } else {
                final_codes[i] = 0;
            }
        }
        // put them in the lookup table.
        let mut lut: IntMap<u16, HuffmanCode> = IntMap::default();
        for i in 0..bit_lengths.len() {
            let len = bit_lengths[i];
            let code = final_codes[i];
            let i = i as u16;
            if len > 0 {
                lut.insert(code, HuffmanCode { symbol: i, len });
            }
        }

        Self { lut }
    }

    pub fn fixed() -> Self {
        let mut test_values: Vec<u8> = vec![];
        for (next, bit_len) in [(143, 8), (255, 9), (279, 7), (287, 8)] {
            test_values.resize(next + 1, bit_len);
        }

        Self::new(&test_values)
    }

    pub fn fixed_dist() -> Self {
        let test_values_dist: Vec<u8> = vec![5; 31];
        Self::new(&test_values_dist)
    }

    pub fn decode(&self, code: u16, len: u8) -> Option<u16> {
        let lookup = self.lut.get(&code)?;
        if len == lookup.len {
            Some(lookup.symbol)
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn get_lut(&self) -> &HashMap<u16, HuffmanCode, BuildHasherDefault<NoHashHasher<u16>>> {
        return &self.lut;
    }

    pub fn export(&self) {}
}

/**
 * TESTS
 */
#[cfg(test)]
mod test {
    use rstest::*;

    use crate::huffman::HuffmanCode;

    use super::HuffmanTree;

    #[rstest]
    pub fn test_lut_values_correct() {
        let test_values: [u8; 8] = [3, 3, 3, 3, 3, 2, 4, 4];
        let tree = HuffmanTree::new(&test_values);

        let codes = tree.get_lut();

        /*
           Symbol Length   Code
           ------ ------   ----
           0       3        010
           1       3        011
           2       3        100
           3       3        101
           4       3        110
           5       2         00
           6       4       1110
           7       4       1111
        */
        assert_eq!(codes.get(&0b01), None);
        assert_eq!(codes.get(&0b010), Some(&HuffmanCode { symbol: 0, len: 3 }));
        assert_eq!(codes.get(&0b1111), Some(&HuffmanCode { symbol: 7, len: 4 }));
        assert_eq!(codes.get(&0b00), Some(&HuffmanCode { symbol: 5, len: 2 }));
    }

    #[rstest]
    pub fn test_lut_values_with_gaps() {
        let test_values: [u8; 12] = [0, 3, 3, 3, 0, 3, 3, 2, 0, 4, 4, 0];
        /*
           Symbol Length   Code
           ------ ------   ----
           0       N/A
           1       3        010
           2       3        011
           3       3        100
           4       N/A
           5       3        101
           6       3        110
           7       2         00
           8       N/A
           9       4       1110
          10       4       1111
          11       N/A
        */
        let tree = HuffmanTree::new(&test_values);

        let codes = tree.get_lut();

        assert_eq!(codes.get(&0b01), None);
        assert_eq!(codes.get(&0b010), Some(&HuffmanCode { symbol: 1, len: 3 }));
        assert_eq!(codes.get(&0b1111), Some(&HuffmanCode { symbol: 10, len: 4 }));
        assert_eq!(codes.get(&0b00), Some(&HuffmanCode { symbol: 7, len: 2 }));

    }

    #[rstest]
    pub fn test_lut_values_with_gaps_2() {
        //
        // Symbol  Len  Code
        // 0       11  11111111100
        // 1       12  111111111110
        // 2       11  11111111101
        // 3       12  111111111111
        // 4       N/A 
        // 5       11  11111111110
        // 6       9   111111110
        // 7       8   11111110
        // 8       7   1111100
        // 9       7   1111101
        // 10      7   1111110
        // 11      6   111010
        // 12      6   111011
        // 13      6   111100
        // 14      5   11010
        // 15      5   11011
        // 16      4   0010
        // 17      5   11100
        // 18      4   0011
        // 19      4   0100
        // 20      4   0101
        // 21      4   0110
        // 22      3   000
        // 23      4   0111
        // 24      4   1000
        // 25      4   1001
        // 26      4   1010
        // 27      4   1011
        // 28      4   1100
        // 29      6   111101
        let test_values = [
            11_u8, 12, 11, 12,
            0, 11, 9, 8,
            7, 7, 7, 6,
            6, 6, 5, 5,
            4, 5, 4, 4,
            4, 4, 3, 4,
            4, 4, 4, 4,
            4, 6, 0, 0,
            0, 0, 0, 0,
            0, 0
        ];
        let tree = HuffmanTree::new(&test_values);

        let codes = tree.get_lut();
        assert_eq!(codes.get(&0b1011), Some(&HuffmanCode { symbol: 27, len: 4 }));
        assert_eq!(codes.get(&0b11111111110), Some(&HuffmanCode { symbol: 5, len: 11 }));

    }

    #[rstest]
    pub fn test_lut_fixed() {
        let tree = HuffmanTree::fixed();
        let codes = tree.get_lut();
        /*
        Lit Value    Bits        Codes
        ---------    ----        -----
          0 - 143     8          00110000 through
                                10111111
        144 - 255     9          110010000 through
                                111111111
        256 - 279     7          0000000 through
                                0010111
        280 - 287     8          11000000 through
                                11000111
         */
        assert_eq!(
            codes.get(&0b110001),
            Some(&HuffmanCode { symbol: 1, len: 8 })
        );
        assert_eq!(
            codes.get(&0b11000111),
            Some(&HuffmanCode {
                symbol: 287,
                len: 8
            })
        );
        assert_eq!(
            codes.get(&0b111111110),
            Some(&HuffmanCode {
                symbol: 254,
                len: 9
            })
        );
        assert_eq!(
            codes.get(&0b0000000),
            Some(&HuffmanCode {
                symbol: 256,
                len: 7
            })
        );
        assert_eq!(codes.get(&0b1111111111), None);
    }

    #[rstest]
    pub fn test_decode() {
        let test_values: [u8; 8] = [3, 3, 3, 3, 3, 2, 4, 4];
        let tree = HuffmanTree::new(&test_values);
        assert_eq!(tree.decode(0b0, 1), None);
        assert_eq!(tree.decode(0b10, 2), None);
        assert_eq!(tree.decode(0b010, 3), Some(0));
    }
}
